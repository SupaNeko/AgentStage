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
    crate::logger::backend("DEBUG", &format!("[DEBUG send_user_message] session_id={}, content_len={}", req.session_id, req.content.len()));

    let conn = get_db(&state).await?;

    let message = message_repo::insert_message(
        &conn,
        &req.session_id,
        "user",
        "user",
        &req.content,
        "text",
    ).map_err(|e| e.to_string())?;

    crate::logger::backend("DEBUG", &format!("[DEBUG send_user_message] insert_message succeeded, message_id={}", message.id));

    // 更新会话最后消息预览（按字符截断，防止 UTF-8 切片 panic）
    let preview = crate::scheduler::truncate_preview(&req.content, 100);
    let _ = session_repo::update_session_last_message(&conn, &req.session_id, &preview);

    drop(conn);

    // 触发调度器（错误不传播到前端，调度器在后台处理）
    crate::logger::backend("DEBUG", &format!("[DEBUG send_user_message] calling scheduler.on_new_message for session_id={}", req.session_id));
    let scheduler_result = scheduler.on_new_message(&req.session_id, &message).await;
    if let Err(e) = scheduler_result {
        crate::logger::backend("WARN", &format!(
            "[DEBUG send_user_message] scheduler error (non-fatal): {}", e
        ));
    }
    crate::logger::backend("DEBUG", &format!("[DEBUG send_user_message] scheduler.on_new_message completed for session_id={}", req.session_id));

    Ok(message)
}

#[tauri::command]
pub async fn get_session_messages(
    state: State<'_, DbState>,
    session_id: String,
    limit: i32,
    offset: i32,
) -> Result<Vec<Message>, String> {
    println!("[DEBUG get_session_messages] session_id={}, limit={}, offset={}", session_id, limit, offset);

    let conn = get_db(&state).await?;
    let messages = message_repo::get_messages_by_session(&conn, &session_id, 0, limit, offset)
        .map_err(|e| e.to_string())?;

    println!("[DEBUG get_session_messages] returned {} messages", messages.len());
    Ok(messages)
}
