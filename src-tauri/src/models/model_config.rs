use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub model_name: String,
    pub base_url: Option<String>,
    pub api_key_encrypted: Option<Vec<u8>>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i32>,
    pub top_p: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub frequency_penalty: Option<f64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelConfigResponse {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub model_name: String,
    pub base_url: Option<String>,
    pub api_key: String,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i32>,
    pub top_p: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub frequency_penalty: Option<f64>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<ModelConfig> for ModelConfigResponse {
    fn from(cfg: ModelConfig) -> Self {
        let api_key = cfg.api_key_encrypted
            .as_ref()
            .and_then(|enc| crate::crypto::decrypt(enc).ok())
            .unwrap_or_default();
        Self {
            id: cfg.id, name: cfg.name, provider: cfg.provider, model_name: cfg.model_name,
            base_url: cfg.base_url, api_key, temperature: cfg.temperature,
            max_tokens: cfg.max_tokens, top_p: cfg.top_p,
            presence_penalty: cfg.presence_penalty, frequency_penalty: cfg.frequency_penalty,
            created_at: cfg.created_at, updated_at: cfg.updated_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateModelConfigRequest {
    pub name: String, pub provider: String, pub model_name: String,
    pub base_url: Option<String>, pub api_key: String,
    pub temperature: Option<f64>, pub max_tokens: Option<i32>,
    pub top_p: Option<f64>, pub presence_penalty: Option<f64>,
    pub frequency_penalty: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateModelConfigRequest {
    pub id: String, pub name: Option<String>, pub provider: Option<String>,
    pub model_name: Option<String>, pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub temperature: Option<Option<f64>>,
    pub max_tokens: Option<Option<i32>>,
    pub top_p: Option<Option<f64>>,
    pub presence_penalty: Option<Option<f64>>,
    pub frequency_penalty: Option<Option<f64>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeleteModelConfigRequest { pub id: String }

#[derive(Debug, Clone, Deserialize)]
pub struct TestModelConfigConnectionRequest { pub id: String }

#[derive(Debug, Clone, Serialize)]
pub struct TestApiConnectionResponse {
    pub success: bool,
    pub latency_ms: u64,
    pub message: String,
}
