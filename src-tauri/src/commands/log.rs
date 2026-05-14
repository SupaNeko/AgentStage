#[derive(Debug, Clone, serde::Deserialize)]
pub struct LogFrontendRequest {
    pub level: String,
    pub message: String,
}

#[tauri::command]
pub async fn log_frontend(req: LogFrontendRequest) {
    crate::logger::frontend(&req.level, &req.message);
}
