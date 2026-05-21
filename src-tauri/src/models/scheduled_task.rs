use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub id: String,
    pub agent_id: String,
    pub description: String,
    pub task_type: String, // "single" | "recurring"
    pub trigger_mode: Option<String>, // "after_minutes" | "datetime"
    pub after_minutes: Option<i32>,
    pub year: Option<i32>,
    pub month: Option<i32>,
    pub day: Option<i32>,
    pub hour: Option<i32>,
    pub minute: Option<i32>,
    pub interval_minutes: Option<i32>,
    pub next_trigger_at: i64,
    pub created_at: i64,
    pub is_active: i32,
    pub target_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTimerRequest {
    pub description: String,
    pub task_type: String,
    pub trigger_mode: Option<String>,
    pub after_minutes: Option<i32>,
    pub year: Option<i32>,
    pub month: Option<i32>,
    pub day: Option<i32>,
    pub hour: Option<i32>,
    pub minute: Option<i32>,
    pub interval_minutes: Option<i32>,
    pub target_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTimerRequest {
    pub id: String,
    pub description: Option<String>,
    pub task_type: Option<String>,
    pub trigger_mode: Option<String>,
    pub after_minutes: Option<i32>,
    pub year: Option<i32>,
    pub month: Option<i32>,
    pub day: Option<i32>,
    pub hour: Option<i32>,
    pub minute: Option<i32>,
    pub interval_minutes: Option<i32>,
    pub next_trigger_at: Option<i64>,
    pub target_session_id: Option<String>,
}
