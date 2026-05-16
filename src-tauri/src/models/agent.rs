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
    pub model_provider: Option<String>,
    pub model_name: Option<String>,
    pub base_url: Option<String>,
    pub temperature: f64,
    pub max_tokens: i32,
    pub top_p: f64,
    pub presence_penalty: f64,
    pub frequency_penalty: f64,
    pub api_key_encrypted: Option<Vec<u8>>,
    pub thinking_mode: bool,
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
    pub model_provider: Option<String>,
    pub model_name: Option<String>,
    pub base_url: Option<String>,
    pub temperature: f64,
    pub max_tokens: i32,
    pub top_p: f64,
    pub presence_penalty: f64,
    pub frequency_penalty: f64,
    pub thinking_mode: bool,
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
            model_provider: agent.model_provider,
            model_name: agent.model_name,
            base_url: agent.base_url,
            temperature: agent.temperature,
            max_tokens: agent.max_tokens,
            top_p: agent.top_p,
            presence_penalty: agent.presence_penalty,
            frequency_penalty: agent.frequency_penalty,
            thinking_mode: agent.thinking_mode,
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
    pub model_provider: String,
    pub model_name: String,
    pub base_url: Option<String>,
    pub api_key: String,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i32>,
    pub thinking_mode: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateAgentRequest {
    pub id: String,
    pub name: Option<String>,
    pub avatar_path: Option<String>,
    pub detailed_persona: Option<String>,
    pub simplified_persona: Option<String>,
    pub personality: Option<String>,
    pub scenario: Option<String>,
    pub model_provider: Option<String>,
    pub model_name: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i32>,
    pub thinking_mode: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeleteAgentRequest {
    pub id: String,
}
