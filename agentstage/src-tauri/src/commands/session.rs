use tauri::State;
use crate::db::connection::{get_db, DbState};
use crate::db::session as session_repo;
use crate::models::session::{CreatePrivateSessionRequest, SessionResponse};

#[tauri::command]
pub async fn create_private_session(
    state: State<'_, DbState>,
    req: CreatePrivateSessionRequest,
) -> Result<SessionResponse, String> {
    let conn = get_db(&state).await?;
    session_repo::create_private_session(&conn, &req.agent_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_sessions(state: State<'_, DbState>) -> Result<Vec<SessionResponse>, String> {
    let conn = get_db(&state).await?;
    session_repo::list_sessions(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_session(
    state: State<'_, DbState>,
    id: String,
) -> Result<Option<SessionResponse>, String> {
    let conn = get_db(&state).await?;
    session_repo::get_session_by_id(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_session(state: State<'_, DbState>, id: String) -> Result<bool, String> {
    let conn = get_db(&state).await?;
    session_repo::soft_delete_session(&conn, &id).map_err(|e| e.to_string())
}
