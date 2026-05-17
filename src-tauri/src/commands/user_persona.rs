use tauri::State;
use crate::db::connection::{get_db, DbState};
use crate::db::user_persona;
use crate::models::user_persona::{UserPersona, CreateUserPersonaRequest, UpdateUserPersonaRequest, CurrentUserPersonaResponse};

#[tauri::command]
pub async fn list_user_personas(state: State<'_, DbState>) -> Result<Vec<UserPersona>, String> {
    let conn = get_db(&state).await.map_err(|e| e.to_string())?;
    user_persona::list_user_personas(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_user_persona(state: State<'_, DbState>, req: CreateUserPersonaRequest) -> Result<UserPersona, String> {
    let conn = get_db(&state).await.map_err(|e| e.to_string())?;
    user_persona::create_user_persona(&conn, &req).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_user_persona(state: State<'_, DbState>, req: UpdateUserPersonaRequest) -> Result<UserPersona, String> {
    let conn = get_db(&state).await.map_err(|e| e.to_string())?;
    user_persona::update_user_persona(&conn, &req).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_user_persona(state: State<'_, DbState>, id: String) -> Result<(), String> {
    let conn = get_db(&state).await.map_err(|e| e.to_string())?;
    user_persona::delete_user_persona(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_current_user_persona(state: State<'_, DbState>) -> Result<CurrentUserPersonaResponse, String> {
    let conn = get_db(&state).await.map_err(|e| e.to_string())?;
    user_persona::get_current_user_persona(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn activate_user_persona(state: State<'_, DbState>, id: Option<String>) -> Result<(), String> {
    let conn = get_db(&state).await.map_err(|e| e.to_string())?;
    user_persona::activate_user_persona(&conn, id.as_deref()).map_err(|e| e.to_string())
}
