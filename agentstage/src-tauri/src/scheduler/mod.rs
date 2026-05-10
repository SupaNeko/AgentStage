use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{AppHandle, Emitter};
use serde::Serialize;

use crate::db::agent as agent_repo;
use crate::db::message as message_repo;
use crate::db::session as session_repo;
use crate::db::settings as settings_repo;
use crate::db::trigger_state as trigger_repo;
use crate::db::connection::DbState;
use crate::llm::openai::OpenAiCompatibleProvider;
use crate::llm::provider::LlmProvider;
use crate::llm::prompt::PromptAssembler;
use crate::llm::tool::{send_message_tool_schema, LlmResponse};
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
            queue
                .entry(agent_id.clone())
                .or_insert_with(Vec::new)
                .push(PendingMessage::from(message.clone()));
        }

        // 尝试触发
        self.try_trigger_agent(&agent_id).await?;

        Ok(())
    }

    async fn try_trigger_agent(&self, agent_id: &str) -> Result<(), String> {
        let (last_trigger, interval_ms) = {
            let conn = self.db_state.0.lock().await;
            let last_trigger = trigger_repo::get_last_trigger_time(&conn, agent_id)
                .map_err(|e| e.to_string())?;
            let settings = settings_repo::get_or_create_settings(&conn)
                .map_err(|e| e.to_string())?;
            (last_trigger, settings.global_min_trigger_interval as i64 * 1000)
        };

        let now = chrono::Utc::now().timestamp_millis();

        if now - last_trigger >= interval_ms {
            // 间隔满足，立即触发
            self.trigger_agent(agent_id).await
        } else {
            // 间隔未满足，消息留在 pending_queue 中，等待后台扫描任务触发
            Ok(())
        }
    }

    pub async fn trigger_agent(&self, agent_id: &str) -> Result<(), String> {
        // 阶段 1：读取所有数据（持有 conn 锁）
        let (agent, prompt) = {
            let conn = self.db_state.0.lock().await;

            // 取出 pending 消息
            let pending = {
                let mut queue = self.pending_queue.lock().await;
                queue.remove(agent_id).unwrap_or_default()
            };

            if pending.is_empty() {
                return Ok(());
            }

            // 获取角色配置
            let agent = agent_repo::get_by_id(&conn, agent_id)
                .map_err(|e| e.to_string())?
                .ok_or("Agent not found")?;

            // 组装 Prompt
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

            // 更新触发时间
            trigger_repo::update_trigger_time(&conn, agent_id)
                .map_err(|e| e.to_string())?;

            (agent, prompt)
        }; // 释放 conn 锁

        // 阶段 2：LLM 调用（无锁）
        let api_key = if let Some(encrypted) = agent.api_key_encrypted {
            crate::crypto::decrypt(&encrypted)
                .map_err(|e| format!("Failed to decrypt API key: {}", e))?
        } else {
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

        let response =
            match Self::call_llm_with_retry(&provider, &prompt, vec![]).await {
                Ok(resp) => resp,
                Err(e) => {
                    self.emit(
                        "agent_error",
                        serde_json::json!({"agent_id": agent_id, "error": e}),
                    );
                    return Ok(());
                }
            };

        // 阶段 3：写入结果（重新获取 conn 锁）
        if let Some(tool_call) =
            response.tool_calls.into_iter().find(|tc| tc.name == "send_message")
        {
            let args: serde_json::Value = serde_json::from_str(&tool_call.arguments)
                .map_err(|e| format!("Invalid tool arguments: {}", e))?;

            let target_id = args["target_id"].as_str().unwrap_or("");
            let content = args["content"].as_str().unwrap_or("");

            if !target_id.is_empty() && !content.is_empty() {
                let conn = self.db_state.0.lock().await;

                let agent_msg = message_repo::insert_message(
                    &conn, target_id, "agent", agent_id, content, "text",
                )
                .map_err(|e| e.to_string())?;

                // 递增消息计数器
                conn.execute(
                    "UPDATE private_sessions SET agent_message_count = agent_message_count + 1 WHERE session_id = ?1",
                    [target_id],
                )
                .map_err(|e| e.to_string())?;

                // 更新会话最后消息预览
                let preview = if content.len() > 100 {
                    format!("{}...", &content[..100])
                } else {
                    content.to_string()
                };
                let _ =
                    session_repo::update_session_last_message(&conn, target_id, &preview);

                // 获取 target 会话的 agent_id（用于继续触发链）
                let target_agent_id: Option<String> = conn.query_row(
                    "SELECT agent_id FROM private_sessions WHERE session_id = ?1",
                    [target_id],
                    |row| row.get(0),
                ).ok();

                drop(conn);

                self.emit("new_message", &agent_msg);

                // 把消息加入对方角色的 pending_queue，由后台扫描任务处理触发
                if let Some(target_agent_id) = target_agent_id {
                    let mut queue = self.pending_queue.lock().await;
                    queue.entry(target_agent_id)
                        .or_insert_with(Vec::new)
                        .push(PendingMessage::from(agent_msg));
                }
            }
        }

        self.emit(
            "agent_completed",
            serde_json::json!({"agent_id": agent_id}),
        );

        Ok(())
    }

    async fn call_llm_with_retry(
        provider: &OpenAiCompatibleProvider,
        system_prompt: &str,
        messages: Vec<serde_json::Value>,
    ) -> Result<LlmResponse, String> {
        let tools = vec![send_message_tool_schema()];
        let mut last_error = String::new();

        for attempt in 0..3 {
            match provider
                .chat(system_prompt, messages.clone(), tools.clone())
                .await
            {
                Ok(response) => return Ok(response),
                Err(e) => {
                    last_error = e;
                    if attempt < 2 {
                        let delay = 2u64.pow(attempt as u32);
                        tokio::time::sleep(tokio::time::Duration::from_secs(delay))
                            .await;
                    }
                }
            }
        }

        Err(format!(
            "LLM call failed after 3 retries: {}",
            last_error
        ))
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
                let conn = self.db_state.0.lock().await;
                let last_trigger =
                    trigger_repo::get_last_trigger_time(&conn, &agent_id).unwrap_or(0);
                let settings =
                    settings_repo::get_or_create_settings(&conn).unwrap_or_default();
                drop(conn);

                let now = chrono::Utc::now().timestamp_millis();
                if now - last_trigger >= settings.global_min_trigger_interval as i64 * 1000
                {
                    let _ = self.trigger_agent(&agent_id).await;
                }
            }
        }
    }
}
