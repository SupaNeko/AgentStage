use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub session_type: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_message_at: Option<i64>,
    pub unread_count: i32,
    pub is_deleted: bool,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateSession {
    pub session_id: String,
    pub participant_1_type: String,
    pub participant_1_id: String,
    pub participant_2_type: String,
    pub participant_2_id: String,
    pub message_limit: Option<i32>,
    pub message_limit_enabled: bool,
    pub agent_message_count: i32,
    pub last_reset_at: i64,
    pub current_chat_page: i32,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionParticipant {
    pub participant_type: String,
    pub participant_id: String,
    pub name: String,
    pub avatar_path: Option<String>,
    pub is_deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResponse {
    pub id: String,
    pub session_type: String,
    pub last_message_at: Option<i64>,
    pub unread_count: i32,
    pub participants: Vec<SessionParticipant>,
    pub group_name: Option<String>,
    pub group_avatar: Option<String>,
    pub mute_enabled: Option<bool>,
    pub current_chat_page: i32,
    pub is_dissolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupSession {
    pub session_id: String,
    pub name: String,
    pub avatar_path: Option<String>,
    pub mute_enabled: bool,
    pub message_limit: Option<i32>,
    pub message_limit_enabled: bool,
    pub agent_message_count: i32,
    pub last_reset_at: i64,
    pub created_at: i64,
    pub is_dissolved: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatePrivateSessionRequest {
    pub agent_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateGroupSessionRequest {
    pub name: String,
    pub agent_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupMemberResponse {
    pub participant_type: String,
    pub participant_id: String,
    pub name: String,
    pub avatar_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub session_id: String,
    pub history_limit: i32,
    pub message_limit: i32,
    pub message_limit_enabled: bool,
    pub mute_enabled: bool,
    pub agent_message_count: i32,
    pub overflow_summary_threshold: Option<i32>,
    pub last_overflow_summary_index: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateSessionConfigRequest {
    pub session_id: String,
    pub history_limit: Option<i32>,
    pub message_limit: Option<i32>,
    pub message_limit_enabled: Option<bool>,
    pub mute_enabled: Option<bool>,
    pub overflow_summary_threshold: Option<i32>,
    pub last_overflow_summary_index: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResetSessionRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AddGroupMemberRequest {
    pub session_id: String,
    pub agent_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RemoveGroupMemberRequest {
    pub session_id: String,
    pub agent_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetSessionConfigRequest {
    pub session_id: String,
    pub session_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResetMessageCountRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DisbandGroupRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClearSessionHistoryRequest {
    pub session_id: String,
}
