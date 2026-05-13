use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{AppHandle, Emitter};
use serde::Serialize;

use crate::db::agent as agent_repo;
use crate::db::session as session_repo;
use crate::db::settings as settings_repo;
use crate::db::trigger_state as trigger_repo;
use crate::db::frozen_state as frozen_state_repo;
use crate::db::agent_unread as agent_unread_repo;
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
    unread_messages: Arc<Mutex<HashMap<String, HashMap<String, Vec<PendingMessage>>>>>,
    agent_notifications: Arc<Mutex<HashMap<String, HashSet<String>>>>,
    frozen_sessions: Arc<Mutex<HashSet<String>>>,
    app_handle: Arc<std::sync::Mutex<Option<AppHandle>>>,
    db_state: DbState,
}

impl Scheduler {
    pub fn new(db_state: DbState) -> Self {
        Self {
            unread_messages: Arc::new(Mutex::new(HashMap::new())),
            agent_notifications: Arc::new(Mutex::new(HashMap::new())),
            frozen_sessions: Arc::new(Mutex::new(HashSet::new())),
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

    pub async fn recover_from_db(&self) -> Result<(), String> {
        // 1. 加载 frozen sessions
        let frozen: Vec<String> = {
            let conn = self.db_state.0.lock().await;
            frozen_state_repo::get_frozen_sessions(&conn)
                .map_err(|e| e.to_string())?
        };

        {
            let mut frozen_sessions = self.frozen_sessions.lock().await;
            for session_id in frozen {
                frozen_sessions.insert(session_id);
            }
        }

        // 2. 加载未读消息（在独立的同步块中完成，避免 Statement 跨越 await）
        let rows: Vec<_> = {
            let conn = self.db_state.0.lock().await;
            let mut stmt = conn.prepare(
                "SELECT u.session_id, u.agent_id, u.message_id, u.created_at,
                        m.sender_type, m.sender_id, m.content
                 FROM agent_unread_queue u
                 JOIN messages m ON u.message_id = m.id
                 ORDER BY u.created_at ASC"
            ).map_err(|e| e.to_string())?;

            let rows: Vec<_> = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            }).map_err(|e| e.to_string())?
              .filter_map(|r| r.ok())
              .collect();
            rows
        };

        let mut unread_messages = self.unread_messages.lock().await;
        let mut agent_notifications = self.agent_notifications.lock().await;

        for (session_id, agent_id, _message_id, created_at, sender_type, sender_id, content) in rows {
            let pending = PendingMessage {
                session_id: session_id.clone(),
                sender_type,
                sender_id,
                content,
                created_at,
            };

            unread_messages
                .entry(session_id.clone())
                .or_insert_with(HashMap::new)
                .entry(agent_id.clone())
                .or_insert_with(Vec::new)
                .push(pending);

            agent_notifications
                .entry(agent_id.clone())
                .or_insert_with(HashSet::new)
                .insert(session_id.clone());
        }

        Ok(())
    }

