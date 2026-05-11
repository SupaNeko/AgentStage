#[tauri::command]
pub async fn log_frontend(level: String, message: String) {
    crate::logger::frontend(&level, &message);
}
