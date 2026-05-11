use tauri::State;
use crate::db::connection::{get_db, DbState};
use crate::db::session as session_repo;
use crate::models::session::{CreateGroupSessionRequest, CreatePrivateSessionRequest, GroupMemberResponse, SessionResponse};

#[tauri::command]
pub async fn create_private_session(
    state: State<'_, DbState>,
    req: CreatePrivateSessionRequest,
) -> Result<SessionResponse, String> {
    crate::logger::backend("DEBUG", &format!("[DEBUG create_private_session] agent_id={}", req.agent_id));

    let conn = get_db(&state).await?;
    let session = session_repo::create_private_session(&conn, &req.agent_id)
        .map_err(|e| e.to_string())?;

    crate::logger::backend("DEBUG", &format!("[DEBUG create_private_session] returned session_id={}", session.id));
    Ok(session)
}

#[tauri::command]
pub async fn list_sessions(state: State<'_, DbState>) -> Result<Vec<SessionResponse>, String> {
    let conn = get_db(&state).await?;
    let sessions = session_repo::list_sessions(&conn).map_err(|e| e.to_string())?;

    crate::logger::backend("DEBUG", &format!("[DEBUG list_sessions] returned {} sessions", sessions.len()));
    Ok(sessions)
}

#[tauri::command]
pub async fn get_session(
    state: State<'_, DbState>,
    id: String,
) -> Result<Option<SessionResponse>, String> {
    crate::logger::backend("DEBUG", &format!("[DEBUG get_session] id={}", id));

    let conn = get_db(&state).await?;
    let result = session_repo::get_session_by_id(&conn, &id).map_err(|e| e.to_string())?;

    crate::logger::backend("DEBUG", &format!("[DEBUG get_session] id={}, found={}", id, result.is_some()));
    Ok(result)
}

#[tauri::command]
pub async fn delete_session(state: State<'_, DbState>, id: String) -> Result<bool, String> {
    crate::logger::backend("DEBUG", &format!("[DEBUG delete_session] id={}", id));

    let conn = get_db(&state).await?;
    let rows_affected = session_repo::soft_delete_session(&conn, &id).map_err(|e| e.to_string())?;

    crate::logger::backend("DEBUG", &format!("[DEBUG delete_session] id={}, rows_affected={}", id, rows_affected));
    Ok(rows_affected)
}

#[tauri::command]
pub async fn create_group_session(
    state: State<'_, DbState>,
    req: CreateGroupSessionRequest,
) -> Result<SessionResponse, String> {
    crate::logger::backend("DEBUG", &format!("[DEBUG create_group_session] name={}, agents={:?}", req.name, req.agent_ids));
    let conn = get_db(&state).await?;
    let session = session_repo::create_group_session(&conn, &req.name, &req.agent_ids)
        .map_err(|e| e.to_string())?;
    Ok(session)
}

#[tauri::command]
pub async fn get_group_members(
    state: State<'_, DbState>,
    session_id: String,
) -> Result<Vec<GroupMemberResponse>, String> {
    let conn = get_db(&state).await?;
    let members = session_repo::get_group_members(&conn, &session_id)
        .map_err(|e| e.to_string())?;
    Ok(members)
}
