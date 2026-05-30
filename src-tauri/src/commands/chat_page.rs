use serde::Serialize;
use tauri::State;
use crate::db::connection::{get_db, DbState};
use crate::db::chat_page_participant;
use crate::models::chat_page::UpdateChatPageNameRequest;

#[tauri::command]
pub async fn update_chat_page_name(
    state: State<'_, DbState>,
    req: UpdateChatPageNameRequest,
) -> Result<(), String> {
    let conn = get_db(&state).await?;
    crate::db::chat_page::update_name(&conn, &req.session_id, req.page_index, &req.name)
        .map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct ChatPageParticipantResponse {
    pub participant_id: String,
    pub participant_type: String,
    pub participant_name: String,
    pub participant_avatar: Option<String>,
    pub participant_simplified_persona: Option<String>,
}

#[tauri::command]
pub async fn get_chat_page_id(
    state: State<'_, DbState>,
    session_id: String,
    page_index: i32,
) -> Result<Option<String>, String> {
    let conn = get_db(&state).await?;
    chat_page_participant::get_chat_page_id(&conn, &session_id, page_index)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_chat_page_participants(
    state: State<'_, DbState>,
    chat_page_id: String,
) -> Result<Vec<ChatPageParticipantResponse>, String> {
    let conn = get_db(&state).await?;
    let participants = chat_page_participant::list_by_chat_page(&conn, &chat_page_id)
        .map_err(|e| e.to_string())?;
    Ok(participants.into_iter().map(|p| ChatPageParticipantResponse {
        participant_id: p.participant_id,
        participant_type: p.participant_type,
        participant_name: p.participant_name,
        participant_avatar: p.participant_avatar,
        participant_simplified_persona: p.participant_simplified_persona,
    }).collect())
}
