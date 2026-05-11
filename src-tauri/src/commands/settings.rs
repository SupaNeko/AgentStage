use tauri::State;
use crate::db::connection::{get_db, DbState};
use crate::db::settings as settings_repo;
use crate::models::settings::{SettingsResponse, UpdateAppSettingsRequest};

#[tauri::command]
pub async fn get_settings(state: State<'_, DbState>) -> Result<SettingsResponse, String> {
    let conn = get_db(&state).await?;
    let settings = settings_repo::get_or_create_settings(&conn)
        .map_err(|e| e.to_string())?;
    Ok(settings.into())
}

#[tauri::command]
pub async fn update_settings(
    state: State<'_, DbState>,
    req: UpdateAppSettingsRequest,
) -> Result<SettingsResponse, String> {
    let conn = get_db(&state).await?;
    settings_repo::update_settings(&conn, &req)
        .map_err(|e| e.to_string())?;
    let settings = settings_repo::get_or_create_settings(&conn)
        .map_err(|e| e.to_string())?;
    Ok(settings.into())
}
