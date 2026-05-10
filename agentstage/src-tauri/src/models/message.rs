use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub sender_type: String,
    pub sender_id: String,
    pub content: String,
    pub created_at: i64,
    pub message_type: String,
    pub tool_call_data: Option<String>,
    pub generation_info: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageResponse {
    pub id: String,
    pub session_id: String,
    pub sender_type: String,
    pub sender_id: String,
    pub sender_name: String,
    pub content: String,
    pub created_at: i64,
    pub message_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SendMessageRequest {
    pub session_id: String,
    pub content: String,
}
