use tauri::State;
use crate::db::connection::{get_db, DbState};
use crate::db::message as message_repo;
use crate::db::session as session_repo;
use crate::models::message::{Message, SendMessageRequest};
use crate::scheduler::Scheduler;

#[tauri::command]
pub async fn send_user_message(
    state: State<'_, DbState>,
    scheduler: State<'_, Scheduler>,
    req: SendMessageRequest,
) -> Result<Message, String> {
    let conn = get_db(&state).await?;

    let message = message_repo::insert_message(
        &conn,
        &req.session_id,
        "user",
        "user",
        &req.content,
        "text",
    ).map_err(|e| e.to_string())?;

    // 更新会话最后消息预览
    let preview = if req.content.len() > 100 {
        format!("{}...", &req.content[..100])
    } else {
        req.content.clone()
    };
    let _ = session_repo::update_session_last_message(&conn, &req.session_id, &preview);

    drop(conn);

    // 触发调度器
    scheduler.on_new_message(&req.session_id, &message).await?;

    Ok(message)
}

#[tauri::command]
pub async fn get_session_messages(
    state: State<'_, DbState>,
    session_id: String,
    limit: i32,
    offset: i32,
) -> Result<Vec<Message>, String> {
    let conn = get_db(&state).await?;
    message_repo::get_messages_by_session(&conn, &session_id, limit, offset)
        .map_err(|e| e.to_string())
}
