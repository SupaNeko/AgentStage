use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {
    pub id: i32,
    pub global_min_trigger_interval: i32,
    pub private_message_limit_default: i32,
    pub group_message_limit_default: i32,
    pub private_limit_enabled_default: bool,
    pub group_limit_enabled_default: bool,
    pub theme: String,
    pub font_size: String,
    pub language: String,
    pub enter_to_send: bool,
    pub launch_on_startup: bool,
    pub minimize_to_tray: bool,
    pub active_persona_id: Option<String>,
    pub default_avatar_path: Option<String>,
    pub quiet_hours_start: i32,
    pub quiet_hours_end: i32,
    pub summary_model_config_id: Option<String>,
    pub search_provider: Option<String>,
    pub search_api_key_encrypted: Option<Vec<u8>>,
    pub virtual_time_enabled: bool,
    pub virtual_time_base: Option<i64>,
    pub virtual_time_set_at: Option<i64>,
    pub virtual_time_rate: i32,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsResponse {
    pub global_min_trigger_interval: i32,
    pub private_message_limit_default: i32,
    pub group_message_limit_default: i32,
    pub private_limit_enabled_default: bool,
    pub group_limit_enabled_default: bool,
    pub theme: String,
    pub font_size: String,
    pub language: String,
    pub enter_to_send: bool,
    pub launch_on_startup: bool,
    pub minimize_to_tray: bool,
    pub active_persona_id: Option<String>,
    pub default_avatar_path: Option<String>,
    pub quiet_hours_start: i32,
    pub quiet_hours_end: i32,
    pub summary_model_config_id: Option<String>,
    /// 搜索 API 厂商（'bocha' | 'zhipu' | 'kimi'），不暴露明文 Key
    pub search_provider: Option<String>,
    /// 是否已保存搜索 API Key（不返回 Key 本身）
    pub search_api_key_set: bool,
    pub virtual_time_enabled: bool,
    pub virtual_time_base: Option<i64>,
    pub virtual_time_set_at: Option<i64>,
    pub virtual_time_rate: i32,
}

impl From<AppSettings> for SettingsResponse {
    fn from(s: AppSettings) -> Self {
        Self {
            global_min_trigger_interval: s.global_min_trigger_interval,
            private_message_limit_default: s.private_message_limit_default,
            group_message_limit_default: s.group_message_limit_default,
            private_limit_enabled_default: s.private_limit_enabled_default,
            group_limit_enabled_default: s.group_limit_enabled_default,
            theme: s.theme,
            font_size: s.font_size,
            language: s.language,
            enter_to_send: s.enter_to_send,
            launch_on_startup: s.launch_on_startup,
            minimize_to_tray: s.minimize_to_tray,
            active_persona_id: s.active_persona_id,
            default_avatar_path: s.default_avatar_path,
            quiet_hours_start: s.quiet_hours_start,
            quiet_hours_end: s.quiet_hours_end,
            summary_model_config_id: s.summary_model_config_id,
            search_provider: s.search_provider,
            search_api_key_set: s
                .search_api_key_encrypted
                .as_ref()
                .map(|k| !k.is_empty())
                .unwrap_or(false),
            virtual_time_enabled: s.virtual_time_enabled,
            virtual_time_base: s.virtual_time_base,
            virtual_time_set_at: s.virtual_time_set_at,
            virtual_time_rate: s.virtual_time_rate,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateAppSettingsRequest {
    pub global_min_trigger_interval: Option<i32>,
    pub private_message_limit_default: Option<i32>,
    pub group_message_limit_default: Option<i32>,
    pub private_limit_enabled_default: Option<bool>,
    pub group_limit_enabled_default: Option<bool>,
    pub theme: Option<String>,
    pub font_size: Option<String>,
    pub language: Option<String>,
    pub enter_to_send: Option<bool>,
    pub launch_on_startup: Option<bool>,
    pub minimize_to_tray: Option<bool>,
    pub active_persona_id: Option<String>,
    pub default_avatar_path: Option<String>,
    pub summary_model_config_id: Option<String>,
    /// 搜索 API 厂商；与当前不同时会清空已保存的 Key
    pub search_provider: Option<String>,
    /// 搜索 API 明文 Key（仅传输用，入库前加密）；空字符串 = 清除已存 Key
    pub search_api_key: Option<String>,
    pub virtual_time_enabled: Option<bool>,
    /// 用户设定的虚拟时间（ms 时间戳）；提供时 set_at 由后端重置为当前真实时间
    pub virtual_time_base: Option<i64>,
    /// 流速：现实 1 分钟 = 虚拟 N 分钟（整数，>= 1）
    pub virtual_time_rate: Option<i32>,
}
