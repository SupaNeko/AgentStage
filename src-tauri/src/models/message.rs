use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub session_id: String,
    pub sender_type: String,
    pub sender_id: String,
    #[serde(default)]
    pub sender_name: String,
    #[serde(default)]
    pub sender_avatar: Option<String>,
    pub content: String,
    pub created_at: i64,
    pub message_type: String,
    pub tool_call_data: Option<String>,
    pub generation_info: Option<String>,
    pub is_deleted: bool,
    #[serde(default)]
    pub page_index: i32,
}

impl From<Message> for MessageResponse {
    fn from(msg: Message) -> Self {
        Self {
            id: msg.id,
            session_id: msg.session_id,
            sender_type: msg.sender_type,
            sender_id: msg.sender_id,
            sender_name: String::new(), // populated by handler
            content: msg.content,
            created_at: msg.created_at,
            message_type: msg.message_type,
        }
    }
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
    pub page_index: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetSessionMessagesRequest {
    pub session_id: String,
    pub limit: i32,
    pub offset: i32,
    pub page_index: Option<i32>,
}
