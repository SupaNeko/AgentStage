use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmUsageRecord {
    pub id: String,
    pub agent_id: String,
    pub model_config_id: String,
    pub session_id: Option<String>,
    pub trigger_type: String,
    pub call_round: i32,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
    pub message_id: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageOverview {
    pub total_calls: i64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub total_tokens: i64,
    pub daily_trend: Vec<DailyTrend>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyTrend {
    pub date: String,
    pub calls: i64,
    pub tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUsageItem {
    pub model_config_id: String,
    pub model_name: String,
    pub provider: String,
    pub calls: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentUsageItem {
    pub agent_id: String,
    pub agent_name: String,
    pub avatar_path: Option<String>,
    pub calls: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentModelUsageItem {
    pub model_config_id: String,
    pub model_name: String,
    pub calls: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelAgentUsageItem {
    pub agent_id: String,
    pub agent_name: String,
    pub calls: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionUsageItem {
    pub session_id: String,
    pub session_name: String,
    pub session_type: String,
    pub calls: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionAgentUsageItem {
    pub agent_id: String,
    pub agent_name: String,
    pub calls: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionModelUsageItem {
    pub model_config_id: String,
    pub model_name: String,
    pub calls: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionAgentModelUsageItem {
    pub agent_id: String,
    pub agent_name: String,
    pub model_config_id: String,
    pub model_name: String,
    pub calls: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerUsageItem {
    pub trigger_type: String,
    pub calls: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageFilters {
    pub agent_id: Option<String>,
    pub model_config_id: Option<String>,
    pub session_id: Option<String>,
    pub trigger_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecordDetail {
    pub id: String,
    pub agent_name: String,
    pub model_name: String,
    pub session_name: Option<String>,
    pub trigger_type: String,
    pub call_round: i32,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedUsageRecords {
    pub records: Vec<UsageRecordDetail>,
    pub total: i64,
    pub page: i32,
    pub page_size: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start_time: i64,
    pub end_time: i64,
}
