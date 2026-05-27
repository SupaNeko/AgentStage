use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{AppHandle, Emitter};
use serde::Serialize;
use chrono::Timelike;

use crate::db::agent as agent_repo;
use crate::db::settings as settings_repo;
use crate::db::trigger_state as trigger_repo;
use crate::db::frozen_state as frozen_state_repo;
use crate::db::agent_unread as agent_unread_repo;
use crate::db::scheduled_task as scheduled_task_repo;
use crate::db::connection::DbState;
use rand::Rng;
use crate::llm::openai::OpenAiCompatibleProvider;
use crate::llm::provider::LlmProvider;
use crate::llm::prompt::PromptAssembler;
use crate::llm::prompt_templates;
use crate::llm::tool::{get_all_tool_schemas, update_relationship_tool_schema, update_memory_tool_schema, LlmResponse, ToolExecutor};
use crate::models::message::Message;

#[derive(Clone)]
pub struct PendingMessage {
    pub message_id: String,
    pub session_id: String,
    pub sender_type: String,
    pub sender_id: String,
    pub content: String,
    pub created_at: i64,
    pub page_index: i32,
    pub restored_from_failure: bool,
}

impl From<Message> for PendingMessage {
    fn from(msg: Message) -> Self {
        Self {
            message_id: msg.id,
            session_id: msg.session_id,
            sender_type: msg.sender_type,
            sender_id: msg.sender_id,
            content: msg.content,
            created_at: msg.created_at,
            page_index: msg.page_index,
            restored_from_failure: false,
        }
    }
}

#[derive(Clone)]
pub enum SpecialTriggerContext {
    Timer {
        description: String,
        target_session_id: Option<String>,
    },
    Proactive,
}

#[derive(Clone)]
pub struct Scheduler {
    unread_messages: Arc<Mutex<HashMap<String, HashMap<String, Vec<PendingMessage>>>>>,
    agent_notifications: Arc<Mutex<HashMap<String, HashSet<String>>>>,
    frozen_sessions: Arc<Mutex<HashSet<String>>>,
    running_summaries: Arc<Mutex<HashSet<String>>>,
    proactive_timers: Arc<Mutex<HashMap<String, i64>>>,
    app_handle: Arc<std::sync::Mutex<Option<AppHandle>>>,
    db_state: DbState,
}

impl Scheduler {
    pub fn new(db_state: DbState) -> Self {
        Self {
            unread_messages: Arc::new(Mutex::new(HashMap::new())),
            agent_notifications: Arc::new(Mutex::new(HashMap::new())),
            frozen_sessions: Arc::new(Mutex::new(HashSet::new())),
            running_summaries: Arc::new(Mutex::new(HashSet::new())),
            proactive_timers: Arc::new(Mutex::new(HashMap::new())),
            app_handle: Arc::new(std::sync::Mutex::new(None)),
            db_state,
        }
    }

    pub fn set_app_handle(&self, handle: AppHandle) {
        *self.app_handle.lock().unwrap() = Some(handle);
    }

    fn emit(&self, event: &str, payload: impl Serialize + Clone) {
        if let Some(handle) = self.app_handle.lock().unwrap().as_ref() {
            let result = handle.emit(event, payload.clone());
            match &result {
                Ok(_) => crate::logger::debug(&format!(
                    "[DEBUG emit] event='{}' sent OK", event
                )),
                Err(e) => crate::logger::error(&format!(
                    "[DEBUG emit] event='{}' failed: {}", event, e
                )),
            }
        } else {
            crate::logger::warn(&format!(
                "[DEBUG emit] event='{}' dropped (no app_handle)", event
            ));
        }
    }

