use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub session_type: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_message_at: Option<i64>,
    pub last_message_preview: Option<String>,
    pub unread_count: i32,
    pub is_deleted: bool,
    pub deleted_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateSession {
    pub session_id: String,
    pub agent_id: String,
    pub message_limit: Option<i32>,
    pub message_limit_enabled: bool,
    pub agent_message_count: i32,
    pub last_reset_at: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResponse {
    pub id: String,
    pub session_type: String,
    pub last_message_at: Option<i64>,
    pub last_message_preview: Option<String>,
    pub unread_count: i32,
    pub agent_id: Option<String>,
    pub agent_name: Option<String>,
    pub agent_avatar: Option<String>,
    pub group_name: Option<String>,
    pub group_avatar: Option<String>,
    pub mute_enabled: Option<bool>,
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
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatePrivateSessionRequest {
    pub agent_id: String,
}

impl From<(Session, PrivateSession)> for SessionResponse {
    fn from((session, ps): (Session, PrivateSession)) -> Self {
        Self {
            id: session.id,
            session_type: session.session_type,
            last_message_at: session.last_message_at,
            last_message_preview: session.last_message_preview,
            unread_count: session.unread_count,
            agent_id: Some(ps.agent_id),
            agent_name: None, // populated by handler
            agent_avatar: None,
            group_name: None,
            group_avatar: None,
            mute_enabled: None,
        }
    }
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
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateSessionConfigRequest {
    pub session_id: String,
    pub history_limit: Option<i32>,
    pub message_limit: Option<i32>,
    pub message_limit_enabled: Option<bool>,
    pub mute_enabled: Option<bool>,
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
