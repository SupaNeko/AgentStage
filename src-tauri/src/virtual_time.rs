use chrono::{Local, LocalResult, TimeZone};
use rusqlite::Connection;

use crate::models::settings::AppSettings;

/// 纯函数：根据配置把真实时间戳换算为虚拟时间戳。
/// 公式：virtual = base + (real - set_at) * rate
pub fn compute_virtual_ms(base: i64, set_at: i64, rate: i32, real_ms: i64) -> i64 {
    base + (real_ms - set_at) * rate.max(1) as i64
}

/// 虚拟时钟。未启用时所有方法退化为真实时间。
#[derive(Debug, Clone, Copy)]
pub struct VirtualClock {
    pub enabled: bool,
    pub base: i64,
    pub set_at: i64,
    pub rate: i32,
}

impl VirtualClock {
    /// 真实时钟（虚拟时间未启用或配置不完整时的行为）
    pub fn real() -> Self {
        Self {
            enabled: false,
            base: 0,
            set_at: 0,
            rate: 1,
        }
    }

    pub fn from_settings(s: &AppSettings) -> Self {
        match (s.virtual_time_enabled, s.virtual_time_base, s.virtual_time_set_at) {
            (true, Some(base), Some(set_at)) => Self {
                enabled: true,
                base,
                set_at,
                rate: s.virtual_time_rate.max(1),
            },
            _ => Self::real(),
        }
    }

    pub fn load(conn: &Connection) -> Self {
        match crate::db::settings::get_or_create_settings(conn) {
            Ok(s) => Self::from_settings(&s),
            Err(_) => Self::real(),
        }
    }

    /// 当前（虚拟）时间戳，ms
    pub fn now_ms(&self) -> i64 {
        let real_now = Local::now().timestamp_millis();
        self.map_ms(real_now)
    }

    /// 把一个真实时间戳映射到虚拟时间轴（用于历史消息时间戳展示）
    pub fn map_ms(&self, real_ms: i64) -> i64 {
        if self.enabled {
            compute_virtual_ms(self.base, self.set_at, self.rate, real_ms)
        } else {
            real_ms
        }
    }

    pub fn format_now(&self) -> String {
        Self::format_ms(self.now_ms())
    }

    pub fn format_ts(&self, real_ms: i64) -> String {
        Self::format_ms(self.map_ms(real_ms))
    }

    fn format_ms(ts: i64) -> String {
        match Local.timestamp_millis_opt(ts) {
            LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            _ => "??".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_virtual_ms_basic() {
        // base=10000, set_at=1000（真实）, rate=5：真实过去 60000ms → 虚拟走 300000ms
        assert_eq!(compute_virtual_ms(10_000, 1_000, 5, 61_000), 310_000);
    }

    #[test]
    fn test_compute_virtual_ms_rate_one_is_identity_shift() {
        assert_eq!(compute_virtual_ms(10_000, 1_000, 1, 61_000), 70_000);
    }

    #[test]
    fn test_compute_virtual_ms_clamps_rate() {
        // rate <= 0 按 1 处理
        assert_eq!(compute_virtual_ms(10_000, 1_000, 0, 61_000), 70_000);
    }

    #[test]
    fn test_rate_change_continuity() {
        // 旧配置：base=10000, set_at=1000, rate=1；真实 now=61000 → 虚拟 70000
        let old_virtual = compute_virtual_ms(10_000, 1_000, 1, 61_000);
        // 前端把当前虚拟时间作为新 base 提交：新配置从 70000 起以 rate=10 走
        let new_virtual = compute_virtual_ms(old_virtual, 61_000, 10, 61_000 + 6_000);
        assert_eq!(new_virtual, old_virtual + 60_000);
    }

    #[test]
    fn test_from_settings_disabled() {
        let s = AppSettings::default();
        let clock = VirtualClock::from_settings(&s);
        assert!(!clock.enabled);
        // 未启用时 map_ms 恒等
        assert_eq!(clock.map_ms(123_456), 123_456);
    }

    #[test]
    fn test_from_settings_enabled_requires_base_and_set_at() {
        let mut s = AppSettings::default();
        s.virtual_time_enabled = true;
        // 缺 base/set_at 时退化为真实时钟
        let clock = VirtualClock::from_settings(&s);
        assert!(!clock.enabled);

        s.virtual_time_base = Some(10_000);
        s.virtual_time_set_at = Some(1_000);
        s.virtual_time_rate = 5;
        let clock = VirtualClock::from_settings(&s);
        assert!(clock.enabled);
        assert_eq!(clock.map_ms(61_000), 310_000);
    }
}
