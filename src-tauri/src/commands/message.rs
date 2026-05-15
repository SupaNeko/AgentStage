use tauri::State;
use crate::db::connection::{get_db, DbState};
use crate::db::message as message_repo;
use crate::db::session as session_repo;
use crate::models::message::{Message, SendMessageRequest, GetSessionMessagesRequest, SendHistoryMessageRequest};
use crate::llm::provider::LlmProvider;
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

/// 确定历史会话模式下的目标回复 Agent
/// - 私聊：返回对方 Agent ID
/// - 群聊：返回所有 Agent 成员 ID
fn resolve_history_target_agents(conn: &rusqlite::Connection, session_id: &str) -> Result<Vec<String>, String> {
    let session_type: String = conn.query_row(
        "SELECT session_type FROM sessions WHERE id = ?1 AND is_deleted = 0",
        [session_id],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;

    let mut agents = Vec::new();
    if session_type == "private" {
        let agent_id: String = conn.query_row(
            "SELECT agent_id FROM private_sessions WHERE session_id = ?1",
            [session_id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        agents.push(agent_id);
    } else {
        let mut stmt = conn.prepare(
            "SELECT participant_id FROM group_members WHERE session_id = ?1 AND participant_type = 'agent'"
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map([session_id], |row| {
            row.get::<_, String>(0)
        }).map_err(|e| e.to_string())?;
        for row in rows {
            agents.push(row.map_err(|e| e.to_string())?);
        }
    }
    Ok(agents)
}

#[tauri::command]
pub async fn send_history_message(
    state: State<'_, DbState>,
    req: SendHistoryMessageRequest,
) -> Result<Vec<Message>, String> {
    crate::logger::backend("DEBUG", &format!(
        "[DEBUG send_history_message] START session_id={}, page_index={}, content_len={}",
        req.session_id, req.page_index, req.content.len()
    ));

    let conn = get_db(&state).await?;

    // 1. 插入用户消息到指定 page
    let user_msg = message_repo::insert_message(
        &conn, &req.session_id, "user", "user", &req.content, "text", Some(req.page_index),
    ).map_err(|e| e.to_string())?;

    // 2. 更新会话最后消息预览
    let preview = crate::scheduler::truncate_preview(&req.content, 100);
    let _ = session_repo::update_session_last_message(&conn, &req.session_id, &preview);

    // 3. 查询该 page 的所有历史消息作为上下文
    let history_msgs = message_repo::get_messages_by_session(&conn, &req.session_id, req.page_index, 1000, 0)
        .map_err(|e| e.to_string())?;

    // 4. 确定目标 Agents
    let target_agents = resolve_history_target_agents(&conn, &req.session_id)?;

    // 5. 为每个 Agent 调用 LLM 并收集回复
    let mut all_messages = vec![user_msg.clone()];
    for agent_id in target_agents {
        let prompt = match crate::llm::history_prompt::HistoryPromptAssembler::assemble(
            &conn, &agent_id, &req.session_id, req.page_index, &history_msgs
        ) {
            Ok(p) => p,
            Err(e) => {
                crate::logger::backend("ERROR", &format!(
                    "[DEBUG send_history_message] Failed to assemble prompt for agent {}: {}", agent_id, e
                ));
                continue;
            }
        };

        // 获取 Agent 配置
        let agent = match crate::db::agent::get_by_id(&conn, &agent_id) {
            Ok(Some(a)) => a,
            Ok(None) => {
                crate::logger::backend("WARN", &format!(
                    "[DEBUG send_history_message] Agent {} not found", agent_id
                ));
                continue;
            }
            Err(e) => {
                crate::logger::backend("ERROR", &format!(
                    "[DEBUG send_history_message] DB error for agent {}: {}", agent_id, e
                ));
                continue;
            }
        };

        let api_key = match agent.api_key_encrypted {
            Some(enc) => match crate::crypto::decrypt(&enc) {
                Ok(k) => k,
                Err(e) => {
                    crate::logger::backend("ERROR", &format!(
                        "[DEBUG send_history_message] Failed to decrypt API key for agent {}: {}", agent_id, e
                    ));
                    continue;
                }
            },
            None => {
                crate::logger::backend("WARN", &format!(
                    "[DEBUG send_history_message] Agent {} has no API key", agent_id
                ));
                continue;
            }
        };

        let provider = crate::llm::openai::OpenAiCompatibleProvider::new(
            api_key,
            agent.base_url,
            agent.model_name.unwrap_or_else(|| "gpt-4o".to_string()),
            agent.temperature,
            agent.max_tokens,
        );

        // 调用 LLM（History 模式也要求使用 send_message 工具回复，避免模型输出自由文本携带 think 标签）
        let tools = vec![crate::llm::tool::send_message_tool_schema()];
        let llm_messages = vec![serde_json::json!({
            "role": "user",
            "content": &prompt
        })];

        match provider.chat("", llm_messages, tools).await {
            Ok(resp) => {
                // 优先解析 tool_calls，提取 content
                let mut content_extracted = false;
                for tc in &resp.tool_calls {
                    if tc.name == "send_message" {
                        if let Ok(args) = serde_json::from_str::<serde_json::Value>(&tc.arguments) {
                            let content = args["content"].as_str().unwrap_or("").trim();
                            if !content.is_empty() {
                                let contents = crate::llm::tool::split_br_tags(content);
                                for c in &contents {
                                    let agent_msg = message_repo::insert_message(
                                        &conn, &req.session_id, "agent", &agent_id, c, "text", Some(req.page_index),
                                    ).map_err(|e| e.to_string())?;
                                    all_messages.push(agent_msg);
                                }
                                content_extracted = true;
                                break; // 只处理第一个有效的 send_message
                            }
                        } else {
                            crate::logger::backend("WARN", &format!(
                                "[DEBUG send_history_message] Failed to parse tool call arguments for agent {}: {}", agent_id, tc.arguments
                            ));
                        }
                    }
                }
                // fallback: 如果模型没有使用工具，直接使用 content
                if !content_extracted {
                    if let Some(content) = resp.content {
                        let trimmed = content.trim();
                        if !trimmed.is_empty() {
                            let contents = crate::llm::tool::split_br_tags(&trimmed);
                            for c in &contents {
                                let agent_msg = message_repo::insert_message(
                                    &conn, &req.session_id, "agent", &agent_id, c, "text", Some(req.page_index),
                                ).map_err(|e| e.to_string())?;
                                all_messages.push(agent_msg);
                            }
                        }
                    }
                }
            }
            Err(e) => {
                crate::logger::backend("ERROR", &format!(
                    "[DEBUG send_history_message] LLM call failed for agent {}: {}", agent_id, e
                ));
            }
        }
    }

    crate::logger::backend("DEBUG", &format!(
        "[DEBUG send_history_message] END session_id={}, returned {} messages",
        req.session_id, all_messages.len()
    ));

    Ok(all_messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn init_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V1).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V2).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V3).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V4).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V5).unwrap();
        conn
    }

    #[test]
    fn test_resolve_history_target_agents_private() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES ('a1', 'Agent1', '', '', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO sessions (id, session_type, created_at, updated_at) VALUES ('s1', 'private', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO private_sessions (session_id, agent_id, created_at, current_chat_page) VALUES ('s1', 'a1', 0, 0)",
            [],
        ).unwrap();

        let agents = resolve_history_target_agents(&conn, "s1").unwrap();
        assert_eq!(agents, vec!["a1"]);
    }

    #[test]
    fn test_resolve_history_target_agents_group() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES ('a1', 'Agent1', '', '', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES ('a2', 'Agent2', '', '', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO sessions (id, session_type, created_at, updated_at) VALUES ('s1', 'group', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO group_sessions (session_id, name, created_at, current_chat_page) VALUES ('s1', 'Group1', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO group_members (session_id, participant_id, participant_type, created_at) VALUES ('s1', 'a1', 'agent', 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO group_members (session_id, participant_id, participant_type, created_at) VALUES ('s1', 'a2', 'agent', 0)",
            [],
        ).unwrap();

        let mut agents = resolve_history_target_agents(&conn, "s1").unwrap();
        agents.sort();
        assert_eq!(agents, vec!["a1", "a2"]);
    }

    #[test]
    fn test_resolve_history_target_agents_deleted_session() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES ('a1', 'Agent1', '', '', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO sessions (id, session_type, created_at, updated_at, is_deleted) VALUES ('s1', 'private', 0, 0, 1)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO private_sessions (session_id, agent_id, created_at, current_chat_page) VALUES ('s1', 'a1', 0, 0)",
            [],
        ).unwrap();

        let result = resolve_history_target_agents(&conn, "s1");
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_history_target_agents_group_excludes_non_agents() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES ('a1', 'Agent1', '', '', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO sessions (id, session_type, created_at, updated_at) VALUES ('s1', 'group', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO group_sessions (session_id, name, created_at, current_chat_page) VALUES ('s1', 'Group1', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO group_members (session_id, participant_id, participant_type, created_at) VALUES ('s1', 'a1', 'agent', 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO group_members (session_id, participant_id, participant_type, created_at) VALUES ('s1', 'user', 'user', 0)",
            [],
        ).unwrap();

        let agents = resolve_history_target_agents(&conn, "s1").unwrap();
        assert_eq!(agents, vec!["a1"]);
    }
}
