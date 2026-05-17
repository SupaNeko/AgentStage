use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct ModelConfig {
    pub model_provider: String,
    pub model_name: String,
    pub base_url: Option<String>,
    pub api_key: String,
    pub temperature: f64,
    pub max_tokens: i32,
    pub thinking_mode: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeneratePersonaRequest {
    pub agent_id: Option<String>,
    pub model_config: Option<ModelConfig>,
    pub reference_character: Option<String>,
    pub supplement: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeneratePersonaResponse {
    pub personality: Option<String>,
    pub scenario: Option<String>,
    pub example_messages: Option<String>,
    pub creator_notes: Option<String>,
    pub detailed_persona: String,
    pub simplified_persona: String,
}
