use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{AppHandle, Emitter};
use serde::Serialize;

use crate::db::agent as agent_repo;
use crate::db::session as session_repo;
use crate::db::settings as settings_repo;
use crate::db::trigger_state as trigger_repo;
use crate::db::connection::DbState;
use crate::llm::openai::OpenAiCompatibleProvider;
use crate::llm::provider::LlmProvider;
use crate::llm::prompt::PromptAssembler;
use crate::llm::tool::{send_message_tool_schema, LlmResponse, ToolExecutor};
use crate::models::message::Message;

#[derive(Clone)]
pub struct PendingMessage {
    pub session_id: String,
    pub sender_type: String,
    pub sender_id: String,
    pub content: String,
    pub created_at: i64,
}

impl From<Message> for PendingMessage {
    fn from(msg: Message) -> Self {
        Self {
            session_id: msg.session_id,
            sender_type: msg.sender_type,
            sender_id: msg.sender_id,
            content: msg.content,
            created_at: msg.created_at,
        }
    }
}

#[derive(Clone)]
pub struct Scheduler {
    pending_queue: Arc<Mutex<HashMap<String, Vec<PendingMessage>>>>,
    app_handle: Arc<std::sync::Mutex<Option<AppHandle>>>,
    db_state: DbState,
}

impl Scheduler {
    pub fn new(db_state: DbState) -> Self {
        Self {
            pending_queue: Arc::new(Mutex::new(HashMap::new())),
            app_handle: Arc::new(std::sync::Mutex::new(None)),
            db_state,
        }
    }

    pub fn set_app_handle(&self, handle: AppHandle) {
        *self.app_handle.lock().unwrap() = Some(handle);
    }

    fn emit(&self, event: &str, payload: impl Serialize + Clone) {
        if let Some(handle) = self.app_handle.lock().unwrap().as_ref() {
            let _ = handle.emit(event, payload);
        }
    }

