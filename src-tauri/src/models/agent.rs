use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub avatar_path: Option<String>,
    pub detailed_persona: String,
    pub simplified_persona: String,
    pub personality: Option<String>,
    pub scenario: Option<String>,
    pub example_messages: Option<String>,
    pub first_message: Option<String>,
    pub creator_notes: Option<String>,
    pub tags: Option<String>,
    pub model_config_id: Option<String>,
    pub temperature: Option<f64>,
    pub long_term_memory: Option<String>,
    pub memory_enabled: bool,
    pub proactive_enabled: bool,
    pub proactive_min_minutes: i32,
    pub proactive_max_minutes: i32,
    pub is_deleted: bool,
    pub deleted_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub id: String,
    pub name: String,
    pub avatar_path: Option<String>,
    pub detailed_persona: String,
    pub simplified_persona: String,
    pub personality: Option<String>,
    pub scenario: Option<String>,
    pub example_messages: Option<String>,
    pub first_message: Option<String>,
    pub creator_notes: Option<String>,
    pub tags: Option<String>,
    pub model_config_id: Option<String>,
    pub model_name: Option<String>,
    pub temperature: Option<f64>,
    pub long_term_memory: Option<String>,
    pub memory_enabled: bool,
    pub proactive_enabled: bool,
    pub proactive_min_minutes: i32,
    pub proactive_max_minutes: i32,
    pub is_deleted: bool,
    pub deleted_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<Agent> for AgentResponse {
    fn from(agent: Agent) -> Self {
        Self {
            id: agent.id,
            name: agent.name,
            avatar_path: agent.avatar_path,
            detailed_persona: agent.detailed_persona,
            simplified_persona: agent.simplified_persona,
            personality: agent.personality,
            scenario: agent.scenario,
            example_messages: agent.example_messages,
            first_message: agent.first_message,
            creator_notes: agent.creator_notes,
            tags: agent.tags,
            model_config_id: agent.model_config_id,
            model_name: None,
            temperature: agent.temperature,
            long_term_memory: agent.long_term_memory,
            memory_enabled: agent.memory_enabled,
            proactive_enabled: agent.proactive_enabled,
            proactive_min_minutes: agent.proactive_min_minutes,
            proactive_max_minutes: agent.proactive_max_minutes,
            is_deleted: agent.is_deleted,
            deleted_at: agent.deleted_at,
            created_at: agent.created_at,
            updated_at: agent.updated_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateAgentRequest {
    pub name: String,
    pub avatar_path: Option<String>,
    pub detailed_persona: String,
    pub simplified_persona: String,
    pub personality: Option<String>,
    pub scenario: Option<String>,
    pub example_messages: Option<String>,
    pub first_message: Option<String>,
    pub creator_notes: Option<String>,
    pub tags: Option<String>,
    pub model_config_id: String,
    pub temperature: Option<f64>,
    pub long_term_memory: Option<String>,
    pub memory_enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateAgentRequest {
    pub id: String,
    pub name: Option<String>,
    pub avatar_path: Option<String>,
    pub detailed_persona: Option<String>,
    pub simplified_persona: Option<String>,
    pub personality: Option<String>,
    pub scenario: Option<String>,
    pub example_messages: Option<String>,
    pub first_message: Option<String>,
    pub creator_notes: Option<String>,
    pub tags: Option<String>,
    pub model_config_id: Option<String>,
    pub temperature: Option<Option<f64>>,
    pub long_term_memory: Option<String>,
    pub memory_enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeleteAgentRequest {
    pub id: String,
}
