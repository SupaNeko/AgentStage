use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::db::connection::DbState;
use crate::db::session as session_repo;
use crate::db::message as message_repo;
use crate::db::agent as agent_repo;
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

pub fn start_private_chat_tool_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "start_private_chat",
            "description": "向另一个角色发起私聊。你需要提供对方的精确名称（target_name）和第一条消息内容（content）。如果对方不存在或名称不匹配，会返回错误。",
            "parameters": {
                "type": "object",
                "properties": {
                    "target_name": {
                        "type": "string",
                        "description": "目标角色的精确名称（exact match）"
                    },
                    "content": {
                        "type": "string",
                        "description": "第一条消息内容"
                    }
                },
                "required": ["target_name", "content"]
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
                "start_private_chat" => {
                    let msgs = self.execute_start_private_chat(agent_id, &tc.arguments, session_pages).await?;
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

    async fn execute_start_private_chat(
        &self,
        agent_id: &str,
        arguments: &str,
        session_pages: &HashMap<String, i32>,
    ) -> Result<Vec<Message>, ToolError> {
        crate::logger::backend("DEBUG", &format!(
            "[DEBUG ToolExecutor::execute_start_private_chat] START agent_id={}, args_raw={}",
            agent_id, arguments
        ));

        let args: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| ToolError::InvalidArguments(e.to_string()))?;

        let target_name = args["target_name"].as_str().unwrap_or("");
        let content = args["content"].as_str().unwrap_or("");

        crate::logger::backend("DEBUG", &format!(
            "[DEBUG ToolExecutor::execute_start_private_chat] parsed target_name={}, content_len={}",
            target_name, content.len()
        ));

        if content.is_empty() {
            crate::logger::backend("WARN", &format!(
                "[DEBUG ToolExecutor::execute_start_private_chat] Empty content, aborting"
            ));
            return Err(ToolError::EmptyContent);
        }

        let conn = self.db_state.0.lock().await;

        let target_agent = agent_repo::get_agent_by_name(&conn, target_name)
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?
            .ok_or_else(|| ToolError::TargetNotFound(target_name.to_string()))?;

        if target_agent.id == agent_id {
            return Err(ToolError::InvalidArguments("无法与自己发起私聊".to_string()));
        }

        let target_id = target_agent.id;

        let session = session_repo::get_private_session_between_agents(&conn, agent_id, &target_id)
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        let session_id = if let Some(session) = session {
            session.id
        } else {
            let new_session = session_repo::create_agent_agent_session(&conn, agent_id, &target_id)
                .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
            new_session.id
        };

        // 插入双向 friendships
        let now = chrono::Utc::now().timestamp_millis();
        let _ = conn.execute(
            "INSERT OR IGNORE INTO friendships (id, agent_id_1, agent_id_2, participant_type_2, created_at, source_session_id) VALUES (?1, ?2, ?3, 'agent', ?4, ?5)",
            rusqlite::params![uuid::Uuid::new_v4().to_string(), agent_id, &target_id, now, &session_id],
        );
        let _ = conn.execute(
            "INSERT OR IGNORE INTO friendships (id, agent_id_1, agent_id_2, participant_type_2, created_at, source_session_id) VALUES (?1, ?2, ?3, 'agent', ?4, ?5)",
            rusqlite::params![uuid::Uuid::new_v4().to_string(), &target_id, agent_id, now, &session_id],
        );

        let bound_page = session_pages.get(&session_id).copied();
        let contents = split_br_tags(content);
        let mut messages = Vec::new();
        for c in &contents {
            let msg = message_repo::insert_message(
                &conn, &session_id, "agent", agent_id, c, "text", bound_page,
            ).map_err(|e| ToolError::DatabaseError(e.to_string()))?;
            messages.push(msg);
        }

        crate::logger::backend("DEBUG", &format!(
            "[DEBUG ToolExecutor::execute_start_private_chat] wrote {} messages session_id={}, page_index={:?}",
            messages.len(), session_id, bound_page
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

        // 2. 如果 raw 是 agent_id，优先查找与 sender 的 agent-agent 私聊 session
        if let Ok(Some(session)) = session_repo::get_private_session_between_agents(&conn, agent_id, raw) {
            crate::logger::backend("DEBUG", &format!(
                "[DEBUG resolve_target_id] raw='{}' resolved to agent-agent session_id={}", raw, session.id
            ));
            return Ok(session.id);
        }

        // 3. 查找 raw 对应的 user-agent 私聊 session
        if let Ok(Some(session)) = session_repo::get_private_session_by_agent_id(&conn, raw) {
            crate::logger::backend("DEBUG", &format!(
                "[DEBUG resolve_target_id] raw='{}' resolved to user-agent session_id={}", raw, session.id
            ));
            return Ok(session.id);
        }

        // 4. 默认：使用该 agent 自己的 user-agent 私聊 session
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use rusqlite::Connection;
    use crate::db::connection::DbState;
    use crate::db::schema::{MIGRATION_V1, MIGRATION_V2, MIGRATION_V3, MIGRATION_V4, MIGRATION_V5, MIGRATION_V6, MIGRATION_V7, MIGRATION_V8};

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

    fn create_test_agent(conn: &Connection, agent_id: &str, name: &str) {
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            (agent_id, name, 0i64),
        ).unwrap();
    }

    #[tokio::test]
    async fn test_start_private_chat_creates_session_and_message() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1", "Alice");
        create_test_agent(&conn, "agent-2", "Bob");
        let db_state = make_db_state(conn);

        let executor = ToolExecutor::new(db_state);
        let session_pages = HashMap::new();
        let tool_call = ToolCall {
            id: "tc-1".to_string(),
            name: "start_private_chat".to_string(),
            arguments: r#"{"target_name":"Bob","content":"Hello Bob"}"#.to_string(),
        };

        let messages = executor.execute("agent-1", vec![tool_call], &session_pages).await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Hello Bob");
        assert_eq!(messages[0].sender_id, "agent-1");

        let conn = executor.db_state.0.lock().await;
        let session = crate::db::session::get_private_session_between_agents(&conn, "agent-1", "agent-2").unwrap();
        assert!(session.is_some());
        let session_id = session.unwrap().id;
        assert_eq!(messages[0].session_id, session_id);

        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM friendships WHERE source_session_id = ?1",
            [&session_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_start_private_chat_reuses_existing_session() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1", "Alice");
        create_test_agent(&conn, "agent-2", "Bob");
        let db_state = make_db_state(conn);

        let executor = ToolExecutor::new(db_state);
        let session_pages = HashMap::new();

        let tc1 = ToolCall {
            id: "tc-1".to_string(),
            name: "start_private_chat".to_string(),
            arguments: r#"{"target_name":"Bob","content":"First"}"#.to_string(),
        };
        let msgs1 = executor.execute("agent-1", vec![tc1], &session_pages).await.unwrap();
        let session_id_1 = msgs1[0].session_id.clone();

        let tc2 = ToolCall {
            id: "tc-2".to_string(),
            name: "start_private_chat".to_string(),
            arguments: r#"{"target_name":"Bob","content":"Second"}"#.to_string(),
        };
        let msgs2 = executor.execute("agent-1", vec![tc2], &session_pages).await.unwrap();
        assert_eq!(msgs2[0].session_id, session_id_1);
    }

    #[tokio::test]
    async fn test_start_private_chat_target_not_found() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1", "Alice");
        let db_state = make_db_state(conn);

        let executor = ToolExecutor::new(db_state);
        let tc = ToolCall {
            id: "tc-1".to_string(),
            name: "start_private_chat".to_string(),
            arguments: r#"{"target_name":"Unknown","content":"Hi"}"#.to_string(),
        };
        let result = executor.execute("agent-1", vec![tc], &HashMap::new()).await;
        assert!(matches!(result, Err(ToolError::TargetNotFound(_))));
    }

    #[tokio::test]
    async fn test_start_private_chat_self_chat_fails() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1", "Alice");
        let db_state = make_db_state(conn);

        let executor = ToolExecutor::new(db_state);
        let tc = ToolCall {
            id: "tc-1".to_string(),
            name: "start_private_chat".to_string(),
            arguments: r#"{"target_name":"Alice","content":"Hi me"}"#.to_string(),
        };
        let result = executor.execute("agent-1", vec![tc], &HashMap::new()).await;
        assert!(matches!(result, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn test_resolve_target_id_prefers_agent_agent_session() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1", "Alice");
        create_test_agent(&conn, "agent-2", "Bob");
        let _ua_session = crate::db::session::create_private_session(&conn, "agent-2").unwrap();
        let aa_session = crate::db::session::create_agent_agent_session(&conn, "agent-1", "agent-2").unwrap();
        let _ = crate::db::session::create_private_session(&conn, "agent-1").unwrap();
        let db_state = make_db_state(conn);

        let executor = ToolExecutor::new(db_state);

        let resolved = executor.resolve_target_id("agent-1", "agent-2").await.unwrap();
        assert_eq!(resolved, aa_session.id);

        let resolved2 = executor.resolve_target_id("agent-1", "nonexistent").await.unwrap();
        let conn_guard = executor.db_state.0.lock().await;
        let fallback = crate::db::session::get_private_session_by_agent_id(&*conn_guard, "agent-1").unwrap().unwrap();
        assert_eq!(resolved2, fallback.id);
    }
}
