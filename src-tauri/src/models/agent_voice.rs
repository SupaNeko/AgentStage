use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentVoice {
    pub id: String,
    pub agent_id: String,
    pub model_name: String,
    pub model_path: String,
    pub speaker_id: Option<String>,
    pub target_language: String,
    pub emotion_params: Option<String>,
    pub speed: f64,
    pub translate_enabled: bool,
    pub translate_model_config_id: Option<String>,
    pub generation_mode: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveAgentVoiceRequest {
    pub agent_id: String,
    pub model_name: String,
    pub model_path: String,
    pub speaker_id: Option<String>,
    pub target_language: String,
    pub emotion_params: Option<String>,
    pub speed: f64,
    pub translate_enabled: bool,
    pub translate_model_config_id: Option<String>,
    pub generation_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VitsModelInfo {
    pub name: String,
    pub path: String,
    pub language: Option<String>,
    pub speakers: Vec<String>,
    pub has_config: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateVoiceRequest {
    pub message_id: String,
    pub session_id: String,
    pub agent_id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceCacheItem {
    pub id: String,
    pub message_id: String,
    pub session_id: String,
    pub agent_id: String,
    pub file_path: String,
    pub file_size: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateForTtsRequest {
    pub text: String,
    pub target_language: String,
    pub agent_persona: String,
    pub agent_relationships: String,
    pub memories: String,
    pub model_config_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateForTtsResponse {
    pub need_translate: bool,
    pub translated_text: String,
}

#[derive(Debug, Clone)]
pub struct TranslateForTtsResult {
    pub response: TranslateForTtsResponse,
    pub usage: Option<serde_json::Value>,
}
