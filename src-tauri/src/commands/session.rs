use tauri::State;
use crate::db::connection::{get_db, DbState};
use crate::db::session as session_repo;
use crate::db::frozen_state as frozen_state_repo;
use crate::db::agent_unread as agent_unread_repo;
use crate::models::session::{CreateGroupSessionRequest, CreatePrivateSessionRequest, GroupMemberResponse, SessionResponse};
use crate::scheduler::Scheduler;

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

#[tauri::command]
pub async fn get_session_config(
    state: State<'_, DbState>,
    session_id: String,
    session_type: String,
) -> Result<crate::models::session::SessionConfig, String> {
    let conn = get_db(&state).await?;
    session_repo::get_session_config(&conn, &session_id, &session_type)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_session_config(
    state: State<'_, DbState>,
    req: crate::models::session::UpdateSessionConfigRequest,
) -> Result<(), String> {
    let conn = get_db(&state).await?;
    session_repo::update_session_config(&conn, &req)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reset_session(
    state: State<'_, DbState>,
    req: crate::models::session::ResetSessionRequest,
) -> Result<String, String> {
    let conn = get_db(&state).await?;
    session_repo::reset_session(&conn, &req.session_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reset_message_count(
    state: State<'_, DbState>,
    scheduler: State<'_, Scheduler>,
    session_id: String,
) -> Result<(), String> {
    let conn = get_db(&state).await?;

    // 1. 重置计数器
    session_repo::reset_message_count(&conn, &session_id)
        .map_err(|e| e.to_string())?;

    // 2. 解除冻结
    let _ = frozen_state_repo::remove_frozen(&conn, &session_id);
    scheduler.unfreeze_session(&session_id).await;

    // 3. 触发有未读消息的 agents
    let agents_with_unread = agent_unread_repo::get_agents_with_unread(&conn, &session_id)
        .map_err(|e| e.to_string())?;

    drop(conn);

    for agent_id in agents_with_unread {
        let _ = scheduler.try_trigger_agent(&agent_id).await;
    }

    Ok(())
}

#[tauri::command]
pub async fn disband_group(
    state: State<'_, DbState>,
    session_id: String,
) -> Result<bool, String> {
    let conn = get_db(&state).await?;
    session_repo::disband_group(&conn, &session_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_group_member(
    state: State<'_, DbState>,
    req: crate::models::session::AddGroupMemberRequest,
) -> Result<(), String> {
    let conn = get_db(&state).await?;
    session_repo::add_group_member(&conn, &req.session_id, &req.agent_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_group_member(
    state: State<'_, DbState>,
    req: crate::models::session::RemoveGroupMemberRequest,
) -> Result<bool, String> {
    let conn = get_db(&state).await?;
    session_repo::remove_group_member(&conn, &req.session_id, &req.agent_id)
        .map_err(|e| e.to_string())
}
