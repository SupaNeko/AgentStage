use tauri::State;
use crate::db::connection::{get_db, DbState};
use crate::db::agent as agent_repo;
use crate::models::agent::{Agent, CreateAgentRequest, UpdateAgentRequest};

#[tauri::command]
pub fn create_agent(state: State<DbState>, req: CreateAgentRequest) -> Result<Agent, String> {
    let conn = get_db(&state)?;
    agent_repo::create(&conn, &req).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_agent(state: State<DbState>, id: String) -> Result<Option<Agent>, String> {
    let conn = get_db(&state)?;
    agent_repo::get_by_id(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_agents(state: State<DbState>) -> Result<Vec<Agent>, String> {
    let conn = get_db(&state)?;
    agent_repo::list_all(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_agent(state: State<DbState>, req: UpdateAgentRequest) -> Result<Agent, String> {
    let conn = get_db(&state)?;
    agent_repo::update(&conn, &req).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_agent(state: State<DbState>, id: String) -> Result<bool, String> {
    let conn = get_db(&state)?;
    agent_repo::soft_delete(&conn, &id).map_err(|e| e.to_string())
}