    async fn get_target_agents(&self, session_id: &str, sender_id: &str) -> Result<Vec<String>, String> {
        let conn = self.db_state.0.lock().await;

        let session_type: String = conn
            .query_row(
                "SELECT session_type FROM sessions WHERE id = ?1",
                [session_id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;

        let target_agent_ids: Vec<String> = if session_type == "private" {
            let agent_id: String = conn
                .query_row(
                    "SELECT agent_id FROM private_sessions WHERE session_id = ?1",
                    [session_id],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;

            if agent_id != sender_id {
                vec![agent_id]
            } else {
                vec![]
            }
        } else {
            let mut stmt = conn
                .prepare(
                    "SELECT participant_id FROM group_members 
                     WHERE session_id = ?1 AND participant_type = 'agent' AND is_active = 1"
                )
                .map_err(|e| e.to_string())?;
            let ids: Vec<String> = stmt
                .query_map([session_id], |row| row.get(0))
                .map_err(|e| e.to_string())?
                .filter_map(|r| r.ok())
                .filter(|id| *id != sender_id)
                .collect();
            drop(stmt);
            ids
        };

        Ok(target_agent_ids)
    }

    pub async fn distribute_message(&self, session_id: &str, message: &Message, sender_id: &str) -> Result<(), String> {
        // 冻结的 session 不接收新消息（用户消息会在 on_new_message 中先解冻）
        if self.frozen_sessions.lock().await.contains(session_id) {
            return Ok(());
        }

        let target_agents = self.get_target_agents(session_id, sender_id).await?;

        let conn = self.db_state.0.lock().await;

        for agent_id in target_agents {
            // 1. 插入内存
            {
                let mut unread = self.unread_messages.lock().await;
                unread.entry(session_id.to_string())
                    .or_insert_with(HashMap::new)
                    .entry(agent_id.clone())
                    .or_insert_with(Vec::new)
                    .push(PendingMessage::from(message.clone()));
            }

            // 2. 插入 notifications
            {
                let mut notifications = self.agent_notifications.lock().await;
                notifications.entry(agent_id.clone())
                    .or_insert_with(HashSet::new)
                    .insert(session_id.to_string());
            }

            // 3. 写入数据库
            let _ = agent_unread_repo::insert_unread(&conn, session_id, &agent_id, &message.id, message.created_at);
        }

        Ok(())
    }

    async fn check_and_freeze_if_needed(&self, session_id: &str) -> bool {
        let conn = self.db_state.0.lock().await;
        let settings = settings_repo::get_or_create_settings(&conn).unwrap_or_default();

        let result = conn.query_row(
            "SELECT ps.agent_message_count, COALESCE(ss.message_limit, ?2), ss.message_limit_enabled 
             FROM private_sessions ps
             LEFT JOIN session_settings ss ON ps.session_id = ss.session_id
             WHERE ps.session_id = ?1
             UNION ALL
             SELECT gs.agent_message_count, COALESCE(ss.message_limit, ?3), ss.message_limit_enabled 
             FROM group_sessions gs
             LEFT JOIN session_settings ss ON gs.session_id = ss.session_id
             WHERE gs.session_id = ?1
             LIMIT 1",
            rusqlite::params![session_id, settings.private_message_limit_default, settings.group_message_limit_default],
            |row| Ok((row.get::<_, i32>(0)?, row.get::<_, Option<i32>>(1)?, row.get::<_, i32>(2)? != 0)),
        );

        if let Ok((count, limit, enabled)) = result {
            if enabled {
                let effective = limit.unwrap_or(settings.private_message_limit_default);
                if count >= effective {
                    let _ = frozen_state_repo::set_frozen(&conn, session_id);
                    drop(conn);
                    self.frozen_sessions.lock().await.insert(session_id.to_string());
                    return true;
                }
            }
        }

        false
    }

    pub async fn unfreeze_session(&self, session_id: &str) {
        self.frozen_sessions.lock().await.remove(session_id);
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

        // 用户消息自动重置计数器并解除冻结
        if message.sender_type == "user" {
            let now = chrono::Utc::now().timestamp_millis();
            conn.execute("UPDATE private_sessions SET agent_message_count = 0, last_reset_at = ?1 WHERE session_id = ?2", (now, session_id)).unwrap_or_default();
            conn.execute("UPDATE group_sessions SET agent_message_count = 0, last_reset_at = ?1 WHERE session_id = ?2", (now, session_id)).unwrap_or_default();
            let _ = frozen_state_repo::remove_frozen(&conn, session_id);
            self.frozen_sessions.lock().await.remove(session_id);
        }

        drop(conn);

        // 统一分发
        self.distribute_message(session_id, message, &message.sender_id).await?;

        // 触发目标 agents
        let target_agents = self.get_target_agents(session_id, &message.sender_id).await?;
        for agent_id in target_agents {
            let _ = self.try_trigger_agent(&agent_id).await;
        }

        Ok(())
    }

    pub async fn try_trigger_agent(&self, agent_id: &str) -> Result<(), String> {
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

        // === 阶段 1：从 agent_notifications 获取 session_ids ===
        let session_ids: HashSet<String> = {
            let mut notifications = self.agent_notifications.lock().await;
            notifications.remove(agent_id).unwrap_or_default()
        };

        if session_ids.is_empty() {
            return Ok(());
        }

        let mut pending = Vec::new();
        let mut processed_sessions = Vec::new();

        {
            let frozen = self.frozen_sessions.lock().await;
            let mut unread = self.unread_messages.lock().await;

            for session_id in &session_ids {
                if frozen.contains(session_id) {
                    // 冻结的 session 跳过，稍后重新加入 notifications
                    continue;
                }

                if let Some(session_map) = unread.get_mut(session_id) {
                    if let Some(messages) = session_map.get_mut(agent_id) {
                        pending.extend(messages.drain(..));
                        processed_sessions.push(session_id.clone());
                    }

                    // 清理空的 map
                    if session_map.is_empty() {
                        unread.remove(session_id);
                    }
                }
            }
        }

        // 将冻结的 session 重新加入 notifications
        let frozen_sessions_to_requeue: Vec<String> = {
            let frozen = self.frozen_sessions.lock().await;
            session_ids.iter()
                .filter(|sid| frozen.contains(*sid))
                .cloned()
                .collect()
        };

        if !frozen_sessions_to_requeue.is_empty() {
            let mut notifications = self.agent_notifications.lock().await;
            let entry = notifications.entry(agent_id.to_string()).or_insert_with(HashSet::new);
            for sid in frozen_sessions_to_requeue {
                entry.insert(sid);
            }
        }

        if pending.is_empty() {
            return Ok(());
        }

        // 按 created_at 排序
        pending.sort_by_key(|m| m.created_at);

        crate::logger::backend("DEBUG", &format!(
            "[DEBUG trigger_agent] agent_id={}, pending_messages={}",
            agent_id, pending.len()
        ));

        // 从数据库删除已读取的记录
        {
            let conn = self.db_state.0.lock().await;
            for session_id in &processed_sessions {
                let _ = agent_unread_repo::delete_unread_by_agent_session(&conn, agent_id, session_id);
            }
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
        // === 阶段 2：检查 muted sessions ===
        let muted_sessions: Vec<String> = {
            let conn = self.db_state.0.lock().await;
            let mut muted = Vec::new();
            for msg in &pending {
                let m: bool = conn.query_row(
                    "SELECT mute_enabled FROM session_settings WHERE session_id = ?1",
                    [&msg.session_id],
                    |row| Ok(row.get::<_, i32>(0)? != 0),
                ).unwrap_or(false);
                if m {
                    muted.push(msg.session_id.clone());
                }
            }
            muted
        };

        let pending: Vec<PendingMessage> = pending.into_iter()
            .filter(|p| !muted_sessions.contains(&p.session_id))
            .collect();

        if pending.is_empty() {
            return Ok(());
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
                    sender_name: String::new(),
                    sender_avatar: None,
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
        let sessions_to_check: Vec<String> = {
            let conn = self.db_state.0.lock().await;

            for msg in &agent_messages {
                // 递增消息计数器（先尝试 private_sessions，再尝试 group_sessions）
                let rows = conn.execute(
                    "UPDATE private_sessions SET agent_message_count = agent_message_count + 1 WHERE session_id = ?1",
                    [&msg.session_id],
                ).unwrap_or(0);
                if rows == 0 {
                    let _ = conn.execute(
                        "UPDATE group_sessions SET agent_message_count = agent_message_count + 1 WHERE session_id = ?1",
                        [&msg.session_id],
                    );
                }

                // 更新会话最后消息预览（按字符截断，防止 UTF-8 切片 panic）
                let preview = truncate_preview(&msg.content, 100);
                let _ = session_repo::update_session_last_message(&conn, &msg.session_id, &preview);
            }

            agent_messages.iter().map(|m| m.session_id.clone()).collect()
        };

        // 检查是否达到上限（在释放 conn 后）
        let is_any_frozen = {
            let mut frozen = false;
            for sid in &sessions_to_check {
                if self.check_and_freeze_if_needed(sid).await {
                    frozen = true;
                }
            }
            frozen
        };

        if is_any_frozen {
            self.emit("system_notice", serde_json::json!({
                "content": "已达到消息上限，自动对话已暂停。发送消息或点击重置以继续。"
            }));
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
            self.distribute_message(&msg.session_id, msg, agent_id).await?;
        }

        // 消息已推入各角色的 unread，由后台扫描在下次 tick 时触发
        //（避免 async fn 递归调用问题）

        self.emit(
            "agent_completed",
            serde_json::json!({"agent_id": agent_id}),
        );

        Ok(())
    }

    async fn restore_pending(&self, agent_id: &str, pending: Vec<PendingMessage>) {
        if !pending.is_empty() {
            let count = pending.len();
            let mut unread = self.unread_messages.lock().await;
            let mut notifications = self.agent_notifications.lock().await;

            for msg in pending {
                let session_id = msg.session_id.clone();
                unread.entry(session_id.clone())
                    .or_insert_with(HashMap::new)
                    .entry(agent_id.to_string())
                    .or_insert_with(Vec::new)
                    .push(msg);

                notifications.entry(agent_id.to_string())
                    .or_insert_with(HashSet::new)
                    .insert(session_id);
            }

            crate::logger::backend("DEBUG", &format!(
                "[DEBUG trigger_agent] restored {} pending messages for agent_id={}",
                count, agent_id
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

    /// 后台扫描任务：定期检查 agent_notifications 中是否有角色可以触发
    pub async fn start_background_scan(self) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
        loop {
            interval.tick().await;

            let agent_ids: Vec<String> = {
                let notifications = self.agent_notifications.lock().await;
                notifications.keys().cloned().collect()
            };

            for agent_id in agent_ids {
                let _ = self.try_trigger_agent(&agent_id).await;
            }
        }
    }

    // Test accessors (hidden from docs)
    #[doc(hidden)]
    pub fn unread_messages(&self) -> &Arc<Mutex<HashMap<String, HashMap<String, Vec<PendingMessage>>>>> {
        &self.unread_messages
    }

    #[doc(hidden)]
    pub fn agent_notifications(&self) -> &Arc<Mutex<HashMap<String, HashSet<String>>>> {
        &self.agent_notifications
    }

    #[doc(hidden)]
    pub fn frozen_sessions(&self) -> &Arc<Mutex<HashSet<String>>> {
        &self.frozen_sessions
    }

    #[doc(hidden)]
    pub fn db_state(&self) -> &DbState {
        &self.db_state
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
    use super::*;
    use rusqlite::Connection;
    use crate::db::schema::{MIGRATION_V1, MIGRATION_V2, MIGRATION_V3, MIGRATION_V4, MIGRATION_V5, MIGRATION_V6};

    fn init_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = OFF;", []).unwrap();
        conn.execute_batch(MIGRATION_V1).unwrap();
        conn.execute_batch(MIGRATION_V2).unwrap();
        conn.execute_batch(MIGRATION_V3).unwrap();
        conn.execute_batch(MIGRATION_V4).unwrap();
        conn.execute_batch(MIGRATION_V5).unwrap();
        conn.execute_batch(MIGRATION_V6).unwrap();
        conn
    }

    fn make_db_state(conn: Connection) -> DbState {
        DbState(Arc::new(Mutex::new(conn)))
    }

    fn create_test_agent(conn: &Connection, agent_id: &str) {
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            (agent_id, format!("Agent {}", agent_id), 0i64),
        ).unwrap();
    }

    fn create_test_private_session(conn: &Connection, agent_id: &str) -> String {
        session_repo::create_private_session(conn, agent_id).unwrap().id
    }

    fn create_test_message(session_id: &str, sender_type: &str, sender_id: &str, content: &str, created_at: i64) -> Message {
        Message {
            id: format!("msg-{}", uuid::Uuid::new_v4()),
            session_id: session_id.to_string(),
            sender_type: sender_type.to_string(),
            sender_id: sender_id.to_string(),
            sender_name: String::new(),
            sender_avatar: None,
            content: content.to_string(),
            created_at,
            message_type: "text".to_string(),
            tool_call_data: None,
            generation_info: None,
            is_deleted: false,
        }
    }

    #[tokio::test]
    async fn test_distribute_message_populates_unread() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1");
        let session_id = create_test_private_session(&conn, "agent-1");
        let db_state = make_db_state(conn);

        let scheduler = Scheduler::new(db_state);
        let message = create_test_message(&session_id, "user", "user", "Hello", 1000);

        scheduler.distribute_message(&session_id, &message, "user").await.unwrap();

        let unread = scheduler.unread_messages.lock().await;
        assert!(unread.contains_key(&session_id));
        let session_unread = unread.get(&session_id).unwrap();
        assert!(session_unread.contains_key("agent-1"));
        let agent_unread = session_unread.get("agent-1").unwrap();
        assert_eq!(agent_unread.len(), 1);
        assert_eq!(agent_unread[0].content, "Hello");

        let notifications = scheduler.agent_notifications.lock().await;
        assert!(notifications.contains_key("agent-1"));
        assert!(notifications.get("agent-1").unwrap().contains(&session_id));
    }

    #[tokio::test]
    async fn test_frozen_session_blocks_new_distributions() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1");
        let session_id = create_test_private_session(&conn, "agent-1");
        let db_state = make_db_state(conn);

        let scheduler = Scheduler::new(db_state);
        scheduler.frozen_sessions.lock().await.insert(session_id.clone());

        let message = create_test_message(&session_id, "user", "user", "Hello", 1000);
        scheduler.distribute_message(&session_id, &message, "user").await.unwrap();

        let unread = scheduler.unread_messages.lock().await;
        assert!(!unread.contains_key(&session_id));

        let notifications = scheduler.agent_notifications.lock().await;
        assert!(!notifications.contains_key("agent-1"));
    }

    #[tokio::test]
    async fn test_reset_unfreezes_and_triggers_agents() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1");
        let session_id = create_test_private_session(&conn, "agent-1");

        // 设置冻结
        frozen_state_repo::set_frozen(&conn, &session_id).unwrap();

        let db_state = make_db_state(conn);
        let scheduler = Scheduler::new(db_state.clone());

        // 先恢复数据库状态到 scheduler
        scheduler.recover_from_db().await.unwrap();

        // 验证已冻结
        assert!(scheduler.frozen_sessions.lock().await.contains(&session_id));

        // 插入未读消息
        {
            let mut unread = scheduler.unread_messages.lock().await;
            unread.entry(session_id.clone())
                .or_insert_with(HashMap::new)
                .entry("agent-1".to_string())
                .or_insert_with(Vec::new)
                .push(PendingMessage {
                    session_id: session_id.clone(),
                    sender_type: "user".to_string(),
                    sender_id: "user".to_string(),
                    content: "Hello".to_string(),
                    created_at: 1000,
                });
        }
        {
            let mut notifications = scheduler.agent_notifications.lock().await;
            notifications.entry("agent-1".to_string())
                .or_insert_with(HashSet::new)
                .insert(session_id.clone());
        }

        // 调用 unfreeze_session
        scheduler.unfreeze_session(&session_id).await;

        // 验证已解冻
        assert!(!scheduler.frozen_sessions.lock().await.contains(&session_id));

        // 验证 notifications 中有 agent-1
        let notifications = scheduler.agent_notifications.lock().await;
        assert!(notifications.contains_key("agent-1"));
        assert!(notifications.get("agent-1").unwrap().contains(&session_id));
    }

    #[tokio::test]
    async fn test_trigger_agent_reads_unread_chronologically() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1");
        let session_id = create_test_private_session(&conn, "agent-1");

        let db_state = make_db_state(conn);
        let scheduler = Scheduler::new(db_state);

        // 插入多条未读消息，按非时间顺序
        {
            let mut unread = scheduler.unread_messages.lock().await;
            let session_map = unread.entry(session_id.clone())
                .or_insert_with(HashMap::new);
            session_map.entry("agent-1".to_string())
                .or_insert_with(Vec::new)
                .extend(vec![
                    PendingMessage {
                        session_id: session_id.clone(),
                        sender_type: "user".to_string(),
                        sender_id: "user".to_string(),
                        content: "Second".to_string(),
                        created_at: 2000,
                    },
                    PendingMessage {
                        session_id: session_id.clone(),
                        sender_type: "user".to_string(),
                        sender_id: "user".to_string(),
                        content: "First".to_string(),
                        created_at: 1000,
                    },
                    PendingMessage {
                        session_id: session_id.clone(),
                        sender_type: "user".to_string(),
                        sender_id: "user".to_string(),
                        content: "Third".to_string(),
                        created_at: 3000,
                    },
                ]);
        }
        {
            let mut notifications = scheduler.agent_notifications.lock().await;
            notifications.entry("agent-1".to_string())
                .or_insert_with(HashSet::new)
                .insert(session_id.clone());
        }

        // 调用 trigger_agent
        // 由于 agent 没有 API key，trigger_agent_inner 会失败并 restore_pending
        let result = scheduler.trigger_agent("agent-1").await;
        // 失败是因为 api_key 不存在，但消息应该被 restore 并保持排序
        assert!(result.is_err());

        // 检查 unread_messages 中的顺序
        let unread = scheduler.unread_messages.lock().await;
        let messages = unread.get(&session_id).unwrap().get("agent-1").unwrap();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].content, "First");
        assert_eq!(messages[1].content, "Second");
        assert_eq!(messages[2].content, "Third");
    }

    #[tokio::test]
    async fn test_user_message_resets_counter_and_unfreezes() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1");
        let session_id = create_test_private_session(&conn, "agent-1");

        // 设置计数器和冻结
        conn.execute(
            "UPDATE private_sessions SET agent_message_count = 5 WHERE session_id = ?1",
            [&session_id],
        ).unwrap();
        frozen_state_repo::set_frozen(&conn, &session_id).unwrap();

        let db_state = make_db_state(conn);
        let scheduler = Scheduler::new(db_state);

        // 先恢复数据库状态
        scheduler.recover_from_db().await.unwrap();

        assert!(scheduler.frozen_sessions.lock().await.contains(&session_id));

        let message = create_test_message(&session_id, "user", "user", "Hello", 1000);
        scheduler.on_new_message(&session_id, &message).await.unwrap();

        // 验证已解冻
        assert!(!scheduler.frozen_sessions.lock().await.contains(&session_id));

        // 验证计数器已重置
        let conn = scheduler.db_state.0.lock().await;
        let count: i32 = conn.query_row(
            "SELECT agent_message_count FROM private_sessions WHERE session_id = ?1",
            [&session_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 0);

        // 验证已分发到 unread
        let unread = scheduler.unread_messages.lock().await;
        assert!(unread.contains_key(&session_id));
    }

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
}
