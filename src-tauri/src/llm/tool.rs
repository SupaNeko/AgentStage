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

pub fn update_relationship_tool_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "update_relationship",
            "description": "更新你对某个参与者的主观关系描述。这用于记录你对对方的整体定位（如朋友/同事/竞争对手）和基本态度（如喜欢/讨厌/尊敬），不是记忆具体事件。请遵守以下规则：\n1. 只更新整体关系定位，不要记录日常琐事（如\"他今天吃了汉堡\"）\n2. 描述控制在 200 字以内\n3. 必须提供 old_text（当前关系描述的完整内容），系统会匹配替换\n4. 如果 old_text 不匹配（说明你记错了当前关系），系统会返回错误，请重新查询后再修改\n5. target_name 必须是参与者的精确名称（见【你认识的参与者】列表）",
            "parameters": {
                "type": "object",
                "properties": {
                    "target_name": {
                        "type": "string",
                        "description": "目标参与者的精确名称（从【你认识的参与者】列表中获取）"
                    },
                    "old_text": {
                        "type": "string",
                        "description": "当前关系描述的完整文本（空字符串表示尚无描述）"
                    },
                    "new_text": {
                        "type": "string",
                        "description": "新的关系描述文本（200字以内）"
                    }
                },
                "required": ["target_name", "old_text", "new_text"]
            }
        }
    })
}

pub fn fill_character_fields_tool_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "fill_character_fields",
            "description": "将分析提取到的角色信息填入对应字段。如果某项信息无法确定或该角色为原创角色不在你的知识库中，可将对应字段设为空字符串。",
            "parameters": {
                "type": "object",
                "properties": {
                    "personality": {
                        "type": "string",
                        "description": "角色的性格特征描述，如'傲娇、善良、有些天然呆'。可空。"
                    },
                    "scenario": {
                        "type": "string",
                        "description": "角色所处的世界观、场景或背景设定。可空。"
                    },
                    "example_messages": {
                        "type": "string",
                        "description": "角色的经典台词或代表性对话示例。可空。"
                    }
                },
                "required": ["personality", "scenario", "example_messages"]
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
                "update_relationship" => {
                    let _msgs = self.execute_update_relationship(agent_id, &tc.arguments).await?;
                    // update_relationship 不返回消息，仅修改数据库
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

    async fn execute_update_relationship(
        &self,
        agent_id: &str,
        arguments: &str,
    ) -> Result<Vec<Message>, ToolError> {
        crate::logger::backend("DEBUG", &format!(
            "[DEBUG ToolExecutor::execute_update_relationship] START agent_id={}, args_raw={}",
            agent_id, arguments
        ));

        let args: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| ToolError::InvalidArguments(e.to_string()))?;

        let target_name = args["target_name"].as_str().unwrap_or("");
        let old_text = args["old_text"].as_str().unwrap_or("");
        let new_text = args["new_text"].as_str().unwrap_or("");

        crate::logger::backend("DEBUG", &format!(
            "[DEBUG ToolExecutor::execute_update_relationship] parsed target_name='{}', old_text_len={}, new_text_len={}",
            target_name, old_text.len(), new_text.len()
        ));

        if target_name.is_empty() {
            return Err(ToolError::InvalidArguments("target_name 不能为空".to_string()));
        }

        // 校验长度
        if new_text.chars().count() > 200 {
            crate::logger::backend("WARN", &format!(
                "[DEBUG ToolExecutor::execute_update_relationship] Text too long: {} chars", new_text.chars().count()
            ));
            return Err(ToolError::InvalidArguments(format!(
                "关系描述超过 200 字限制（当前 {} 字）", new_text.chars().count()
            )));
        }

        let conn = self.db_state.0.lock().await;

        // 根据名称查找目标
        let (target_id, target_type) = if let Ok(Some(agent)) = agent_repo::get_agent_by_name(&conn, target_name) {
            crate::logger::backend("DEBUG", &format!(
                "[DEBUG ToolExecutor::execute_update_relationship] resolved to agent id={}", agent.id
            ));
            (agent.id, "agent".to_string())
        } else {
            // 尝试查找当前激活的用户人设
            let active_id: Option<String> = conn.query_row(
                "SELECT active_persona_id FROM app_settings WHERE id = 1", [], |row| row.get(0),
            ).ok().flatten();
            if let Some(pid) = active_id {
                if let Ok(persona) = crate::db::user_persona::get_user_persona_by_id(&conn, &pid) {
                    if persona.name == target_name {
                        crate::logger::backend("DEBUG", &format!(
                            "[DEBUG ToolExecutor::execute_update_relationship] resolved to user_persona id={}", pid
                        ));
                        (pid, "user_persona".to_string())
                    } else {
                        crate::logger::backend("WARN", &format!(
                            "[DEBUG ToolExecutor::execute_update_relationship] active persona name '{}' does not match target_name '{}'", persona.name, target_name
                        ));
                        return Err(ToolError::InvalidArguments(format!(
                            "找不到目标参与者 '{}'", target_name
                        )));
                    }
                } else {
                    return Err(ToolError::InvalidArguments(format!(
                        "找不到目标参与者 '{}'", target_name
                    )));
                }
            } else {
                return Err(ToolError::InvalidArguments(format!(
                    "找不到目标参与者 '{}'", target_name
                )));
            }
        };

        // old_text 匹配校验
        let current = crate::db::agent_relationship::get_relationship(&conn, agent_id, &target_id, &target_type)
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        crate::logger::backend("DEBUG", &format!(
            "[DEBUG ToolExecutor::execute_update_relationship] compare current='{}' (len={}) vs old_text='{}' (len={}) equal={}",
            current, current.len(), old_text, old_text.len(), current == old_text
        ));

        if current != old_text {
            crate::logger::backend("WARN", &format!(
                "[DEBUG ToolExecutor::execute_update_relationship] old_text mismatch"
            ));
            return Err(ToolError::InvalidArguments(format!(
                "old_text 不匹配。当前关系描述为：\"{}\"（长度{}），你提交的是：\"{}\"（长度{}）。请基于当前内容重新提交修改。",
                current, current.len(), old_text, old_text.len()
            )));
        }

        crate::db::agent_relationship::upsert_relationship(&conn, agent_id, &target_id, &target_type, new_text)
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        crate::logger::backend("DEBUG", &format!(
            "[DEBUG ToolExecutor::execute_update_relationship] END updated agent_id={} -> target_id={}",
            agent_id, target_id
        ));

        Ok(Vec::new())
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
