use tauri::State;
use crate::db::connection::{get_db, DbState};
use crate::db::message as message_repo;
use crate::db::session as session_repo;
use crate::models::message::{Message, SendMessageRequest, GetSessionMessagesRequest};
use crate::scheduler::Scheduler;

#[tauri::command]
pub async fn send_user_message(
    state: State<'_, DbState>,
    scheduler: State<'_, Scheduler>,
    req: SendMessageRequest,
) -> Result<Message, String> {
    crate::logger::backend("DEBUG", &format!("[DEBUG send_user_message] START session_id={}, content_len={}", req.session_id, req.content.len()));

    let conn = get_db(&state).await?;

    let page_index = match req.page_index {
        Some(p) => p,
        None => {
            conn.query_row(
                "SELECT COALESCE(current_chat_page, 0) FROM private_sessions WHERE session_id = ?1
                 UNION ALL
                 SELECT COALESCE(current_chat_page, 0) FROM group_sessions WHERE session_id = ?1
                 LIMIT 1",
                [&req.session_id],
                |row| row.get(0),
            ).unwrap_or(0)
        }
    };

    let message = message_repo::insert_message(
        &conn,
        &req.session_id,
        "user",
        "user",
        &req.content,
        "text",
        Some(page_index),
    ).map_err(|e| e.to_string())?;

    crate::logger::backend("DEBUG", &format!("[DEBUG send_user_message] insert_message succeeded, message_id={}, page_index={}", message.id, message.page_index));

    // 更新会话最后消息预览（按字符截断，防止 UTF-8 切片 panic）
    let preview = crate::scheduler::truncate_preview(&req.content, 100);
    let _ = session_repo::update_session_last_message(&conn, &req.session_id, &preview);

    drop(conn);

    // 触发调度器（错误不传播到前端，调度器在后台处理）
    crate::logger::backend("DEBUG", &format!("[DEBUG send_user_message] calling scheduler.on_new_message for session_id={}", req.session_id));
    let scheduler_result = scheduler.on_new_message(&req.session_id, &message).await;
    match &scheduler_result {
        Ok(_) => crate::logger::backend("DEBUG", "[DEBUG send_user_message] scheduler.on_new_message completed OK"),
        Err(e) => crate::logger::backend("WARN", &format!(
            "[DEBUG send_user_message] scheduler error (non-fatal): {}", e
        )),
    }

    crate::logger::backend("DEBUG", &format!("[DEBUG send_user_message] END session_id={}, message_id={}", req.session_id, message.id));

    Ok(message)
}

#[tauri::command]
pub async fn get_session_messages(
    state: State<'_, DbState>,
    req: GetSessionMessagesRequest,
) -> Result<Vec<Message>, String> {
    println!("[DEBUG get_session_messages] session_id={}, limit={}, offset={}", req.session_id, req.limit, req.offset);

    let conn = get_db(&state).await?;

    let page_index = match req.page_index {
        Some(p) => p,
        None => {
            conn.query_row(
                "SELECT COALESCE(current_chat_page, 0) FROM private_sessions WHERE session_id = ?1
                 UNION ALL
                 SELECT COALESCE(current_chat_page, 0) FROM group_sessions WHERE session_id = ?1
                 LIMIT 1",
                [&req.session_id],
                |row| row.get(0),
            ).unwrap_or(0)
        }
    };

    let messages = message_repo::get_messages_by_session(&conn, &req.session_id, page_index, req.limit, req.offset)
        .map_err(|e| e.to_string())?;

    println!("[DEBUG get_session_messages] returned {} messages (page_index={})", messages.len(), page_index);
    Ok(messages)
}
