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
    pub is_deleted: bool,
    pub deleted_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
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
}