    /// 执行 async future，捕获 panic 并转为 Err，防止静默崩溃导致 is_triggering 永久卡死
    async fn catch_async_panic<F, T>(future: F) -> Result<T, String>
    where
        F: std::future::Future<Output = Result<T, String>> + Send + 'static,
        T: Send + 'static,
    {
        match tokio::spawn(future).await {
            Ok(result) => result,
            Err(join_error) => {
                let msg = format!("[Scheduler] async task panicked: {}", join_error);
                crate::logger::error(&msg);
                Err(msg)
            }
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
                        m.sender_type, m.sender_id, m.content, m.page_index
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
                    row.get::<_, i32>(7)?,
                ))
            }).map_err(|e| e.to_string())?
              .filter_map(|r| r.ok())
              .collect();
            rows
        };

        let mut unread_messages = self.unread_messages.lock().await;
        let mut agent_notifications = self.agent_notifications.lock().await;

        for (session_id, agent_id, message_id, created_at, sender_type, sender_id, content, page_index) in rows {
            let pending = PendingMessage {
                message_id,
                session_id: session_id.clone(),
                sender_type,
                sender_id,
                content,
                created_at,
                page_index,
                restored_from_failure: false,
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

        // 3. 启动时清理已过期的单次任务（应用关闭期间错过的）
        {
            let conn = self.db_state.0.lock().await;
            let now = chrono::Utc::now().timestamp_millis();
            match conn.execute(
                "DELETE FROM scheduled_tasks WHERE task_type = 'single' AND next_trigger_at <= ?1",
                [now],
            ) {
                Ok(deleted) => {
                    if deleted > 0 {
                        crate::logger::info(&format!("[recover_from_db] cleaned up {} expired single tasks", deleted));
                    }
                }
                Err(e) => {
                    crate::logger::error(&format!("[recover_from_db] cleanup expired single tasks failed: {}", e));
                }
            }
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

        crate::logger::debug(&format!(
            "[DEBUG get_target_agents] session_id={}, sender_id={}, session_type={}",
            session_id, sender_id, session_type
        ));

        let target_agent_ids: Vec<String> = if session_type == "private" {
            let mut stmt = conn
                .prepare(
                    "SELECT participant_1_id as other_id FROM private_sessions 
                     WHERE session_id = ?1 AND participant_1_type = 'agent' AND participant_1_id != ?2
                     UNION
                     SELECT participant_2_id as other_id FROM private_sessions 
                     WHERE session_id = ?1 AND participant_2_type = 'agent' AND participant_2_id != ?2"
                )
                .map_err(|e| e.to_string())?;
            let ids: Vec<String> = stmt
                .query_map(rusqlite::params![session_id, sender_id], |row| row.get(0))
                .map_err(|e| e.to_string())?
                .filter_map(|r| r.ok())
                .collect();
            drop(stmt);
            ids
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

        crate::logger::debug(&format!(
            "[DEBUG get_target_agents] session_id={}, target_agents={:?}",
            session_id, target_agent_ids
        ));

        Ok(target_agent_ids)
    }

    pub async fn distribute_message(&self, session_id: &str, message: &Message, sender_id: &str) -> Result<(), String> {
        crate::logger::debug(&format!(
            "[DEBUG distribute_message] START session_id={}, message_id={}, sender_id={}",
            session_id, message.id, sender_id
        ));

        // Bug 2 fix: Distribution is never blocked by frozen state.
        // Freezing only pauses automatic triggers (handled in trigger_agent), not message enqueue.
        let target_agents = self.get_target_agents(session_id, sender_id).await?;

        crate::logger::debug(&format!(
            "[DEBUG distribute_message] target_agents_count={}, agents={:?}",
            target_agents.len(), target_agents
        ));

        let conn = self.db_state.0.lock().await;

        for agent_id in &target_agents {
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
            let insert_result = agent_unread_repo::insert_unread(&conn, session_id, agent_id, &message.id, message.created_at);
            crate::logger::debug(&format!(
                "[DEBUG distribute_message] agent_id={}, db_insert={}",
                agent_id, if insert_result.is_ok() { "OK" } else { "ERR" }
            ));
        }

        crate::logger::debug(&format!(
            "[DEBUG distribute_message] END session_id={}, distributed_to={} agents",
            session_id, target_agents.len()
        ));

        Ok(())
    }

    async fn check_and_freeze_if_needed(&self, session_id: &str) -> Option<String> {
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
                    // 查询会话名称
                    let session_name: String = conn.query_row(
                        "SELECT name FROM group_sessions WHERE session_id = ?1
                         UNION ALL
                         SELECT CASE WHEN ps.participant_1_type = 'user' THEN a.name ELSE '私聊' END
                         FROM private_sessions ps
                         LEFT JOIN agents a ON ps.participant_2_type = 'agent' AND ps.participant_2_id = a.id
                         WHERE ps.session_id = ?1
                         LIMIT 1",
                        [session_id],
                        |row| row.get(0),
                    ).unwrap_or_else(|_| "会话".to_string());
                    drop(conn);
                    self.frozen_sessions.lock().await.insert(session_id.to_string());
                    return Some(session_name);
                }
            }
        }

        None
    }

    pub async fn unfreeze_session(&self, session_id: &str) {
        self.frozen_sessions.lock().await.remove(session_id);
    }

    /// 重置会话时调用：清理该会话在调度器中的所有内存状态
    pub async fn cancel_session(&self, session_id: &str) {
        // 1. 清除 unread 队列
        let agent_ids: Vec<String> = {
            let mut unread = self.unread_messages.lock().await;
            if let Some(session_map) = unread.remove(session_id) {
                let ids: Vec<String> = session_map.keys().cloned().collect();
                drop(session_map);
                ids
            } else {
                Vec::new()
            }
        };

        // 2. 无论 unread 中是否存在，都清除 notifications（防止残留）
        {
            let mut notifications = self.agent_notifications.lock().await;
            let all_agent_ids: Vec<String> = if agent_ids.is_empty() {
                // 如果 unread 中没有，扫描所有 notifications 查找该 session
                notifications.iter()
                    .filter(|(_, sessions)| sessions.contains(session_id))
                    .map(|(agent_id, _)| agent_id.clone())
                    .collect()
            } else {
                agent_ids
            };

            for agent_id in all_agent_ids {
                if let Some(sessions) = notifications.get_mut(&agent_id) {
                    sessions.remove(session_id);
                    if sessions.is_empty() {
                        notifications.remove(&agent_id);
                    }
                }
            }
        }

        // 3. 清除 frozen 状态
        self.frozen_sessions.lock().await.remove(session_id);

        crate::logger::debug(&format!(
            "[DEBUG cancel_session] session_id={} cleaned", session_id
        ));
    }

    pub fn spawn_session_summary(&self, session_id: String, page_index: i32) {
        let scheduler = self.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = scheduler.run_session_summary(&session_id, page_index).await {
                crate::logger::error(&format!("[SessionSummary] failed for session={} page={}: {}", session_id, page_index, e));
            }
        });
    }

    pub fn spawn_overflow_summary(&self, session_id: String) {
        let scheduler = self.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = scheduler.run_overflow_summary(&session_id).await {
                crate::logger::error(&format!("[OverflowSummary] failed for session={}: {}", session_id, e));
            }
        });
    }

    /// 当有新消息到达时调用（用户发送消息或角色发送消息）
    pub async fn on_new_message(
        &self,
        session_id: &str,
        message: &Message,
    ) -> Result<(), String> {
        crate::logger::debug(&format!(
            "[DEBUG on_new_message] START session_id={}, message_id={}, sender_type={}, sender_id={}",
            session_id, message.id, message.sender_type, message.sender_id
        ));

        let conn = self.db_state.0.lock().await;

        // 用户消息自动重置计数器并解除冻结
        if message.sender_type == "user" {
            let now = chrono::Utc::now().timestamp_millis();
            conn.execute("UPDATE private_sessions SET agent_message_count = 0, last_reset_at = ?1 WHERE session_id = ?2", (now, session_id)).unwrap_or_default();
            conn.execute("UPDATE group_sessions SET agent_message_count = 0, last_reset_at = ?1 WHERE session_id = ?2", (now, session_id)).unwrap_or_default();
            let _ = frozen_state_repo::remove_frozen(&conn, session_id);
            self.frozen_sessions.lock().await.remove(session_id);
            crate::logger::debug(&format!(
                "[DEBUG on_new_message] user message resets counters and unfreezes session_id={}", session_id
            ));
        }

        drop(conn);

        // 统一分发
        self.distribute_message(session_id, message, &message.sender_id).await?;

        // 触发目标 agents
        let target_agents = self.get_target_agents(session_id, &message.sender_id).await?;
        crate::logger::debug(&format!(
            "[DEBUG on_new_message] will try_trigger {} agents: {:?}",
            target_agents.len(), target_agents
        ));
        for agent_id in target_agents {
            crate::logger::debug(&format!(
                "[DEBUG on_new_message] calling try_trigger_agent agent_id={}", agent_id
            ));
            let _ = self.try_trigger_agent(&agent_id).await;
        }

        crate::logger::debug(&format!(
            "[DEBUG on_new_message] END session_id={}, message_id={}", session_id, message.id
        ));

        Ok(())
    }

    pub async fn try_trigger_agent(&self, agent_id: &str) -> Result<(), String> {
        crate::logger::debug(&format!(
            "[DEBUG try_trigger_agent] START agent_id={}", agent_id
        ));

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
                crate::logger::warn(&format!(
                    "[DEBUG try_trigger_agent] agent_id={}, is_triggering=true but stale ({} min), resetting",
                    agent_id, (now - updated_at) / 60000
                ));
                conn.execute(
                    "UPDATE trigger_states SET is_triggering = 0, updated_at = ?1 WHERE agent_id = ?2",
                    (now, agent_id),
                ).unwrap_or_default();
                // 继续触发，不 return
            } else {
                crate::logger::debug(&format!(
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

        crate::logger::debug(&format!(
            "[DEBUG try_trigger_agent] agent_id={}, last_trigger={}, interval_ms={}, decision={}",
            agent_id, last_trigger, interval_ms, decision
        ));

        if now - last_trigger >= interval_ms {
            let result = self.trigger_agent(agent_id).await;
            match &result {
                Ok(_) => crate::logger::debug(&format!(
                    "[DEBUG try_trigger_agent] END agent_id={}, trigger_agent OK", agent_id
                )),
                Err(e) => crate::logger::error(&format!(
                    "[DEBUG try_trigger_agent] END agent_id={}, trigger_agent FAILED: {}", agent_id, e
                )),
            }
            result
        } else {
            crate::logger::debug(&format!(
                "[DEBUG try_trigger_agent] END agent_id={}, skipped (interval not met)", agent_id
            ));
            Ok(())
        }
    }

    pub async fn trigger_agent(&self, agent_id: &str) -> Result<(), String> {
        crate::logger::debug(&format!(
            "[DEBUG trigger_agent] START agent_id={}", agent_id
        ));

        // === 阶段 1：从 agent_notifications 获取 session_ids ===
        let session_ids: HashSet<String> = {
            let mut notifications = self.agent_notifications.lock().await;
            notifications.remove(agent_id).unwrap_or_default()
        };

        crate::logger::debug(&format!(
            "[DEBUG trigger_agent] agent_id={}, notification_sessions={:?}, count={}",
            agent_id, session_ids, session_ids.len()
        ));

        if session_ids.is_empty() {
            crate::logger::debug(&format!(
                "[DEBUG trigger_agent] END agent_id={}, no sessions in notifications", agent_id
            ));
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
                        // 如果该 session 的消息全部是由于调用失败恢复的，跳过本次触发
                        let has_new = messages.iter().any(|m| !m.restored_from_failure);
                        if !has_new {
                            continue;
                        }
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

        let requeue_count = frozen_sessions_to_requeue.len();
        if requeue_count > 0 {
            let mut notifications = self.agent_notifications.lock().await;
            let entry = notifications.entry(agent_id.to_string()).or_insert_with(HashSet::new);
            for sid in frozen_sessions_to_requeue {
                entry.insert(sid);
            }
            crate::logger::debug(&format!(
                "[DEBUG trigger_agent] agent_id={}, requeued_frozen_sessions_count={}",
                agent_id, requeue_count
            ));
        }

        if pending.is_empty() {
            crate::logger::debug(&format!(
                "[DEBUG trigger_agent] END agent_id={}, pending is empty after filtering", agent_id
            ));
            return Ok(());
        }

        // 按 created_at 排序
        pending.sort_by_key(|m| m.created_at);

        crate::logger::debug(&format!(
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

        // 从 pending 消息中读取各 session 的 page_index（确保回复绑定到触发消息所在页）
        let session_pages: HashMap<String, i32> = {
            let mut map = HashMap::new();
            for msg in &pending {
                map.entry(msg.session_id.clone()).or_insert(msg.page_index);
            }
            crate::logger::debug(&format!(
                "[DEBUG trigger_agent] agent_id={}, session_pages={:?}",
                agent_id, map
            ));
            map
        };

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

        // 快照各 session 当前的 page_index（用于阶段 7 判断是否为触发中重置）
        let snapshot_pages: HashMap<String, i32> = {
            let conn = self.db_state.0.lock().await;
            let mut map = HashMap::new();
            for session_id in &processed_sessions {
                let page: i32 = conn.query_row(
                    "SELECT COALESCE(current_chat_page, 0) FROM private_sessions WHERE session_id = ?1
                     UNION ALL
                     SELECT COALESCE(current_chat_page, 0) FROM group_sessions WHERE session_id = ?1
                     LIMIT 1",
                    [session_id],
                    |row| row.get(0),
                ).unwrap_or(0);
                map.insert(session_id.clone(), page);
            }
            map
        };

        // 使用 finally 模式：无论内部逻辑成功或失败，总是清除 is_triggering
        let scheduler = self.clone();
        let agent_id_owned = agent_id.to_string();
        let inner_result = Self::catch_async_panic(async move {
            scheduler.trigger_agent_inner(&agent_id_owned, pending, session_pages, snapshot_pages).await
        }).await;
        match &inner_result {
            Ok(_) => crate::logger::debug(&format!(
                "[DEBUG trigger_agent] agent_id={}, trigger_agent_inner OK", agent_id
            )),
            Err(e) => crate::logger::error(&format!(
                "[DEBUG trigger_agent] agent_id={}, trigger_agent_inner FAILED: {}", agent_id, e
            )),
        }

        // 防御性清除：即使 inner_result 是 Err，也要清除标志
        if let Err(e) = self.clear_triggering_flag(agent_id).await {
            crate::logger::error(&format!(
                "[DEBUG trigger_agent] failed to clear is_triggering for agent_id={}: {}",
                agent_id, e
            ));
        }

        inner_result
    }

    async fn trigger_agent_inner(&self, agent_id: &str, pending: Vec<PendingMessage>, session_pages: HashMap<String, i32>, _snapshot_pages: HashMap<String, i32>) -> Result<(), String> {
        let inner_start = chrono::Utc::now().timestamp_millis();
        crate::logger::debug(&format!(
            "[DEBUG trigger_agent_inner] START agent_id={}, pending_count={}, session_pages={:?}",
            agent_id, pending.len(), session_pages
        ));

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

        crate::logger::debug(&format!(
            "[DEBUG trigger_agent_inner] agent_id={}, after_mute_filter pending_count={}",
            agent_id, pending.len()
        ));

        if pending.is_empty() {
            return Ok(());
        }

        // === 阶段 3：读取 agent 配置和 prompt ===
        let prompt_start = chrono::Utc::now().timestamp_millis();
        let (_agent, llm_config, prompt) = {
            let conn = self.db_state.0.lock().await;

            let agent = agent_repo::get_by_id(&conn, agent_id)
                .map_err(|e| e.to_string())?
                .ok_or("Agent not found")?;

            let llm_config = agent_repo::resolve_llm_config(&conn, &agent)
                .map_err(|e| format!("Agent LLM config error: {}", e))?;

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
                    page_index: p.page_index,
                })
                .collect();

            // 收集本次所有新消息的 message_id，用于 prompt 中的 [新] 标记
            let pending_ids: std::collections::HashSet<String> = pending
                .iter()
                .map(|p| p.message_id.clone())
                .collect();

            let trigger_msg = pending.first();
            let parts =
                PromptAssembler::assemble(
                    &conn,
                    agent_id,
                    trigger_msg.map(|m| m.session_id.as_str()),
                    trigger_msg.map(|m| m.page_index),
                    &messages_for_prompt,
                    &pending_ids,
                )
                .map_err(|e| e.to_string())?;

            crate::logger::debug(&format!(
                "[DEBUG trigger_agent_inner] agent_id={}, system_len={}, user_len={}, model={:?}, base_url={:?}",
                agent_id, parts.system.len(), parts.user.len(), llm_config.model_name, llm_config.base_url
            ));

            (agent, llm_config, parts)
        };
        let prompt_elapsed = chrono::Utc::now().timestamp_millis() - prompt_start;
        crate::logger::debug(&format!(
            "[DEBUG trigger_agent_inner] agent_id={}, prompt_build_elapsed_ms={}",
            agent_id, prompt_elapsed
        ));

        // === 阶段 4：LLM 调用（无锁）===
        let provider = OpenAiCompatibleProvider::new(
            llm_config.api_key,
            llm_config.base_url,
            llm_config.model_name,
            llm_config.temperature,
            llm_config.max_tokens,
        );

        self.emit(
            "agent_triggered",
            serde_json::json!({"agent_id": agent_id}),
        );

        // === 阶段 4：调用 LLM（多轮对话）===
        let llm_start = chrono::Utc::now().timestamp_millis();
        use crate::llm::conversation::LlmConversation;
        use crate::llm::tool::get_all_tool_schemas;

        let conversation = LlmConversation::new(provider, self.db_state.clone(), self.clone());
        let result = match conversation.run(
            &prompt.system,
            &prompt.user,
            get_all_tool_schemas(),
            5,
            agent_id,
            &session_pages,
        ).await {
            Ok(r) => r,
            Err(e) => {
                let llm_elapsed = chrono::Utc::now().timestamp_millis() - llm_start;
                crate::logger::error(&format!(
                    "[trigger_agent_inner] LLM conversation failed after {}ms: {}", llm_elapsed, e
                ));
                self.restore_pending(agent_id, pending).await;
                self.emit("agent_error", serde_json::json!({"agent_id": agent_id, "error": e}));
                return Ok(());
            }
        };
        let llm_elapsed = chrono::Utc::now().timestamp_millis() - llm_start;
        crate::logger::debug(&format!(
            "[DEBUG trigger_agent_inner] agent_id={}, LLM total_elapsed_ms={} rounds={} tool_calls={}",
            agent_id, llm_elapsed, result.total_rounds, result.executed_tool_calls.len()
        ));

        let agent_messages = result.messages;

        crate::logger::debug(&format!(
            "[DEBUG trigger_agent_inner] agent_id={}, agent_messages_count={}",
            agent_id, agent_messages.len()
        ));
        for (i, msg) in agent_messages.iter().enumerate() {
            let preview: String = msg.content.chars().take(80).collect();
            crate::logger::debug(&format!(
                "[DEBUG trigger_agent_inner] agent_id={}, agent_message[{}]: session_id={}, content_preview={}",
                agent_id, i, msg.session_id, preview
            ));
        }

        // === 阶段 6+7：统一后处理（emit, distribute, freeze check, counter） ===
        let post_start = chrono::Utc::now().timestamp_millis();
        self.handle_agent_response(agent_id, &agent_messages).await?;
        let post_elapsed = chrono::Utc::now().timestamp_millis() - post_start;
        crate::logger::debug(&format!(
            "[DEBUG trigger_agent_inner] agent_id={}, unified_post_elapsed_ms={}",
            agent_id, post_elapsed
        ));

        // 更新触发时间
        {
            let conn = self.db_state.0.lock().await;
            trigger_repo::update_trigger_time(&conn, agent_id)
                .map_err(|e| e.to_string())?;
        }

        let inner_elapsed = chrono::Utc::now().timestamp_millis() - inner_start;
        crate::logger::debug(&format!(
            "[DEBUG trigger_agent_inner] END agent_id={}, total_elapsed_ms={}", agent_id, inner_elapsed
        ));

        Ok(())
    }

    /// Unified post-LLM processing for all agent-produced messages.
    /// Called by both trigger_agent_inner (user/chain-triggered) and
    /// trigger_special (proactive/timer-triggered).
    async fn handle_agent_response(
        &self,
        agent_id: &str,
        agent_messages: &[Message],
    ) -> Result<(), String> {
        if agent_messages.is_empty() {
            crate::logger::debug(&format!(
                "[DEBUG handle_agent_response] agent_id={}, no messages, emitting agent_completed",
                agent_id
            ));
            self.emit("agent_completed", serde_json::json!({"agent_id": agent_id}));
            return Ok(());
        }

        // 1. Update agent_message_count for each session
        let mut session_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        {
            let conn = self.db_state.0.lock().await;
            for msg in agent_messages {
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
                session_ids.insert(msg.session_id.clone());
            }
        }

        // 2. Check freeze for each session
        for sid in &session_ids {
            if let Some(session_name) = self.check_and_freeze_if_needed(sid).await {
                self.emit("system_notice", serde_json::json!({
                    "content": format!("{} 已达到消息上限，自动对话已暂停。发送消息或点击重置以继续。", session_name)
                }));
            }
        }

        // 3. Emit new_message + distribute to other agents
        for msg in agent_messages {
            let session_exists: bool = {
                let conn = self.db_state.0.lock().await;
                conn.query_row(
                    "SELECT COUNT(*) FROM sessions WHERE id = ?1 AND is_deleted = 0",
                    [&msg.session_id],
                    |row| Ok(row.get::<_, i32>(0)? > 0),
                ).unwrap_or(false)
            };
            if !session_exists {
                continue;
            }

            crate::logger::debug(&format!(
                "[DEBUG handle_agent_response] agent_id={}, session_id={}, msg_page={}, emitting new_message message_id={}",
                agent_id, msg.session_id, msg.page_index, msg.id
            ));
            self.emit("new_message", msg);
            self.distribute_message(&msg.session_id, msg, agent_id).await?;
        }

        // 4. Emit completion
        crate::logger::debug(&format!(
            "[DEBUG handle_agent_response] agent_id={}, all messages emitted, emitting agent_completed",
            agent_id
        ));
        self.emit("agent_completed", serde_json::json!({"agent_id": agent_id}));

        Ok(())
    }

    async fn restore_pending(&self, agent_id: &str, pending: Vec<PendingMessage>) {
        if !pending.is_empty() {
            let count = pending.len();
            let mut unread = self.unread_messages.lock().await;
            let mut notifications = self.agent_notifications.lock().await;

            for mut msg in pending {
                msg.restored_from_failure = true;
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

            crate::logger::debug(&format!(
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

    #[allow(dead_code)]
    async fn call_llm<P: LlmProvider>(
        provider: &P,
        system_prompt: &str,
        messages: Vec<serde_json::Value>,
    ) -> Result<LlmResponse, String> {
        crate::logger::debug(&format!(
            "[DEBUG call_llm] system_prompt_len={}, messages_count={}",
            system_prompt.len(), messages.len()
        ));

        let tools = get_all_tool_schemas();
        let result = provider
            .chat(system_prompt, messages, tools)
            .await;

        match &result {
            Ok(_) => crate::logger::debug("[DEBUG call_llm] llm_call succeeded"),
            Err(e) => crate::logger::error(&format!("[DEBUG call_llm] llm_call failed: {}", e)),
        }

        result.map_err(|e| format!("LLM call failed: {}", e))
    }

    /// 调用 LLM 并自动重试，最多 3 次
    #[allow(dead_code)]
    async fn call_llm_with_retry<P: LlmProvider>(
        provider: &P,
        system_prompt: &str,
        messages: Vec<serde_json::Value>,
    ) -> Result<LlmResponse, String> {
        for attempt in 0..3 {
            match Self::call_llm(provider, system_prompt, messages.clone()).await {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    crate::logger::error(&format!(
                        "[DEBUG call_llm_with_retry] attempt {}/3 failed: {}", attempt + 1, e
                    ));
                    if attempt < 2 {
                        continue;
                    }
                    return Err(e);
                }
            }
        }
        unreachable!()
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

    pub async fn trigger_special(
        &self,
        agent_id: &str,
        context: SpecialTriggerContext,
    ) -> Result<(), String> {
        // 0. Check if already triggering (with stale timeout)
        {
            let conn = self.db_state.0.lock().await;
            let is_triggering: bool = conn
                .query_row(
                    "SELECT is_triggering FROM trigger_states WHERE agent_id = ?1",
                    [agent_id],
                    |row| Ok(row.get::<_, i32>(0)? != 0),
                )
                .unwrap_or(false);
            if is_triggering {
                let updated_at: i64 = conn
                    .query_row(
                        "SELECT updated_at FROM trigger_states WHERE agent_id = ?1",
                        [agent_id],
                        |row| row.get(0),
                    )
                    .unwrap_or(0);
                let now = chrono::Utc::now().timestamp_millis();
                if now - updated_at > 5 * 60 * 1000 {
                    crate::logger::warn(&format!(
                        "[trigger_special] agent_id={}, is_triggering=true but stale ({} min), resetting",
                        agent_id, (now - updated_at) / 60000
                    ));
                    conn.execute(
                        "UPDATE trigger_states SET is_triggering = 0, updated_at = ?1 WHERE agent_id = ?2",
                        (now, agent_id),
                    ).unwrap_or_default();
                } else {
                    crate::logger::debug(&format!(
                        "[trigger_special] agent_id={}, is_triggering=true, skip", agent_id
                    ));
                    return Ok(());
                }
            }
        }

        // 1. Set is_triggering (same pattern as trigger_agent)
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

        // 2. Get agent and provider
        let (_agent, llm_config) = {
            let conn = self.db_state.0.lock().await;
            let agent = agent_repo::get_by_id(&conn, agent_id)
                .map_err(|e| e.to_string())?
                .ok_or("Agent not found")?;
            let llm_config = agent_repo::resolve_llm_config(&conn, &agent)
                .map_err(|e| format!("Agent LLM config error: {}", e))?;
            (agent, llm_config)
        };

        let provider = OpenAiCompatibleProvider::new(
            llm_config.api_key,
            llm_config.base_url,
            llm_config.model_name,
            llm_config.temperature,
            llm_config.max_tokens,
        );

        // 3. Build base prompt using PromptAssembler
        let parts = {
            let conn = self.db_state.0.lock().await;
            PromptAssembler::assemble(
                &conn, agent_id, None, None, &[], &std::collections::HashSet::new()
            ).map_err(|e| e.to_string())?
        };

        // 4. Append special context layer
        let special_layer = match &context {
            SpecialTriggerContext::Timer { description, target_session_id } => {
                let mut s = format!("【定时任务触发】\n本次调用由定时任务发起。\n定时事件：{}", description);
                if target_session_id.is_some() {
                    s.push_str("\n你之前期望在指定会话中处理此事。");
                }
                s
            }
            SpecialTriggerContext::Proactive => {
                "【主动会话触发】\n本次调用由主动会话机制触发。\n你可以选择一个会话开始话题、延续之前的话题，或保持沉默。如果决定发起话题，请使用 send_message 工具；如果保持沉默，无需操作。".to_string()
            }
        };

        let full_user_prompt = format!("{}\n\n{}", parts.user, special_layer);

        // Log full prompt (including special layer)
        crate::logger::info(&format!(
            "[trigger_special] Full prompt for agent {} | context={:?} | system_length={} | user_length={}\n---SYSTEM START---\n{}\n---SYSTEM END---\n---USER START---\n{}\n---USER END---",
            agent_id,
            match &context {
                SpecialTriggerContext::Timer { description, .. } => format!("Timer: {}", description),
                SpecialTriggerContext::Proactive => "Proactive".to_string(),
            },
            parts.system.len(),
            full_user_prompt.len(),
            parts.system,
            full_user_prompt
        ));

        // 5. Call LLM and execute tools (wrapped for finally-style cleanup, with panic catching)
        let scheduler = self.clone();
        let agent_id_owned = agent_id.to_string();
        let system = parts.system.clone();
        let user_prompt = full_user_prompt.clone();
        let db_state = self.db_state.clone();
        let tools = get_all_tool_schemas();

        let inner_result = Self::catch_async_panic(async move {
            use crate::llm::conversation::LlmConversation;

            let conversation = LlmConversation::new(provider, db_state, scheduler.clone());
            let result = conversation.run(
                &system,
                &user_prompt,
                tools,
                5,
                &agent_id_owned,
                &HashMap::new(),
            ).await?;

            // Unified post-processing (emit, distribute, freeze check, counter)
            scheduler.handle_agent_response(&agent_id_owned, &result.messages).await?;

            // Log result
            crate::logger::info(&format!(
                "[trigger_special] LLM response for agent {} | content_len={} | tool_calls_count={} | total_rounds={} | content={:?} | emitted_messages={}",
                agent_id_owned,
                result.final_content.as_ref().map(|c| c.len()).unwrap_or(0),
                result.executed_tool_calls.len(),
                result.total_rounds,
                result.final_content,
                result.messages.len()
            ));
            Ok(())
        }).await;

        // 7. Clear is_triggering (always, even if inner failed)
        if let Err(e) = self.clear_triggering_flag(agent_id).await {
            crate::logger::error(&format!("[trigger_special] failed to clear is_triggering: {}", e));
        }

        inner_result
    }

    pub async fn start_timer_scan(self) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
        loop {
            interval.tick().await;
            self.scan_scheduled_tasks().await;
            self.scan_proactive_timers().await;
        }
    }

    async fn scan_scheduled_tasks(&self) {
        let conn = self.db_state.0.lock().await;
        let now = chrono::Utc::now().timestamp_millis();
        let tasks = match scheduled_task_repo::get_due_tasks(&conn, now) {
            Ok(t) => t,
            Err(e) => {
                crate::logger::error(&format!("[TimerScan] query failed: {}", e));
                return;
            }
        };
        drop(conn);

        for task in tasks {
            // 安静时段检查：如果在安静时段内，跳过本次触发（不更新状态，下次扫描继续检查）
            if self.is_in_quiet_hours().await {
                crate::logger::debug(&format!("[TimerScan] task_id={} skipped due to quiet hours", task.id));
                continue;
            }

            {
                let conn = self.db_state.0.lock().await;
                if task.task_type == "single" {
                    // 单次任务触发后直接删除，不显示为暂停
                    if let Err(e) = scheduled_task_repo::delete_task(&conn, &task.id) {
                        crate::logger::error(&format!("[TimerScan] delete single task failed: {}", e));
                    }
                } else {
                    let interval_ms = (task.interval_minutes.unwrap_or(60) as i64) * 60 * 1000;
                    // 修复级联触发：如果已经错过多个周期，从 now 开始计算下一次
                    let new_next = if now > task.next_trigger_at + interval_ms {
                        now + interval_ms
                    } else {
                        task.next_trigger_at + interval_ms
                    };
                    if let Err(e) = scheduled_task_repo::update_next_trigger(&conn, &task.id, new_next) {
                        crate::logger::error(&format!("[TimerScan] update next trigger failed: {}", e));
                    }
                }
            }

            let scheduler = self.clone();
            let task_clone = task.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = scheduler.trigger_special(
                    &task_clone.agent_id,
                    SpecialTriggerContext::Timer {
                        description: task_clone.description,
                        target_session_id: task_clone.target_session_id,
                    }
                ).await {
                    crate::logger::error(&format!("[TimerTrigger] failed: {}", e));
                }
            });
        }
    }

    async fn scan_proactive_timers(&self) {
        let now = chrono::Utc::now().timestamp_millis();
        let timers = self.proactive_timers.lock().await.clone();

        crate::logger::debug(&format!(
            "[Proactive] scan start | timers_count={} | now={}",
            timers.len(),
            chrono::DateTime::from_timestamp_millis(now).map(|d| d.format("%H:%M:%S").to_string()).unwrap_or_default()
        ));

        for (agent_id, next_at) in &timers {
            let remaining = next_at.saturating_sub(now);
            crate::logger::debug(&format!(
                "[Proactive] timer check | agent={} | next_at={} | remaining_ms={} | due={}",
                agent_id,
                chrono::DateTime::from_timestamp_millis(*next_at).map(|d| d.format("%H:%M:%S").to_string()).unwrap_or_default(),
                remaining,
                remaining == 0
            ));
        }

        for (agent_id, next_at) in timers {
            if next_at > now {
                continue;
            }

            if self.is_in_quiet_hours().await {
                crate::logger::info(&format!("[Proactive] agent={} due but in quiet hours, rescheduling", agent_id));
                self.reset_proactive_timer(&agent_id).await;
                continue;
            }

            crate::logger::info(&format!("[Proactive] agent={} timer DUE, triggering proactive session", agent_id));

            let scheduler = self.clone();
            let agent_id_clone = agent_id.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = scheduler.trigger_special(
                    &agent_id_clone,
                    SpecialTriggerContext::Proactive
                ).await {
                    crate::logger::error(&format!("[ProactiveTrigger] failed: {}", e));
                }
            });

            self.reset_proactive_timer(&agent_id).await;
        }
    }

    async fn is_in_quiet_hours(&self) -> bool {
        let conn = self.db_state.0.lock().await;
        let settings = match settings_repo::get_or_create_settings(&conn) {
            Ok(s) => s,
            Err(_) => return false,
        };
        drop(conn);

        if settings.quiet_hours_start < 0 || settings.quiet_hours_end < 0 {
            return false;
        }

        let now = chrono::Local::now();
        let current_minutes = (now.hour() * 60 + now.minute()) as i32;

        if settings.quiet_hours_start <= settings.quiet_hours_end {
            current_minutes >= settings.quiet_hours_start && current_minutes < settings.quiet_hours_end
        } else {
            current_minutes >= settings.quiet_hours_start || current_minutes < settings.quiet_hours_end
        }
    }

    pub async fn set_proactive_timer(&self, agent_id: &str, next_at: i64) {
        self.proactive_timers.lock().await.insert(agent_id.to_string(), next_at);
    }

    pub async fn reset_proactive_timer(&self, agent_id: &str) {
        let conn = self.db_state.0.lock().await;
        let agent = match agent_repo::get_by_id(&conn, agent_id) {
            Ok(Some(a)) => a,
            _ => return,
        };
        drop(conn);

        if !agent.proactive_enabled {
            self.proactive_timers.lock().await.remove(agent_id);
            crate::logger::info(&format!("[Proactive] agent={} proactive disabled, timer removed", agent_id));
            return;
        }

        let min_ms = agent.proactive_min_minutes as i64 * 60 * 1000;
        let max_ms = agent.proactive_max_minutes as i64 * 60 * 1000;
        let random_ms = rand::thread_rng().gen_range(min_ms..=max_ms);
        let next = chrono::Utc::now().timestamp_millis() + random_ms;
        self.proactive_timers.lock().await.insert(agent_id.to_string(), next);
        crate::logger::info(&format!(
            "[Proactive] agent={} timer reset | next_at={} (in {} min)",
            agent_id,
            chrono::DateTime::from_timestamp_millis(next).map(|d| d.format("%H:%M:%S").to_string()).unwrap_or_default(),
            random_ms / 60000
        ));
    }

    pub async fn init_proactive_timers(&self) {
        let conn = self.db_state.0.lock().await;
        let agents = match agent_repo::list_all(&conn) {
            Ok(a) => a.into_iter().filter(|a| a.proactive_enabled).collect::<Vec<_>>(),
            Err(_) => return,
        };
        drop(conn);

        let now = chrono::Utc::now().timestamp_millis();
        let mut timers = self.proactive_timers.lock().await;

        for agent in agents {
            let min_ms = agent.proactive_min_minutes as i64 * 60 * 1000;
            let max_ms = agent.proactive_max_minutes as i64 * 60 * 1000;
            let random_ms = rand::thread_rng().gen_range(min_ms..=max_ms);
            let next = now + random_ms;
            timers.insert(agent.id.clone(), next);
            crate::logger::info(&format!(
                "[Proactive] init timer for agent={} | next_at={} (in {} min)",
                agent.id,
                chrono::DateTime::from_timestamp_millis(next).map(|d| d.format("%H:%M:%S").to_string()).unwrap_or_default(),
                random_ms / 60000
            ));
        }
        crate::logger::info(&format!("[Proactive] initialized {} proactive timers", timers.len()));
    }

    async fn run_session_summary(&self, session_id: &str, page_index: i32) -> Result<(), String> {
        crate::logger::debug(&format!("[SessionSummary] start session={} page={}", session_id, page_index));

        let conn = self.db_state.0.lock().await;

        // 1. Find all agent participants in this session
        let agent_ids: Vec<String> = {
            let mut stmt = conn.prepare(
                "SELECT participant_id FROM group_members WHERE session_id = ?1 AND participant_type = 'agent' AND is_active = 1"
            ).map_err(|e| e.to_string())?;
            let rows = stmt.query_map([session_id], |row| {
                row.get::<_, String>(0)
            }).map_err(|e| e.to_string())?;
            rows.filter_map(|r| r.ok()).collect()
        };

        // Also check private sessions
        let private_agent_ids: Vec<String> = {
            let mut stmt = conn.prepare(
                "SELECT participant_1_id as pid FROM private_sessions WHERE session_id = ?1 AND participant_1_type = 'agent'
                 UNION
                 SELECT participant_2_id as pid FROM private_sessions WHERE session_id = ?1 AND participant_2_type = 'agent'"
            ).map_err(|e| e.to_string())?;
            let rows = stmt.query_map([session_id], |row| {
                row.get::<_, String>(0)
            }).map_err(|e| e.to_string())?;
            rows.filter_map(|r| r.ok()).collect()
        };

        drop(conn);

        let mut all_agents = agent_ids;
        all_agents.extend(private_agent_ids);
        all_agents.sort();
        all_agents.dedup();

        crate::logger::debug(&format!("[SessionSummary] found {} agents", all_agents.len()));

        // 2. For each agent, run summary if memory_enabled
        for agent_id in all_agents {
            let conn = self.db_state.0.lock().await;
            let agent = match agent_repo::get_by_id(&conn, &agent_id) {
                Ok(Some(a)) => a,
                _ => continue,
            };

            if !agent.memory_enabled {
                crate::logger::debug(&format!("[SessionSummary] agent={} memory disabled, skipping", agent_id));
                continue;
            }

            // Check agent has valid LLM config
            let llm_config = match agent_repo::resolve_llm_config(&conn, &agent) {
                Ok(c) => c,
                Err(e) => {
                    crate::logger::warn(&format!("[SessionSummary] agent={} missing LLM config: {}, skipping", agent_id, e));
                    continue;
                }
            };

            // Get history limit for this session
            let history_limit: i32 = conn.query_row(
                "SELECT COALESCE(history_limit, 50) FROM session_settings WHERE session_id = ?1",
                [session_id],
                |row| row.get(0),
            ).unwrap_or(50);

            // Get messages and participants inside a scoped block so conn is released before await
            let (messages, participants_text) = {
                let mut stmt = conn.prepare(
                    "SELECT m.id, m.session_id, m.sender_type, m.sender_id, m.content, m.created_at,
                            m.message_type, m.tool_call_data, m.generation_info, m.is_deleted,
                            COALESCE(a.name, up.name, CASE WHEN m.sender_type = 'user' THEN '用户' ELSE '未知' END) as sender_name,
                            m.page_index
                     FROM messages m
                     LEFT JOIN agents a ON m.sender_type = 'agent' AND m.sender_id = a.id AND a.is_deleted = 0
                     LEFT JOIN user_personas up ON m.sender_type = 'user' AND m.sender_id = up.id
                     WHERE m.session_id = ?1 AND m.page_index = ?2 AND m.is_deleted = 0
                     ORDER BY m.created_at DESC
                     LIMIT ?3"
                ).map_err(|e| e.to_string())?;

                let rows = stmt.query_map(
                    rusqlite::params![session_id, page_index, history_limit],
                    |row| {
                        Ok(crate::models::message::Message {
                            id: row.get(0)?,
                            session_id: row.get(1)?,
                            sender_type: row.get(2)?,
                            sender_id: row.get(3)?,
                            content: row.get(4)?,
                            created_at: row.get(5)?,
                            message_type: row.get(6)?,
                            tool_call_data: row.get(7)?,
                            generation_info: row.get(8)?,
                            is_deleted: row.get::<_, i32>(9)? != 0,
                            sender_name: row.get(10)?,
                            sender_avatar: None,
                            page_index: row.get(11)?,
                        })
                    }
                ).map_err(|e| e.to_string())?;

                let mut messages: Vec<crate::models::message::Message> = rows.filter_map(|r| r.ok()).collect();
                messages.reverse(); // chronological order
                drop(stmt);

                // Build participants text
                let participants = PromptAssembler::get_participants(&conn, &agent_id)
                    .map_err(|e| e.to_string())?;
                let mut participants_text = String::new();
                for item in participants {
                    participants_text.push_str(&format!(
                        "- {}（{}）：{}\n",
                        item.target_name, item.target_label, item.target_simplified_persona
                    ));
                    if !item.relationship_text.is_empty() {
                        participants_text.push_str(&format!("  [印象]：{}\n", item.relationship_text));
                    }
                    if !item.memory_text.is_empty() {
                        participants_text.push_str(&format!("  [记忆]：{}\n", item.memory_text));
                    }
                }

                (messages, participants_text)
            };

            drop(conn);

            if messages.is_empty() {
                crate::logger::debug(&format!("[SessionSummary] agent={} no messages, skipping", agent_id));
                continue;
            }

            // Build session messages text
            let mut session_messages_text = String::new();
            for msg in &messages {
                let time = PromptAssembler::format_time(msg.created_at);
                session_messages_text.push_str(&format!("[{}] {}: {}\n", time, msg.sender_name, msg.content));
            }

            let long_term_memory = agent.long_term_memory.as_deref().unwrap_or("");
            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

            let system_prompt = prompt_templates::SUMMARY_SYSTEM_PROMPT
                .replace("{current_time}", &now)
                .replace("{detailed_persona}", &agent.detailed_persona)
                .replace("{long_term_memory}", long_term_memory)
                .replace("{participants}", &participants_text)
                .replace("{session_messages}", &session_messages_text);

            let provider = OpenAiCompatibleProvider::new(
                llm_config.api_key,
                llm_config.base_url,
                llm_config.model_name,
                llm_config.temperature,
                llm_config.max_tokens,
            );

            let tools = vec![
                update_memory_tool_schema(),
                update_relationship_tool_schema(),
            ];

            let messages_json = vec![serde_json::json!({
                "role": "user",
                "content": "请回顾本次对话，判断是否有值得保存到记忆中的信息。"
            })];

            crate::logger::debug(&format!("[SessionSummary] calling LLM for agent={}", agent_id));

            let response = match provider.chat(&system_prompt, messages_json, tools).await {
                Ok(resp) => resp,
                Err(e) => {
                    crate::logger::error(&format!("[SessionSummary] agent={} LLM call failed: {}", agent_id, e));
                    continue;
                }
            };

            if !response.tool_calls.is_empty() {
                let mut session_pages = HashMap::new();
                session_pages.insert(session_id.to_string(), page_index);
        let executor = ToolExecutor::new(self.db_state.clone(), self.clone());
                if let Err(e) = executor.execute(&agent_id, response.tool_calls, &session_pages).await {
                    crate::logger::error(&format!("[SessionSummary] agent={} tool execution failed: {}", agent_id, e));
                } else {
                    crate::logger::debug(&format!("[SessionSummary] agent={} tools executed successfully", agent_id));
                }
            } else {
                crate::logger::debug(&format!("[SessionSummary] agent={} no tools called", agent_id));
            }
        }

        crate::logger::debug(&format!("[SessionSummary] complete session={} page={}", session_id, page_index));
        Ok(())
    }

    async fn run_overflow_summary(&self, session_id: &str) -> Result<(), String> {
        // Prevent concurrent runs for the same session
        {
            let mut running = self.running_summaries.lock().await;
            if running.contains(session_id) {
                crate::logger::debug(&format!("[OverflowSummary] session={} already running, skipping", session_id));
                return Ok(());
            }
            running.insert(session_id.to_string());
        }

        let result = self.do_run_overflow_summary(session_id).await;

        {
            let mut running = self.running_summaries.lock().await;
            running.remove(session_id);
        }

        result
    }

    async fn do_run_overflow_summary(&self, session_id: &str) -> Result<(), String> {
        crate::logger::debug(&format!("[OverflowSummary] start session={}", session_id));

        let conn = self.db_state.0.lock().await;

        // Get current page index
        let page_index: i32 = conn.query_row(
            "SELECT COALESCE(ps.current_chat_page, gs.current_chat_page, 0)
             FROM sessions s
             LEFT JOIN private_sessions ps ON s.id = ps.session_id
             LEFT JOIN group_sessions gs ON s.id = gs.session_id
             WHERE s.id = ?1",
            [session_id],
            |row| row.get(0),
        ).unwrap_or(0);

        // Get threshold and last index
        let (threshold, last_index): (i32, i32) = conn.query_row(
            "SELECT COALESCE(overflow_summary_threshold, 50), COALESCE(last_overflow_summary_index, 0)
             FROM session_settings WHERE session_id = ?1",
            [session_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap_or((50, 0));

        if threshold <= 0 {
            crate::logger::debug(&format!("[OverflowSummary] session={} threshold=0, skipping", session_id));
            return Ok(());
        }

        // Count total messages on current page
        let total_messages: i32 = conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE session_id = ?1 AND page_index = ?2 AND is_deleted = 0",
            rusqlite::params![session_id, page_index],
            |row| row.get(0),
        ).unwrap_or(0);

        if total_messages - last_index < threshold {
            crate::logger::debug(&format!("[OverflowSummary] session={} total={} last={} threshold={} not met", session_id, total_messages, last_index, threshold));
            return Ok(());
        }

        // Query messages: OFFSET=last_index, LIMIT=threshold
            let messages: Vec<crate::models::message::Message> = {
                let mut stmt = conn.prepare(
                    "SELECT m.id, m.session_id, m.sender_type, m.sender_id, m.content, m.created_at,
                            m.message_type, m.tool_call_data, m.generation_info, m.is_deleted,
                            COALESCE(a.name, up.name, CASE WHEN m.sender_type = 'user' THEN '用户' ELSE '未知' END) as sender_name,
                            m.page_index
                     FROM messages m
                     LEFT JOIN agents a ON m.sender_type = 'agent' AND m.sender_id = a.id AND a.is_deleted = 0
                     LEFT JOIN user_personas up ON m.sender_type = 'user' AND m.sender_id = up.id
                     WHERE m.session_id = ?1 AND m.page_index = ?2 AND m.is_deleted = 0
                     ORDER BY m.created_at ASC
                     LIMIT ?3 OFFSET ?4"
                ).map_err(|e| e.to_string())?;

            let rows = stmt.query_map(
                rusqlite::params![session_id, page_index, threshold, last_index],
                |row| {
                    Ok(crate::models::message::Message {
                        id: row.get(0)?,
                        session_id: row.get(1)?,
                        sender_type: row.get(2)?,
                        sender_id: row.get(3)?,
                        content: row.get(4)?,
                        created_at: row.get(5)?,
                        message_type: row.get(6)?,
                        tool_call_data: row.get(7)?,
                        generation_info: row.get(8)?,
                        is_deleted: row.get::<_, i32>(9)? != 0,
                        sender_name: row.get(10)?,
                        sender_avatar: None,
                        page_index: row.get(11)?,
                    })
                }
            ).map_err(|e| e.to_string())?;

            rows.filter_map(|r| r.ok()).collect()
        };
        drop(conn);

        if messages.is_empty() {
            return Ok(());
        }

        // Build session messages text
        let mut session_messages_text = String::new();
        for msg in &messages {
            let time = PromptAssembler::format_time(msg.created_at);
            session_messages_text.push_str(&format!("[{}] {}: {}\n", time, msg.sender_name, msg.content));
        }

        // Find agents and run summary (same pattern as run_session_summary)
        let conn = self.db_state.0.lock().await;
        let agent_ids: Vec<String> = {
            let mut stmt = conn.prepare(
                "SELECT participant_id FROM group_members WHERE session_id = ?1 AND participant_type = 'agent' AND is_active = 1"
            ).map_err(|e| e.to_string())?;
            let rows = stmt.query_map([session_id], |row| row.get::<_, String>(0)).map_err(|e| e.to_string())?;
            rows.filter_map(|r| r.ok()).collect()
        };
        let private_agent_ids: Vec<String> = {
            let mut stmt = conn.prepare(
                "SELECT participant_1_id as pid FROM private_sessions WHERE session_id = ?1 AND participant_1_type = 'agent'
                 UNION
                 SELECT participant_2_id as pid FROM private_sessions WHERE session_id = ?1 AND participant_2_type = 'agent'"
            ).map_err(|e| e.to_string())?;
            let rows = stmt.query_map([session_id], |row| row.get::<_, String>(0)).map_err(|e| e.to_string())?;
            rows.filter_map(|r| r.ok()).collect()
        };
        drop(conn);

        let mut all_agents = agent_ids;
        all_agents.extend(private_agent_ids);
        all_agents.sort();
        all_agents.dedup();

        for agent_id in all_agents {
            let conn = self.db_state.0.lock().await;
            let agent = match agent_repo::get_by_id(&conn, &agent_id) {
                Ok(Some(a)) => a,
                _ => continue,
            };
            if !agent.memory_enabled { continue; }
            let llm_config = match agent_repo::resolve_llm_config(&conn, &agent) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let participants = PromptAssembler::get_participants(&conn, &agent_id)
                .map_err(|e| e.to_string())?;
            drop(conn);

            let mut participants_text = String::new();
            for item in participants {
                participants_text.push_str(&format!("- {}（{}）：{}\n", item.target_name, item.target_label, item.target_simplified_persona));
                if !item.relationship_text.is_empty() {
                    participants_text.push_str(&format!("  [印象]：{}\n", item.relationship_text));
                }
                if !item.memory_text.is_empty() {
                    participants_text.push_str(&format!("  [记忆]：{}\n", item.memory_text));
                }
            }

            let long_term_memory = agent.long_term_memory.as_deref().unwrap_or("");
            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

            let provider = OpenAiCompatibleProvider::new(
                llm_config.api_key,
                llm_config.base_url,
                llm_config.model_name,
                llm_config.temperature,
                llm_config.max_tokens,
            );

            use crate::llm::conversation::LlmConversation;

            let system = format!(
                "你是一个记忆整理助手。你的任务是在一次聊天会话结束后，回顾对话内容，判断是否有值得长期保存的信息。\n\n当前时间：{}\n\n## 你的角色设定\n{}\n\n## 可用工具\n- update_memory：更新你的记忆\n- update_relationship：更新关系描述\n\n## 任务\n请仔细阅读本次对话记录，判断是否有值得保存的信息。如果有，请使用工具更新。如果没有，可以不调用任何工具。",
                now, agent.detailed_persona
            );

            let user = format!(
                "## 关于你的记忆\n{}\n\n## 你认识的参与者\n{}\n\n## 本次对话记录\n{}\n\n请回顾本次对话，判断是否有值得保存到记忆中的信息。",
                long_term_memory, participants_text, session_messages_text
            );

            let tools = vec![update_memory_tool_schema(), update_relationship_tool_schema()];
            let sched = self.clone();
            let agent_id_owned = agent_id.clone();
            let db = self.db_state.clone();
            let sid = session_id.to_string();

            let _ = Self::catch_async_panic(async move {
                let conversation = LlmConversation::new(provider, db, sched);
                let mut sp = HashMap::new();
                sp.insert(sid, page_index);
                conversation.run(&system, &user, tools, 5, &agent_id_owned, &sp).await
            }).await;
        }

        // Update last_overflow_summary_index
        let conn = self.db_state.0.lock().await;
        let _ = conn.execute(
            "UPDATE session_settings SET last_overflow_summary_index = last_overflow_summary_index + ?1 WHERE session_id = ?2",
            rusqlite::params![threshold, session_id],
        );
        drop(conn);

        crate::logger::debug(&format!("[OverflowSummary] complete session={}", session_id));
        Ok(())
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

    #[cfg(test)]
    mod tests {
        use super::*;
        use rusqlite::Connection;
        use crate::db::session as session_repo;
        use crate::db::schema::{MIGRATION_V1, MIGRATION_V2, MIGRATION_V3, MIGRATION_V4, MIGRATION_V5, MIGRATION_V6, MIGRATION_V7, MIGRATION_V8};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use async_trait::async_trait;
        use crate::llm::provider::LlmProvider;
        use crate::llm::tool::LlmResponse;

        struct MockProvider {
            call_count: AtomicUsize,
            responses: Vec<Result<LlmResponse, String>>,
        }

        impl MockProvider {
            fn new(responses: Vec<Result<LlmResponse, String>>) -> Self {
                Self {
                    call_count: AtomicUsize::new(0),
                    responses,
                }
            }
        }

        #[async_trait]
        impl LlmProvider for MockProvider {
            async fn chat(
                &self,
                _system_prompt: &str,
                _messages: Vec<serde_json::Value>,
                _tools: Vec<serde_json::Value>,
            ) -> Result<LlmResponse, String> {
                let idx = self.call_count.fetch_add(1, Ordering::SeqCst);
                self.responses.get(idx).cloned().unwrap_or(Err("exhausted".to_string()))
            }

            async fn chat_raw(
                &self,
                _messages: Vec<serde_json::Value>,
                _tools: Vec<serde_json::Value>,
            ) -> Result<LlmResponse, String> {
                let idx = self.call_count.fetch_add(1, Ordering::SeqCst);
                self.responses.get(idx).cloned().unwrap_or(Err("exhausted".to_string()))
            }
        }

    fn init_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = OFF;", []).unwrap();
        conn.execute_batch(MIGRATION_V1).unwrap();
        conn.execute_batch(MIGRATION_V2).unwrap();
        conn.execute_batch(MIGRATION_V3).unwrap();
        conn.execute_batch(MIGRATION_V4).unwrap();
        conn.execute_batch(MIGRATION_V5).unwrap();
        conn.execute_batch(MIGRATION_V6).unwrap();
        conn.execute_batch(MIGRATION_V7).unwrap();
        conn.execute_batch(MIGRATION_V8).unwrap();
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

    fn create_test_group_session(conn: &Connection, agent_ids: &[String]) -> String {
        session_repo::create_group_session(conn, "Test Group", agent_ids).unwrap().id
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
            page_index: 0,
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
    async fn test_frozen_session_does_not_block_distribution() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1");
        let session_id = create_test_private_session(&conn, "agent-1");
        let db_state = make_db_state(conn);

        let scheduler = Scheduler::new(db_state);
        scheduler.frozen_sessions.lock().await.insert(session_id.clone());

        let message = create_test_message(&session_id, "user", "user", "Hello", 1000);
        scheduler.distribute_message(&session_id, &message, "user").await.unwrap();

        // Bug 2 fix: frozen session should NOT block distribution.
        // Messages always land in the inbox; freezing only pauses automatic triggers.
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
                    message_id: "msg-1".to_string(),
                    session_id: session_id.clone(),
                    sender_type: "user".to_string(),
                    sender_id: "user".to_string(),
                    content: "Hello".to_string(),
                    created_at: 1000,
                    page_index: 0,
                    restored_from_failure: false,
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
                        message_id: "msg-2".to_string(),
                        session_id: session_id.clone(),
                        sender_type: "user".to_string(),
                        sender_id: "user".to_string(),
                        content: "Second".to_string(),
                        created_at: 2000,
                        page_index: 0,
                        restored_from_failure: false,
                    },
                    PendingMessage {
                        message_id: "msg-1".to_string(),
                        session_id: session_id.clone(),
                        sender_type: "user".to_string(),
                        sender_id: "user".to_string(),
                        content: "First".to_string(),
                        created_at: 1000,
                        page_index: 0,
                        restored_from_failure: false,
                    },
                    PendingMessage {
                        message_id: "msg-3".to_string(),
                        session_id: session_id.clone(),
                        sender_type: "user".to_string(),
                        sender_id: "user".to_string(),
                        content: "Third".to_string(),
                        created_at: 3000,
                        page_index: 0,
                        restored_from_failure: false,
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

    #[tokio::test]
    async fn test_cancel_session_clears_memory_state() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1");
        create_test_agent(&conn, "agent-2");
        let session_id = create_test_group_session(&conn, &["agent-1".into(), "agent-2".into()]);
        let db_state = make_db_state(conn);

        let scheduler = Scheduler::new(db_state);
        let message = create_test_message(&session_id, "user", "user", "Hello", 1000);
        scheduler.distribute_message(&session_id, &message, "user").await.unwrap();

        // Verify unread and notifications exist before cancel
        {
            let unread = scheduler.unread_messages.lock().await;
            assert!(unread.contains_key(&session_id));
        }

        // Cancel the session
        scheduler.cancel_session(&session_id).await;

        // Verify cleared
        let unread = scheduler.unread_messages.lock().await;
        assert!(!unread.contains_key(&session_id));

        let notifications = scheduler.agent_notifications.lock().await;
        if let Some(sessions) = notifications.get("agent-1") {
            assert!(!sessions.contains(&session_id));
        }
        if let Some(sessions) = notifications.get("agent-2") {
            assert!(!sessions.contains(&session_id));
        }
    }

    #[test]
    fn test_insert_message_with_bound_page_index() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1");
        let session_id = create_test_private_session(&conn, "agent-1");

        // Update current_chat_page to 5
        conn.execute(
            "UPDATE private_sessions SET current_chat_page = 5 WHERE session_id = ?1",
            [&session_id],
        ).unwrap();

        // Insert with bound page_index = 3
        let msg = crate::db::message::insert_message(
            &conn, &session_id, "agent", "agent-1", "Hello", "text", Some(3)
        ).unwrap();
        assert_eq!(msg.page_index, 3);

        // Verify in DB
        let db_page: i32 = conn.query_row(
            "SELECT page_index FROM messages WHERE id = ?1",
            [&msg.id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(db_page, 3);

        // Insert without bound page_index (should use current_chat_page = 5)
        let msg2 = crate::db::message::insert_message(
            &conn, &session_id, "agent", "agent-1", "Hello2", "text", None
        ).unwrap();
        assert_eq!(msg2.page_index, 5);
    }

    #[tokio::test]
    async fn test_distribute_message_agent_agent_session_symmetric() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1");
        create_test_agent(&conn, "agent-2");
        let session_id = session_repo::create_agent_agent_session(&conn, "agent-1", "agent-2").unwrap().id;
        let db_state = make_db_state(conn);

        let scheduler = Scheduler::new(db_state);

        // Agent-1 sends message
        let msg1 = create_test_message(&session_id, "agent", "agent-1", "Hello from 1", 1000);
        scheduler.distribute_message(&session_id, &msg1, "agent-1").await.unwrap();

        let unread = scheduler.unread_messages.lock().await;
        let session_unread = unread.get(&session_id).unwrap();
        assert!(session_unread.contains_key("agent-2"));
        assert!(!session_unread.contains_key("agent-1"));
        assert_eq!(session_unread.get("agent-2").unwrap().len(), 1);
        drop(unread);

        // Agent-2 sends message
        let msg2 = create_test_message(&session_id, "agent", "agent-2", "Hello from 2", 2000);
        scheduler.distribute_message(&session_id, &msg2, "agent-2").await.unwrap();

        let unread = scheduler.unread_messages.lock().await;
        let session_unread = unread.get(&session_id).unwrap();
        assert!(session_unread.contains_key("agent-1"));
        assert_eq!(session_unread.get("agent-1").unwrap().len(), 1);
        assert_eq!(session_unread.get("agent-2").unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_call_llm_retry_succeeds_on_third_attempt() {
        let provider = MockProvider::new(vec![
            Err("network error".to_string()),
            Err("timeout".to_string()),
            Ok(LlmResponse {
                content: Some("ok".to_string()),
                tool_calls: vec![],
                usage: None,
            }),
        ]);

        let result = Scheduler::call_llm_with_retry(&provider, "", vec![]).await;
        assert!(result.is_ok());
        assert_eq!(provider.call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_call_llm_retry_fails_after_three_attempts() {
        let provider = MockProvider::new(vec![
            Err("network error".to_string()),
            Err("timeout".to_string()),
            Err("rate limit".to_string()),
        ]);

        let result = Scheduler::call_llm_with_retry(&provider, "", vec![]).await;
        assert!(result.is_err());
        assert_eq!(provider.call_count.load(Ordering::SeqCst), 3);
    }
}