    /// 当有新消息到达时调用（用户发送消息或角色发送消息）
    pub async fn on_new_message(
        &self,
        session_id: &str,
        message: &Message,
    ) -> Result<(), String> {
        crate::logger::backend("DEBUG", &format!(
            "[DEBUG on_new_message] session_id={}, sender_type={}",
            session_id, message.sender_type
        ));

        let conn = self.db_state.0.lock().await;

        // 获取会话类型
        let session_type: String = conn
            .query_row(
                "SELECT session_type FROM sessions WHERE id = ?1",
                [session_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        if session_type != "private" {
            return Ok(()); // Phase 3: 群聊
        }

        // 获取私聊的 agent_id
        let agent_id: String = conn
            .query_row(
                "SELECT agent_id FROM private_sessions WHERE session_id = ?1",
                [session_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        // 检查该会话是否达到消息上限
        let (count, limit, enabled): (i32, Option<i32>, bool) = conn
            .query_row(
                "SELECT agent_message_count, message_limit, message_limit_enabled
                 FROM private_sessions WHERE session_id = ?1",
                [session_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get::<_, i32>(2)? != 0,
                    ))
                },
            )
            .map_err(|e| e.to_string())?;

        if enabled {
            let default_limit = settings_repo::get_or_create_settings(&conn)
                .map(|s| s.private_message_limit_default)
                .unwrap_or(20);
            let effective_limit = limit.unwrap_or(default_limit);
            if count >= effective_limit {
                drop(conn);
                self.emit(
                    "system_notice",
                    serde_json::json!({
                        "session_id": session_id,
                        "content": "已达到消息上限，自动对话已暂停。发送消息或点击重置以继续。"
                    }),
                );
                return Ok(());
            }
        }

        // 如果用户发消息，重置计数器
        if message.sender_type == "user" {
            crate::logger::backend("DEBUG", &format!(
                "[DEBUG on_new_message] resetting agent_message_count for session_id={}",
                session_id
            ));
            conn.execute(
                "UPDATE private_sessions SET agent_message_count = 0, last_reset_at = ?1 WHERE session_id = ?2",
                (chrono::Utc::now().timestamp_millis(), session_id),
            )
            .map_err(|e| e.to_string())?;
        }

        drop(conn);

        // 将消息加入对方角色的 pending_queue
        {
            let mut queue = self.pending_queue.lock().await;
            crate::logger::backend("DEBUG", &format!(
                "[DEBUG on_new_message] pushing message to pending_queue for agent_id={}",
                agent_id
            ));
            queue
                .entry(agent_id.clone())
                .or_insert_with(Vec::new)
                .push(PendingMessage::from(message.clone()));
        }

        // 尝试触发（错误不传播，在 try_trigger_agent 内部处理）
        let _ = self.try_trigger_agent(&agent_id).await;

        Ok(())
    }

    async fn try_trigger_agent(&self, agent_id: &str) -> Result<(), String> {
        let conn = self.db_state.0.lock().await;

        // 检查是否正在触发中（防止并发）
        let is_triggering: bool = conn
            .query_row(
                "SELECT is_triggering FROM trigger_states WHERE agent_id = ?1",
                [agent_id],
                |row| Ok(row.get::<_, i32>(0)? != 0),
            )
            .unwrap_or(false);

        if is_triggering {
            // 检查是否 stale（超过 5 分钟）——防止 panic 导致死锁
            let updated_at: i64 = conn
                .query_row(
                    "SELECT updated_at FROM trigger_states WHERE agent_id = ?1",
                    [agent_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            let now = chrono::Utc::now().timestamp_millis();
            if now - updated_at > 5 * 60 * 1000 {
                crate::logger::backend("WARN", &format!(
                    "[DEBUG try_trigger_agent] agent_id={}, is_triggering=true but stale ({} min), resetting",
                    agent_id, (now - updated_at) / 60000
                ));
                conn.execute(
                    "UPDATE trigger_states SET is_triggering = 0, updated_at = ?1 WHERE agent_id = ?2",
                    (now, agent_id),
                ).unwrap_or_default();
                // 继续触发，不 return
            } else {
                crate::logger::backend("DEBUG", &format!(
                    "[DEBUG try_trigger_agent] agent_id={}, is_triggering=true, skip",
                    agent_id
                ));
                return Ok(());
            }
        }

        let last_trigger = trigger_repo::get_last_trigger_time(&conn, agent_id)
            .map_err(|e| e.to_string())?;
        let settings = settings_repo::get_or_create_settings(&conn)
            .map_err(|e| e.to_string())?;

        drop(conn);

        let now = chrono::Utc::now().timestamp_millis();
        let interval_ms = settings.global_min_trigger_interval as i64 * 1000;
        let decision = if now - last_trigger >= interval_ms { "trigger now" } else { "wait" };

        crate::logger::backend("DEBUG", &format!(
            "[DEBUG try_trigger_agent] agent_id={}, last_trigger={}, interval_ms={}, decision={}",
            agent_id, last_trigger, interval_ms, decision
        ));

        if now - last_trigger >= interval_ms {
            self.trigger_agent(agent_id).await
        } else {
            Ok(())
        }
    }

    pub async fn trigger_agent(&self, agent_id: &str) -> Result<(), String> {
        crate::logger::backend("DEBUG", &format!(
            "[DEBUG trigger_agent] agent_id={}", agent_id
        ));

        // === 阶段 1：原子取出 pending 消息 ===
        let pending = {
            let mut queue = self.pending_queue.lock().await;
            queue.remove(agent_id).unwrap_or_default()
        };

        crate::logger::backend("DEBUG", &format!(
            "[DEBUG trigger_agent] agent_id={}, pending_messages={}",
            agent_id, pending.len()
        ));

        if pending.is_empty() {
            return Ok(());
        }

        // 设置触发中标志
        {
            let conn = self.db_state.0.lock().await;
            let now = chrono::Utc::now().timestamp_millis();
            conn.execute(
                "INSERT INTO trigger_states (agent_id, is_triggering, last_trigger_time, updated_at) 
                 VALUES (?1, 1, ?2, ?2)
                 ON CONFLICT(agent_id) DO UPDATE SET is_triggering = 1, updated_at = excluded.updated_at",
                (agent_id, now),
            ).map_err(|e| e.to_string())?;
        }

        // 发出 typing 事件
        self.emit("agent_typing", serde_json::json!({"agent_id": agent_id}));

        // 使用 finally 模式：无论内部逻辑成功或失败，总是清除 is_triggering
        let inner_result = self.trigger_agent_inner(agent_id, pending).await;

        // 防御性清除：即使 inner_result 是 Err，也要清除标志
        if let Err(e) = self.clear_triggering_flag(agent_id).await {
            crate::logger::backend("ERROR", &format!(
                "[DEBUG trigger_agent] failed to clear is_triggering for agent_id={}: {}",
                agent_id, e
            ));
        }

        inner_result
    }

    async fn trigger_agent_inner(&self, agent_id: &str, pending: Vec<PendingMessage>) -> Result<(), String> {
        // === 阶段 2：检查消息上限 ===
        {
            let conn = self.db_state.0.lock().await;
            let limit_check: Result<(i32, Option<i32>, bool), rusqlite::Error> = conn.query_row(
                "SELECT agent_message_count, message_limit, message_limit_enabled
                 FROM private_sessions WHERE agent_id = ?1",
                [agent_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get::<_, i32>(2)? != 0,
                    ))
                },
            );

            if let Ok((count, limit, enabled)) = limit_check {
                if enabled {
                    let default_limit = settings_repo::get_or_create_settings(&conn)
                        .map(|s| s.private_message_limit_default)
                        .unwrap_or(20);
                    let effective_limit = limit.unwrap_or(default_limit);
                    if count >= effective_limit {
                        drop(conn);
                        self.emit(
                            "system_notice",
                            serde_json::json!({
                                "agent_id": agent_id,
                                "content": "已达到消息上限，自动对话已暂停。发送消息或点击重置以继续。"
                            }),
                        );
                        return Ok(());
                    }
                }
            }
        }

        // === 阶段 3：读取 agent 配置和 prompt ===
        let (agent, prompt) = {
            let conn = self.db_state.0.lock().await;

            let agent = agent_repo::get_by_id(&conn, agent_id)
                .map_err(|e| e.to_string())?
                .ok_or("Agent not found")?;

            let messages_for_prompt: Vec<Message> = pending
                .iter()
                .map(|p| Message {
                    id: String::new(),
                    session_id: p.session_id.clone(),
                    sender_type: p.sender_type.clone(),
                    sender_id: p.sender_id.clone(),
                    content: p.content.clone(),
                    created_at: p.created_at,
                    message_type: "text".to_string(),
                    tool_call_data: None,
                    generation_info: None,
                    is_deleted: false,
                })
                .collect();

            let prompt =
                PromptAssembler::assemble(&conn, agent_id, &messages_for_prompt)
                    .map_err(|e| e.to_string())?;

            crate::logger::backend("DEBUG", &format!(
                "[DEBUG trigger_agent] agent_id={}, prompt_len={}",
                agent_id, prompt.len()
            ));

            (agent, prompt)
        };

        // === 阶段 4：LLM 调用（无锁）===
        let api_key = if let Some(encrypted) = agent.api_key_encrypted {
            crate::crypto::decrypt(&encrypted)
                .map_err(|e| format!("Failed to decrypt API key: {}", e))?
        } else {
            self.restore_pending(agent_id, pending).await;
            return Err("Agent has no API key configured".to_string());
        };

        let provider = OpenAiCompatibleProvider::new(
            api_key,
            agent.base_url,
            agent.model_name.unwrap_or_else(|| "gpt-4o".to_string()),
            agent.temperature,
            agent.max_tokens,
        );

        self.emit(
            "agent_triggered",
            serde_json::json!({"agent_id": agent_id}),
        );

        let response = match Self::call_llm(&provider, &prompt, vec![]).await {
            Ok(resp) => {
                crate::logger::backend("DEBUG", &format!(
                    "[DEBUG trigger_agent] agent_id={}, tool_calls_count={}",
                    agent_id, resp.tool_calls.len()
                ));
                resp
            }
            Err(e) => {
                crate::logger::backend("ERROR", &format!(
                    "[DEBUG trigger_agent] agent_id={}, llm_call_failed={}",
                    agent_id, e
                ));
                self.restore_pending(agent_id, pending).await;
                self.emit(
                    "agent_error",
                    serde_json::json!({"agent_id": agent_id, "error": e}),
                );
                return Ok(());
            }
        };

        // === 阶段 5：执行 Tool Calls ===
        let executor = ToolExecutor::new(self.db_state.clone());
        let agent_messages = match executor.execute(agent_id, response.tool_calls).await {
            Ok(msgs) => msgs,
            Err(e) => {
                crate::logger::backend("ERROR", &format!(
                    "[DEBUG trigger_agent] agent_id={}, tool_execution_failed={}",
                    agent_id, e
                ));
                self.restore_pending(agent_id, pending).await;
                self.emit(
                    "agent_error",
                    serde_json::json!({"agent_id": agent_id, "error": e.to_string()}),
                );
                return Ok(());
            }
        };

        crate::logger::backend("DEBUG", &format!(
            "[DEBUG trigger_agent] agent_id={}, agent_messages_count={}",
            agent_id, agent_messages.len()
        ));

        // === 阶段 6：更新计数器和会话预览 ===
        {
            let conn = self.db_state.0.lock().await;

            for msg in &agent_messages {
                // 递增消息计数器
                conn.execute(
                    "UPDATE private_sessions SET agent_message_count = agent_message_count + 1 WHERE session_id = ?1",
                    [&msg.session_id],
                ).map_err(|e| e.to_string())?;

                // 更新会话最后消息预览（按字符截断，防止 UTF-8 切片 panic）
                let preview = truncate_preview(&msg.content, 100);
                let _ = session_repo::update_session_last_message(&conn, &msg.session_id, &preview);
            }
        }

        // 更新触发时间（同时清除 is_triggering，作为双重保险）
        {
            let conn = self.db_state.0.lock().await;
            trigger_repo::update_trigger_time(&conn, agent_id)
                .map_err(|e| e.to_string())?;
        }

        // === 阶段 7：触发链 ===
        for msg in &agent_messages {
            self.emit("new_message", msg);

            // 把消息加入对方角色的 pending_queue（排除自己，防止自我触发）
            let target_agent_id: Option<String> = {
                let conn = self.db_state.0.lock().await;
                conn.query_row(
                    "SELECT agent_id FROM private_sessions WHERE session_id = ?1",
                    [&msg.session_id],
                    |row| row.get(0),
                ).ok()
            };

            if let Some(target_agent_id) = target_agent_id {
                // 私聊中 target_agent_id 就是发送消息的 agent 自己，不应推回自己的 queue
                if target_agent_id != agent_id {
                    let mut queue = self.pending_queue.lock().await;
                    queue.entry(target_agent_id.clone())
                        .or_insert_with(Vec::new)
                        .push(PendingMessage::from(msg.clone()));
                    crate::logger::backend("DEBUG", &format!(
                        "[DEBUG trigger_agent] pushed message to target_agent_id={} (from agent_id={})",
                        target_agent_id, agent_id
                    ));
                } else {
                    crate::logger::backend("DEBUG", &format!(
                        "[DEBUG trigger_agent] skipped self-trigger for agent_id={}", agent_id
                    ));
                }
            }
        }

        self.emit(
            "agent_completed",
            serde_json::json!({"agent_id": agent_id}),
        );

        Ok(())
    }

    async fn restore_pending(&self, agent_id: &str, pending: Vec<PendingMessage>) {
        if !pending.is_empty() {
            let mut queue = self.pending_queue.lock().await;
            queue.entry(agent_id.to_string())
                .or_insert_with(Vec::new)
                .extend(pending);
            crate::logger::backend("DEBUG", &format!(
                "[DEBUG trigger_agent] restored {} pending messages for agent_id={}",
                queue.get(agent_id).map(|v| v.len()).unwrap_or(0),
                agent_id
            ));
        }
    }

    async fn clear_triggering_flag(&self, agent_id: &str) -> Result<(), String> {
        let conn = self.db_state.0.lock().await;
        conn.execute(
            "UPDATE trigger_states SET is_triggering = 0, updated_at = ?1 WHERE agent_id = ?2",
            (chrono::Utc::now().timestamp_millis(), agent_id),
        ).map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn call_llm(
        provider: &OpenAiCompatibleProvider,
        system_prompt: &str,
        messages: Vec<serde_json::Value>,
    ) -> Result<LlmResponse, String> {
        crate::logger::backend("DEBUG", &format!(
            "[DEBUG call_llm] system_prompt_len={}, messages_count={}",
            system_prompt.len(), messages.len()
        ));

        let tools = vec![send_message_tool_schema()];
        let result = provider
            .chat(system_prompt, messages, tools)
            .await;

        match &result {
            Ok(_) => crate::logger::backend("DEBUG", "[DEBUG call_llm] llm_call succeeded"),
            Err(e) => crate::logger::backend("ERROR", &format!("[DEBUG call_llm] llm_call failed: {}", e)),
        }

        result.map_err(|e| format!("LLM call failed: {}", e))
    }

    /// 后台扫描任务：定期检查 pending_queue 中是否有角色可以触发
    pub async fn start_background_scan(self) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
        loop {
            interval.tick().await;

            let agent_ids: Vec<String> = {
                let queue = self.pending_queue.lock().await;
                queue.keys().cloned().collect()
            };

            for agent_id in agent_ids {
                let _ = self.try_trigger_agent(&agent_id).await;
            }
        }
    }
}

/// 安全截断字符串，按字符计数，避免 UTF-8 切片 panic
pub fn truncate_preview(content: &str, max_chars: usize) -> String {
    if content.chars().count() > max_chars {
        content.chars().take(max_chars).collect::<String>() + "..."
    } else {
        content.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::truncate_preview;

    #[test]
    fn test_truncate_preview_chinese_no_panic() {
        // 120 bytes, 60 chars — old &s[..100] would panic here
        let content = "你好".repeat(60);
        let preview = truncate_preview(&content, 100);
        assert!(preview.ends_with("..."));
        assert_eq!(preview.chars().count(), 103); // 100 chars + "..."
    }

    #[test]
    fn test_truncate_preview_exact_boundary() {
        // 99 bytes, 33 chars — should NOT truncate
        let content = "你好".repeat(33);
        let preview = truncate_preview(&content, 100);
        assert_eq!(preview, content);
    }

    #[test]
    fn test_truncate_preview_short() {
        let preview = truncate_preview("Hello", 100);
        assert_eq!(preview, "Hello");
    }

    #[test]
    fn test_truncate_preview_empty() {
        let preview = truncate_preview("", 100);
        assert_eq!(preview, "");
    }

    #[test]
    fn test_tokio_runtime_new() {
        // 诊断：验证 tokio::runtime::Runtime::new 在 Windows 测试二进制中是否正常
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            assert_eq!(1 + 1, 2);
        });
    }

    #[test]
    fn test_rusqlite_in_memory() {
        // 诊断：验证 rusqlite 在 Windows 测试二进制中是否正常
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE test (id INTEGER PRIMARY KEY)", []).unwrap();
        let count: i32 = conn.query_row("SELECT COUNT(*) FROM test", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 0);
    }

    // 诊断结论：tauri::AppHandle 在 Windows lib 测试二进制中会导致 STATUS_ENTRYPOINT_NOT_FOUND
    // 因此所有涉及 Scheduler（含 AppHandle）的集成测试移至外部验证
    // #[test]
    // fn test_apphandle_option() { ... }

    // 注意：test_clear_triggering_flag 因 Windows 测试二进制入口点问题无法通过 cargo test --lib 运行
    // 已通过 cargo run 手动验证 clear_triggering_flag 逻辑正确
    // #[test]
    // fn test_clear_triggering_flag() { ... }
}
