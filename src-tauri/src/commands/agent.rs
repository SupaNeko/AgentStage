use tauri::State;
use crate::db::connection::{get_db, DbState};
use crate::db::agent as agent_repo;
use crate::models::agent::{AgentResponse, CreateAgentRequest, UpdateAgentRequest};

#[tauri::command]
pub async fn create_agent(state: State<'_, DbState>, req: CreateAgentRequest) -> Result<AgentResponse, String> {
    crate::logger::backend("DEBUG", &format!("[DEBUG create_agent] name={}", req.name));

    let conn = get_db(&state).await?;
    let agent = agent_repo::create(&conn, &req).map_err(|e| e.to_string())?;
    Ok(AgentResponse::from(agent))
}

#[tauri::command]
pub async fn get_agent(state: State<'_, DbState>, id: String) -> Result<Option<AgentResponse>, String> {
    crate::logger::backend("DEBUG", &format!("[DEBUG get_agent] id={}", id));

    let conn = get_db(&state).await?;
    let agent = agent_repo::get_by_id(&conn, &id).map_err(|e| e.to_string())?;
    Ok(agent.map(AgentResponse::from))
}

#[tauri::command]
pub async fn list_agents(state: State<'_, DbState>) -> Result<Vec<AgentResponse>, String> {
    let conn = get_db(&state).await?;
    let agents = agent_repo::list_all(&conn).map_err(|e| e.to_string())?;

    crate::logger::backend("DEBUG", &format!("[DEBUG list_agents] returned {} agents", agents.len()));
    Ok(agents.into_iter().map(AgentResponse::from).collect())
}

#[tauri::command]
pub async fn update_agent(state: State<'_, DbState>, req: UpdateAgentRequest) -> Result<AgentResponse, String> {
    crate::logger::backend("DEBUG", &format!("[DEBUG update_agent] id={}", req.id));

    let conn = get_db(&state).await?;
    let agent = agent_repo::update(&conn, &req).map_err(|e| e.to_string())?;
    Ok(AgentResponse::from(agent))
}

#[tauri::command]
pub async fn delete_agent(state: State<'_, DbState>, id: String) -> Result<bool, String> {
    crate::logger::backend("DEBUG", &format!("[DEBUG delete_agent] id={}", id));

    let conn = get_db(&state).await?;
    agent_repo::soft_delete(&conn, &id).map_err(|e| e.to_string())
}
