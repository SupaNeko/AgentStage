use serde::{Deserialize, Serialize};
use crate::db::connection::DbState;
use crate::db::session as session_repo;
use crate::db::message as message_repo;
use crate::models::message::Message;

pub fn send_message_tool_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "send_message",
            "description": "向指定会话发送一条消息。target_id 必须是系统提供的 session_id，不能使用会话名称或其他 ID。",
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
    ) -> Result<Vec<Message>, ToolError> {
        let mut results = Vec::new();

        for tc in tool_calls {
            match tc.name.as_str() {
                "send_message" => {
                    let msg = self.execute_send_message(agent_id, &tc.arguments).await?;
                    results.push(msg);
                }
                _ => {
                    crate::logger::backend("WARN", &format!("Unknown tool call: {}", tc.name));
                }
            }
        }

        Ok(results)
    }

    async fn execute_send_message(
        &self,
        agent_id: &str,
        arguments: &str,
    ) -> Result<Message, ToolError> {
        let args: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| ToolError::InvalidArguments(e.to_string()))?;

        let raw_target_id = args["target_id"].as_str().unwrap_or("");
        let content = args["content"].as_str().unwrap_or("");

        if content.is_empty() {
            return Err(ToolError::EmptyContent);
        }

        // 自动映射 target_id
        let target_id = self.resolve_target_id(agent_id, raw_target_id).await?;

        // 插入消息
        let conn = self.db_state.0.lock().await;
        let msg = message_repo::insert_message(
            &conn, &target_id, "agent", agent_id, content, "text",
        ).map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        crate::logger::backend("DEBUG", &format!(
            "[DEBUG ToolExecutor] wrote message target_id={}, message_id={}",
            target_id, msg.id
        ));

        Ok(msg)
    }

    async fn resolve_target_id(
        &self,
        agent_id: &str,
        raw: &str,
    ) -> Result<String, ToolError> {
        let conn = self.db_state.0.lock().await;

        // 1. 如果 raw 本身就是合法的 session_id，直接返回
        if let Ok(Some(_)) = session_repo::get_session_by_id(&conn, raw) {
            return Ok(raw.to_string());
        }

        // 2. 如果 raw 是 agent_id，查找对应的私聊 session
        if let Ok(Some(session)) = session_repo::get_private_session_by_agent_id(&conn, raw) {
            return Ok(session.id);
        }

        // 3. 默认：使用该 agent 的默认私聊 session
        if let Ok(Some(session)) = session_repo::get_private_session_by_agent_id(&conn, agent_id) {
            crate::logger::backend("WARN", &format!(
                "[DEBUG ToolExecutor] target_id '{}' not found, fallback to agent's default session {}",
                raw, session.id
            ));
            return Ok(session.id);
        }

        Err(ToolError::TargetNotFound(raw.to_string()))
    }
}
