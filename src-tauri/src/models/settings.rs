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
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
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
}
