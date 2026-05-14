use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatPage {
    pub id: String,
    pub session_id: String,
    pub page_index: i32,
    pub name: String,
    pub is_active: bool,
    pub message_count: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListChatPagesRequest {
    pub session_id: String,
}
