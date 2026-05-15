use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::db::connection::DbState;
use crate::db::session as session_repo;
use crate::db::message as message_repo;
use crate::models::message::Message;

pub fn split_br_tags(content: &str) -> Vec<String> {
    content.split("<br/>")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn send_message_tool_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "send_message",
            "description": "向指定会话发送一条消息。你可以在 content 中使用 <br/> 标签进行换行。target_id 必须是系统提供的 session_id，不能使用会话名称或其他 ID。",
            "parameters": {
                "type": "object",
                "properties": {
                    "target_type": {
                        "type": "string",
                        "enum": ["private", "group"],
                        "description": "目标会话类型"
                    },
                    "target_id": {
                        "type": "string",
                        "description": "目标会话的 session_id（必须使用系统提供的完整 ID）"
                    },
                    "content": {
                        "type": "string",
                        "description": "消息内容"
                    }
                },
                "required": ["target_type", "target_id", "content"]
            }
        }
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<serde_json::Value>,
}

#[derive(Debug)]
pub enum ToolError {
    InvalidArguments(String),
    EmptyContent,
    TargetNotFound(String),
    DatabaseError(String),
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolError::InvalidArguments(s) => write!(f, "工具参数格式错误: {}", s),
            ToolError::EmptyContent => write!(f, "工具调用内容为空"),
            ToolError::TargetNotFound(s) => write!(f, "找不到目标会话: {}", s),
            ToolError::DatabaseError(s) => write!(f, "保存消息失败: {}", s),
        }
    }
}

#[derive(Clone)]
pub struct ToolExecutor {
    db_state: DbState,
}

impl ToolExecutor {
    pub fn new(db_state: DbState) -> Self {
        Self { db_state }
    }

    pub async fn execute(
        &self,
        agent_id: &str,
        tool_calls: Vec<ToolCall>,
        session_pages: &HashMap<String, i32>,
    ) -> Result<Vec<Message>, ToolError> {
        crate::logger::backend("DEBUG", &format!(
            "[DEBUG ToolExecutor::execute] START agent_id={}, tool_calls_count={}",
            agent_id, tool_calls.len()
        ));

        let mut results = Vec::new();

        for (i, tc) in tool_calls.iter().enumerate() {
            crate::logger::backend("DEBUG", &format!(
                "[DEBUG ToolExecutor::execute] processing tool_call[{}]: name={}, args={}",
                i, tc.name, tc.arguments
            ));
            match tc.name.as_str() {
                "send_message" => {
                    let msgs = self.execute_send_message(agent_id, &tc.arguments, session_pages).await?;
                    for msg in &msgs {
                        crate::logger::backend("DEBUG", &format!(
                            "[DEBUG ToolExecutor::execute] tool_call[{}] returned message_id={}",
                            i, msg.id
                        ));
                    }
                    results.extend(msgs);
                }
                _ => {
                    crate::logger::backend("WARN", &format!("[DEBUG ToolExecutor::execute] Unknown tool call: {}", tc.name));
                }
            }
        }

        crate::logger::backend("DEBUG", &format!(
            "[DEBUG ToolExecutor::execute] END agent_id={}, results_count={}",
            agent_id, results.len()
        ));

        Ok(results)
    }

    async fn execute_send_message(
        &self,
        agent_id: &str,
        arguments: &str,
        session_pages: &HashMap<String, i32>,
    ) -> Result<Vec<Message>, ToolError> {
        crate::logger::backend("DEBUG", &format!(
            "[DEBUG ToolExecutor::execute_send_message] START agent_id={}, args_raw={}",
            agent_id, arguments
        ));

        let args: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| ToolError::InvalidArguments(e.to_string()))?;

        let raw_target_id = args["target_id"].as_str().unwrap_or("");
        let content = args["content"].as_str().unwrap_or("");

        crate::logger::backend("DEBUG", &format!(
            "[DEBUG ToolExecutor::execute_send_message] parsed raw_target_id={}, content_len={}",
            raw_target_id, content.len()
        ));

        if content.is_empty() {
            crate::logger::backend("WARN", &format!(
                "[DEBUG ToolExecutor::execute_send_message] Empty content, aborting"
            ));
            return Err(ToolError::EmptyContent);
        }

        // 自动映射 target_id
        let target_id = self.resolve_target_id(agent_id, raw_target_id).await?;
        crate::logger::backend("DEBUG", &format!(
            "[DEBUG ToolExecutor::execute_send_message] resolved target_id={}", target_id
        ));

        // 使用触发时绑定的 page_index，避免 reset 后的页面漂移
        let bound_page = session_pages.get(&target_id).copied();
        crate::logger::backend("DEBUG", &format!(
            "[DEBUG ToolExecutor::execute_send_message] bound_page={:?} for target_id={}",
            bound_page, target_id
        ));

        // 按 <br/> 拆分内容，每条拆分段作为独立消息插入
        let contents = split_br_tags(content);
        let conn = self.db_state.0.lock().await;
        let mut messages = Vec::new();
        for c in &contents {
            let msg = message_repo::insert_message(
                &conn, &target_id, "agent", agent_id, c, "text", bound_page,
            ).map_err(|e| ToolError::DatabaseError(e.to_string()))?;
            messages.push(msg);
        }

        crate::logger::backend("DEBUG", &format!(
            "[DEBUG ToolExecutor::execute_send_message] wrote {} messages target_id={}, page_index={:?}",
            messages.len(), target_id, bound_page
        ));

        Ok(messages)
    }

    async fn resolve_target_id(
        &self,
        agent_id: &str,
        raw: &str,
    ) -> Result<String, ToolError> {
        let conn = self.db_state.0.lock().await;

        // 1. 如果 raw 本身就是合法的 session_id，直接返回
        if let Ok(Some(_)) = session_repo::get_session_by_id(&conn, raw) {
            crate::logger::backend("DEBUG", &format!(
                "[DEBUG resolve_target_id] raw='{}' is valid session_id", raw
            ));
            return Ok(raw.to_string());
        }

        // 2. 如果 raw 是 agent_id，查找对应的私聊 session
        if let Ok(Some(session)) = session_repo::get_private_session_by_agent_id(&conn, raw) {
            crate::logger::backend("DEBUG", &format!(
                "[DEBUG resolve_target_id] raw='{}' resolved to agent session_id={}", raw, session.id
            ));
            return Ok(session.id);
        }

        // 3. 默认：使用该 agent 的默认私聊 session
        if let Ok(Some(session)) = session_repo::get_private_session_by_agent_id(&conn, agent_id) {
            crate::logger::backend("WARN", &format!(
                "[DEBUG resolve_target_id] raw='{}' not found, fallback to agent's default session {}",
                raw, session.id
            ));
            return Ok(session.id);
        }

        crate::logger::backend("ERROR", &format!(
            "[DEBUG resolve_target_id] raw='{}' not found for agent_id={}", raw, agent_id
        ));
        Err(ToolError::TargetNotFound(raw.to_string()))
    }
}
