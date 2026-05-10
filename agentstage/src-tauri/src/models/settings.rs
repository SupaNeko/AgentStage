use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub updated_at: i64,
}
