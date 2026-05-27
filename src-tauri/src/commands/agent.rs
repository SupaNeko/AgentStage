use tauri::State;
use crate::db::connection::{get_db, DbState};
use crate::db::agent as agent_repo;
use crate::db::agent_relationship;
use crate::db::user_persona as user_persona_repo;
use crate::models::agent::{AgentResponse, CreateAgentRequest, UpdateAgentRequest, DeleteAgentRequest};

#[tauri::command]
pub async fn create_agent(state: State<'_, DbState>, req: CreateAgentRequest) -> Result<AgentResponse, String> {
    crate::logger::debug(&format!("[DEBUG create_agent] name={}", req.name));

    let conn = get_db(&state).await?;
    if let Ok(Some(_)) = agent_repo::get_agent_by_name(&conn, &req.name) {
        return Err(format!("已存在同名角色 '{}'，请使用其他名称", req.name));
    }
    if let Ok(Some(_)) = user_persona_repo::get_user_persona_by_name(&conn, &req.name) {
        return Err(format!("该名称已被用户人设 '{}' 使用，请使用其他名称", req.name));
    }
    let agent = agent_repo::create(&conn, &req).map_err(|e| e.to_string())?;
    Ok(AgentResponse::from(agent))
}

#[tauri::command]
pub async fn get_agent(state: State<'_, DbState>, id: String) -> Result<Option<AgentResponse>, String> {
    crate::logger::debug(&format!("[DEBUG get_agent] id={}", id));

    let conn = get_db(&state).await?;
    let agent = agent_repo::get_by_id_with_model_name(&conn, &id).map_err(|e| e.to_string())?;
    if let Some(ref a) = agent {
        crate::logger::debug(&format!(
            "[DEBUG get_agent] id={}, proactive_enabled={}, min={}, max={}",
            a.id, a.proactive_enabled, a.proactive_min_minutes, a.proactive_max_minutes
        ));
    }
    Ok(agent)
}

#[tauri::command]
pub async fn list_agents(state: State<'_, DbState>) -> Result<Vec<AgentResponse>, String> {
    let conn = get_db(&state).await?;
    let agents = agent_repo::list_all_with_model_name(&conn).map_err(|e| e.to_string())?;

    crate::logger::debug(&format!("[DEBUG list_agents] returned {} agents", agents.len()));
    Ok(agents)
}

#[tauri::command]
pub async fn update_agent(state: State<'_, DbState>, req: UpdateAgentRequest) -> Result<AgentResponse, String> {
    crate::logger::debug(&format!("[DEBUG update_agent] id={}", req.id));

    let conn = get_db(&state).await?;
    if let Some(ref name) = req.name {
        if let Ok(Some(existing)) = agent_repo::get_agent_by_name(&conn, name) {
            if existing.id != req.id {
                return Err(format!("已存在同名角色 '{}'，请使用其他名称", name));
            }
        }
        if let Ok(Some(_)) = user_persona_repo::get_user_persona_by_name(&conn, name) {
            return Err(format!("该名称已被用户人设 '{}' 使用，请使用其他名称", name));
        }
    }
    let agent = agent_repo::update(&conn, &req).map_err(|e| e.to_string())?;
    Ok(AgentResponse::from(agent))
}

#[tauri::command]
pub async fn delete_agent(state: State<'_, DbState>, req: DeleteAgentRequest) -> Result<bool, String> {
    crate::logger::debug(&format!("[DEBUG delete_agent] id={}", req.id));

    let conn = get_db(&state).await?;
    agent_repo::soft_delete(&conn, &req.id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reset_agent_memory(
    state: State<'_, DbState>,
    agent_id: String,
) -> Result<(), String> {
    crate::logger::debug(&format!("[DEBUG reset_agent_memory] agent_id={}", agent_id));

    let conn = get_db(&state).await?;
    
    agent_repo::clear_long_term_memory(&conn, &agent_id)
        .map_err(|e| e.to_string())?;
    
    agent_relationship::clear_memories_by_observer(&conn, &agent_id)
        .map_err(|e| e.to_string())?;

    crate::logger::debug(&format!("[DEBUG reset_agent_memory] success agent_id={}", agent_id));
    Ok(())
}
