use tauri::State;
use crate::db::connection::{get_db, DbState};
use crate::models::chat_page::UpdateChatPageNameRequest;

#[tauri::command]
pub async fn update_chat_page_name(
    state: State<'_, DbState>,
    req: UpdateChatPageNameRequest,
) -> Result<(), String> {
    let conn = get_db(&state).await?;
    crate::db::chat_page::update_name(&conn, &req.session_id, req.page_index, &req.name)
        .map_err(|e| e.to_string())
}
