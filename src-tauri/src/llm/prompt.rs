use chrono::{Local, LocalResult, TimeZone};
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};

use crate::llm::prompt_templates;
use crate::models::message::Message;

pub struct PromptAssembler;

impl PromptAssembler {
    pub fn assemble(
        conn: &Connection,
        agent_id: &str,
        trigger_session_id: Option<&str>,
        trigger_page_index: Option<i32>,
        _pending_messages: &[Message],
    ) -> Result<String, String> {
        crate::logger::backend("DEBUG", &format!("[DEBUG prompt::assemble] agent_id={}, pending_messages={}", agent_id, _pending_messages.len()));

        let mut layers: Vec<String> = Vec::new();

        // Layer 1: System Prompt
        layers.push(prompt_templates::SYSTEM_PROMPT.to_string());

        // Layer 2: Self Persona
        let agent = Self::get_agent(conn, agent_id)?;
        layers.push(format!("{}\n{}", prompt_templates::LAYER_PERSONA_TITLE, agent.detailed_persona));

        // Layer 3: Participants Introduction
        let participants = Self::get_participants(conn, agent_id)?;
        if !participants.is_empty() {
            let mut layer = String::from(prompt_templates::LAYER_PARTICIPANTS_TITLE);
            layer.push('\n');
            for (name, relation, persona) in participants {
                layer.push_str(&format!("- {}（{}）：{}\n", name, relation, persona));
            }
            layer.push_str(prompt_templates::LAYER_PARTICIPANTS_USER_LINE);
            layers.push(layer);
        }

        // Layer 4: Chat History — per session with individual history limits
        let mut session_order: Vec<String> = Vec::new();
        let mut grouped: HashMap<String, Vec<Message>> = HashMap::new();
        
        {
            let mut stmt = conn.prepare(
                "SELECT m.id, m.session_id, m.sender_type, m.sender_id, m.content, m.created_at, 
                        m.message_type, m.tool_call_data, m.generation_info, m.is_deleted,
                        COALESCE(a.name, CASE WHEN m.sender_type = 'user' THEN '用户' ELSE '未知' END) as sender_name,
                        a.avatar_path as sender_avatar,
                        m.page_index
                  FROM messages m
                  JOIN (
                      SELECT ps.session_id, COALESCE(ps.current_chat_page, 0) as page 
                      FROM private_sessions ps
                      JOIN sessions s ON ps.session_id = s.id
                      WHERE ps.agent_id = ?1 AND s.is_deleted = 0
                      UNION
                      SELECT gs.session_id, COALESCE(gs.current_chat_page, 0) as page 
                      FROM group_sessions gs
                      JOIN group_members gm ON gs.session_id = gm.session_id
                      JOIN sessions s ON gs.session_id = s.id
                      WHERE gm.participant_id = ?1 AND gm.participant_type = 'agent' AND s.is_deleted = 0
                   ) sp ON m.session_id = sp.session_id
                      AND m.page_index = CASE 
                          WHEN ?2 IS NOT NULL AND m.session_id = ?2 THEN ?3 
                          ELSE sp.page 
                      END
                 LEFT JOIN agents a ON m.sender_type = 'agent' AND m.sender_id = a.id AND a.is_deleted = 0
                 WHERE m.is_deleted = 0
                 ORDER BY m.created_at DESC"
            ).map_err(|e| e.to_string())?;
            
            let rows = stmt.query_map(
                rusqlite::params![agent_id, trigger_session_id, trigger_page_index],
                |row| {
                Ok(Message {
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
                    sender_avatar: row.get(11)?,
                    page_index: row.get(12)?,
                })
            }).map_err(|e| e.to_string())?;
            
            for row in rows {
                let msg = row.map_err(|e| e.to_string())?;
                if !grouped.contains_key(&msg.session_id) {
                    session_order.push(msg.session_id.clone());
                }
                grouped.entry(msg.session_id.clone()).or_default().push(msg);
            }
        }
        
        let mut filtered_messages: Vec<Message> = Vec::new();
        for sid in &session_order {
            if let Some(msgs) = grouped.get_mut(sid) {
                let limit: i32 = conn.query_row(
                    "SELECT COALESCE(history_limit, 50) FROM session_settings WHERE session_id = ?1",
                    [sid],
                    |row| row.get(0),
                ).unwrap_or(50);
                let take = msgs.len().min(limit as usize);
                // Keep the newest `take` messages (they're already in DESC order from SQL)
                msgs.reverse(); // now chronological (oldest first)
                let start = msgs.len().saturating_sub(take);
                filtered_messages.extend(msgs.drain(start..));
            }
        }
        filtered_messages.sort_by_key(|m| m.created_at);
        
        if !filtered_messages.is_empty() {
            let mut layer = String::from(prompt_templates::LAYER_HISTORY_TITLE);
            layer.push('\n');
            let mut current_session = String::new();
            for msg in &filtered_messages {
                if msg.session_id != current_session {
                    current_session = msg.session_id.clone();
                    let session_name = Self::get_session_name(conn, &current_session)?;
                    layer.push_str(&format!("\n--- {} ---\n", session_name));
                }
                let time = Self::format_time(msg.created_at);
                let sender = Self::get_sender_name(conn, &msg.sender_type, &msg.sender_id)?;
                layer.push_str(&format!("[{}] {}: {}\n", time, sender, msg.content));
            }
            layer.push('\n');
            layer.push_str(prompt_templates::LAYER_FOOTER_NOTE);
            layers.push(layer);
        }

        // Layer 6: Instruction（工具使用说明 + 当前上下文 ID）
        let instruction = Self::build_instruction(conn, agent_id, &agent.name)?;
        layers.push(instruction);

        let prompt = layers.join("\n\n");
        let prompt_with_vars = Self::apply_variables(&prompt, &agent.name);

        crate::logger::backend("DEBUG", &format!("[DEBUG prompt::assemble] agent_id={}, total_chars={}", agent_id, prompt_with_vars.len()));
        
        // 记录完整 prompt 内容到日志（新增需求）
        crate::logger::backend("INFO", &format!(
            "[PromptAssembler] Full prompt for agent {} | trigger_session={:?} | trigger_page={:?} | prompt_length={}\n---PROMPT START---\n{}\n---PROMPT END---",
            agent_id, trigger_session_id, trigger_page_index, prompt_with_vars.len(), prompt_with_vars
        ));
        
        Ok(prompt_with_vars)
    }

    /// 构建工具使用说明层，注入当前所有可见会话的 session_id
    fn build_instruction(
        conn: &Connection,
        agent_id: &str,
        _agent_name: &str,
    ) -> Result<String, String> {
        let sessions = Self::get_agent_sessions(conn, agent_id)?;

        let mut context_list = String::new();
        for (session_id, session_name, session_type) in &sessions {
            context_list.push_str(&format!(
                "- session_id: {}, 名称: {}, 类型: {}\n",
                session_id, session_name, session_type
            ));
        }

        let instruction = format!(
            "{}\n{}",
            prompt_templates::LAYER_INSTRUCTION_TITLE,
            prompt_templates::TOOL_INSTRUCTION_TEMPLATE.replace("{context_list}", &context_list)
        );

        Ok(instruction)
    }

    /// 获取 Agent 参与的所有会话列表
    fn get_agent_sessions(
        conn: &Connection,
        agent_id: &str,
    ) -> Result<Vec<(String, String, String)>, String> {
        let mut sessions = Vec::new();

        // 私聊会话
        let mut stmt = conn
            .prepare(
                "SELECT s.id, a.name, 'private' 
                 FROM sessions s 
                 JOIN private_sessions ps ON s.id = ps.session_id 
                 JOIN agents a ON ps.agent_id = a.id 
                 WHERE ps.agent_id = ?1 AND s.is_deleted = 0"
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([agent_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            sessions.push(row.map_err(|e| e.to_string())?);
        }
        drop(stmt);

        // 群聊会话
        let mut stmt = conn
            .prepare(
                "SELECT s.id, gs.name, 'group' 
                 FROM sessions s 
                 JOIN group_sessions gs ON s.id = gs.session_id 
                 JOIN group_members gm ON s.id = gm.session_id 
                 WHERE gm.participant_id = ?1 AND gm.participant_type = 'agent' AND s.is_deleted = 0"
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([agent_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            sessions.push(row.map_err(|e| e.to_string())?);
        }

        Ok(sessions)
    }

    /// 变量替换：{{char}} → agent_name, {{user}} → 用户名称
    fn apply_variables(prompt: &str, agent_name: &str) -> String {
        prompt
            .replace("{{char}}", agent_name)
            .replace("{{user}}", prompt_templates::USER_NAME)
            .replace("{{group}}", "群聊")
    }

    fn get_agent(
        conn: &Connection,
        agent_id: &str,
    ) -> Result<crate::models::agent::Agent, String> {
        crate::db::agent::get_by_id(conn, agent_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Agent not found: {}", agent_id))
    }

    fn get_participants(
        conn: &Connection,
        agent_id: &str,
    ) -> Result<Vec<(String, String, String)>, String> {
        let mut participants: Vec<(String, String, String)> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        seen.insert(agent_id.to_string());

        // Friends (including user relationships via participant_type_2 = 'user')
        let mut stmt = conn
            .prepare(
                "SELECT f.participant_type_2, a.id, a.name, a.simplified_persona 
                 FROM friendships f 
                 LEFT JOIN agents a ON a.id = CASE WHEN f.agent_id_1 = ?1 THEN f.agent_id_2 ELSE f.agent_id_1 END 
                 WHERE ?1 IN (f.agent_id_1, COALESCE(f.agent_id_2, '')) AND (a.is_deleted = 0 OR f.participant_type_2 = 'user')",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([agent_id], |row| {
                let pt: String = row.get(0)?;
                if pt == "user" {
                    Ok((
                        "user".to_string(),
                        "用户".to_string(),
                        "正在与你聊天的真实用户".to_string(),
                    ))
                } else {
                    Ok((
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                }
            })
            .map_err(|e| e.to_string())?;

        for row in rows {
            let (id, name, persona) = row.map_err(|e| e.to_string())?;
            if id == "user" {
                participants.push((name, "私聊对象".to_string(), persona));
            } else if seen.insert(id) {
                participants.push((name, "好友".to_string(), persona));
            }
        }
        drop(stmt);

        // Group mates
        let mut stmt = conn
            .prepare(
                "SELECT a.id, a.name, a.simplified_persona 
                 FROM agents a
                 WHERE a.id IN (
                     SELECT gm2.participant_id
                     FROM group_members gm1
                     JOIN group_members gm2 ON gm1.session_id = gm2.session_id
                     WHERE gm1.participant_id = ?1 AND gm1.participant_type = 'agent'
                     AND gm2.participant_type = 'agent'
                     AND gm2.participant_id != ?1
                 )
                 AND a.is_deleted = 0",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([agent_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(|e| e.to_string())?;

        for row in rows {
            let (id, name, persona) = row.map_err(|e| e.to_string())?;
            if seen.insert(id) {
                participants.push((name, "群友".to_string(), persona));
            }
        }

        Ok(participants)
    }

    fn get_session_name(conn: &Connection, session_id: &str) -> Result<String, String> {
        // Try private session first
        let result: Result<String, rusqlite::Error> = conn.query_row(
            "SELECT a.name FROM private_sessions ps JOIN agents a ON ps.agent_id = a.id WHERE ps.session_id = ?1",
            [session_id],
            |row| row.get(0),
        );
        if let Ok(name) = result {
            return Ok(name);
        }

        // Try group session
        let result: Result<String, rusqlite::Error> = conn.query_row(
            "SELECT name FROM group_sessions WHERE session_id = ?1",
            [session_id],
            |row| row.get(0),
        );
        if let Ok(name) = result {
            return Ok(name);
        }

        Ok(prompt_templates::UNKNOWN_SESSION.to_string())
    }

    pub(crate) fn get_sender_name(
        conn: &Connection,
        sender_type: &str,
        sender_id: &str,
    ) -> Result<String, String> {
        match sender_type {
            "user" => Ok(prompt_templates::USER_NAME.to_string()),
            "system" => Ok(prompt_templates::SYSTEM_NAME.to_string()),
            "agent" => {
                let result: Result<String, rusqlite::Error> = conn.query_row(
                    "SELECT name FROM agents WHERE id = ?1 AND is_deleted = 0",
                    [sender_id],
                    |row| row.get(0),
                );
                match result {
                    Ok(name) => Ok(name),
                    Err(rusqlite::Error::QueryReturnedNoRows) => {
                        Ok(format!("{}{})", prompt_templates::UNKNOWN_AGENT_PREFIX, sender_id))
                    }
                    Err(e) => Err(e.to_string()),
                }
            }
            _ => Ok(format!("{}{})", prompt_templates::UNKNOWN_TYPE_PREFIX, sender_type)),
        }
    }

    pub(crate) fn format_time(timestamp_ms: i64) -> String {
        match Local.timestamp_millis_opt(timestamp_ms) {
            LocalResult::Single(dt) => dt.format("%H:%M").to_string(),
            _ => "??".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use crate::models::message::Message;

    fn init_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V1).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V2).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V3).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V4).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V5).unwrap();
        conn
    }

    fn insert_agent(conn: &Connection, id: &str, name: &str, persona: &str) {
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, ?3, '', ?4, ?4)",
            (id, name, persona, 0i64),
        ).unwrap();
    }

    fn insert_session(conn: &Connection, session_id: &str, session_type: &str) {
        conn.execute(
            "INSERT INTO sessions (id, session_type, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
            (session_id, session_type, 0i64),
        ).unwrap();
    }

    fn insert_private_session(conn: &Connection, session_id: &str, agent_id: &str, page: i32) {
        conn.execute(
            "INSERT INTO private_sessions (session_id, agent_id, created_at, current_chat_page) VALUES (?1, ?2, ?3, ?4)",
            (session_id, agent_id, 0i64, page),
        ).unwrap();
    }

    fn insert_session_settings(conn: &Connection, session_id: &str, history_limit: i32) {
        conn.execute(
            "INSERT INTO session_settings (session_id, history_limit, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
            (session_id, history_limit, 0i64),
        ).unwrap();
    }

    fn insert_group_session(conn: &Connection, session_id: &str, name: &str, page: i32) {
        conn.execute(
            "INSERT INTO group_sessions (session_id, name, created_at, current_chat_page) VALUES (?1, ?2, ?3, ?4)",
            (session_id, name, 0i64, page),
        ).unwrap();
    }

    fn insert_group_member(conn: &Connection, session_id: &str, participant_id: &str, participant_type: &str) {
        conn.execute(
            "INSERT INTO group_members (session_id, participant_id, participant_type, created_at) VALUES (?1, ?2, ?3, ?4)",
            (session_id, participant_id, participant_type, 0i64),
        ).unwrap();
    }

    fn soft_delete_session(conn: &Connection, session_id: &str) {
        conn.execute(
            "UPDATE sessions SET is_deleted = 1 WHERE id = ?1",
            [session_id],
        ).unwrap();
    }

    fn insert_message(conn: &Connection, msg: &Message) {
        conn.execute(
            "INSERT INTO messages (id, session_id, sender_type, sender_id, content, created_at, message_type, tool_call_data, generation_info, is_deleted, page_index) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            (
                &msg.id, &msg.session_id, &msg.sender_type, &msg.sender_id, &msg.content,
                msg.created_at, &msg.message_type, &msg.tool_call_data, &msg.generation_info,
                if msg.is_deleted { 1 } else { 0 }, msg.page_index,
            ),
        ).unwrap();
    }

    #[test]
    fn test_prompt_no_layer5_header() {
        let conn = init_test_db();
        insert_agent(&conn, "agent1", "Test Agent", "A test persona");
        insert_session(&conn, "sess1", "private");
        insert_private_session(&conn, "sess1", "agent1", 0);
        insert_session_settings(&conn, "sess1", 50);

        let msg = Message {
            id: "msg1".to_string(),
            session_id: "sess1".to_string(),
            sender_type: "user".to_string(),
            sender_id: "user".to_string(),
            content: "Hello".to_string(),
            created_at: 1000,
            message_type: "text".to_string(),
            tool_call_data: None,
            generation_info: None,
            is_deleted: false,
            sender_name: "用户".to_string(),
            sender_avatar: None,
            page_index: 0,
        };
        insert_message(&conn, &msg);

        let prompt = PromptAssembler::assemble(&conn, "agent1", None, None, &[]).unwrap();
        assert!(!prompt.contains("【最新消息"), "Prompt should not contain Layer 5 header, but got:\n{}", prompt);
    }

    #[test]
    fn test_prompt_contains_footer_note() {
        let conn = init_test_db();
        insert_agent(&conn, "agent1", "Test Agent", "A test persona");
        insert_session(&conn, "sess1", "private");
        insert_private_session(&conn, "sess1", "agent1", 0);
        insert_session_settings(&conn, "sess1", 50);

        let msg = Message {
            id: "msg1".to_string(),
            session_id: "sess1".to_string(),
            sender_type: "user".to_string(),
            sender_id: "user".to_string(),
            content: "Hello".to_string(),
            created_at: 1000,
            message_type: "text".to_string(),
            tool_call_data: None,
            generation_info: None,
            is_deleted: false,
            sender_name: "用户".to_string(),
            sender_avatar: None,
            page_index: 0,
        };
        insert_message(&conn, &msg);

        let prompt = PromptAssembler::assemble(&conn, "agent1", None, None, &[]).unwrap();
        assert!(prompt.contains(super::prompt_templates::LAYER_FOOTER_NOTE), "Prompt should contain footer note, but got:\n{}", prompt);
    }

    #[test]
    fn test_prompt_chronological_order() {
        let conn = init_test_db();
        insert_agent(&conn, "agent1", "Test Agent", "A test persona");
        insert_session(&conn, "sess1", "private");
        insert_private_session(&conn, "sess1", "agent1", 0);
        insert_session_settings(&conn, "sess1", 50);

        let msg1 = Message {
            id: "msg1".to_string(),
            session_id: "sess1".to_string(),
            sender_type: "user".to_string(),
            sender_id: "user".to_string(),
            content: "First message".to_string(),
            created_at: 1000,
            message_type: "text".to_string(),
            tool_call_data: None,
            generation_info: None,
            is_deleted: false,
            sender_name: "用户".to_string(),
            sender_avatar: None,
            page_index: 0,
        };
        let msg2 = Message {
            id: "msg2".to_string(),
            session_id: "sess1".to_string(),
            sender_type: "user".to_string(),
            sender_id: "user".to_string(),
            content: "Second message".to_string(),
            created_at: 2000,
            message_type: "text".to_string(),
            tool_call_data: None,
            generation_info: None,
            is_deleted: false,
            sender_name: "用户".to_string(),
            sender_avatar: None,
            page_index: 0,
        };
        insert_message(&conn, &msg1);
        insert_message(&conn, &msg2);

        let prompt = PromptAssembler::assemble(&conn, "agent1", None, None, &[]).unwrap();
        let pos1 = prompt.find("First message").expect("First message not found");
        let pos2 = prompt.find("Second message").expect("Second message not found");
        assert!(pos1 < pos2, "Messages should be in chronological order (oldest first), but got:\n{}", prompt);
    }

    #[test]
    fn test_prompt_assemble_uses_trigger_page_for_trigger_session() {
        let conn = init_test_db();
        insert_agent(&conn, "agent1", "Agent One", "Persona 1");
        
        // Create private session for agent1 (page 0)
        insert_session(&conn, "sess1", "private");
        insert_private_session(&conn, "sess1", "agent1", 0);
        insert_session_settings(&conn, "sess1", 50);
        
        // Insert message to page 0
        let msg0 = Message {
            id: "msg0".to_string(), session_id: "sess1".to_string(),
            sender_type: "user".to_string(), sender_id: "user".to_string(),
            content: "Page0 message".to_string(), created_at: 1000,
            message_type: "text".to_string(), tool_call_data: None,
            generation_info: None, is_deleted: false,
            sender_name: "用户".to_string(), sender_avatar: None, page_index: 0,
        };
        insert_message(&conn, &msg0);
        
        // Simulate reset: create page 1 and insert message there
        let msg1 = Message {
            id: "msg1".to_string(), session_id: "sess1".to_string(),
            sender_type: "user".to_string(), sender_id: "user".to_string(),
            content: "Page1 message".to_string(), created_at: 2000,
            message_type: "text".to_string(), tool_call_data: None,
            generation_info: None, is_deleted: false,
            sender_name: "用户".to_string(), sender_avatar: None, page_index: 1,
        };
        insert_message(&conn, &msg1);
        
        // Trigger from page 0: prompt should contain "Page0 message" but NOT "Page1 message"
        let prompt = PromptAssembler::assemble(&conn, "agent1", Some("sess1"), Some(0), &[]).unwrap();
        assert!(prompt.contains("Page0 message"), "Prompt should contain page 0 message");
        assert!(!prompt.contains("Page1 message"), "Prompt should NOT contain page 1 message when triggered from page 0");
        
        // Trigger from page 1: prompt should contain "Page1 message" but NOT "Page0 message"
        let prompt = PromptAssembler::assemble(&conn, "agent1", Some("sess1"), Some(1), &[]).unwrap();
        assert!(prompt.contains("Page1 message"), "Prompt should contain page 1 message");
        assert!(!prompt.contains("Page0 message"), "Prompt should NOT contain page 0 message when triggered from page 1");
    }

    #[test]
    fn test_prompt_excludes_deleted_session_messages() {
        let conn = init_test_db();
        insert_agent(&conn, "agent1", "Test Agent", "A test persona");
        
        // Create active private session
        insert_session(&conn, "sess_active", "private");
        insert_private_session(&conn, "sess_active", "agent1", 0);
        insert_session_settings(&conn, "sess_active", 50);
        
        let active_msg = Message {
            id: "msg_active".to_string(),
            session_id: "sess_active".to_string(),
            sender_type: "user".to_string(),
            sender_id: "user".to_string(),
            content: "Active session message".to_string(),
            created_at: 1000,
            message_type: "text".to_string(),
            tool_call_data: None,
            generation_info: None,
            is_deleted: false,
            sender_name: "用户".to_string(),
            sender_avatar: None,
            page_index: 0,
        };
        insert_message(&conn, &active_msg);
        
        // Create deleted group session that agent1 participates in
        insert_session(&conn, "sess_deleted", "group");
        insert_group_session(&conn, "sess_deleted", "Deleted Group", 0);
        insert_group_member(&conn, "sess_deleted", "agent1", "agent");
        insert_session_settings(&conn, "sess_deleted", 50);
        soft_delete_session(&conn, "sess_deleted");
        
        let deleted_msg = Message {
            id: "msg_deleted".to_string(),
            session_id: "sess_deleted".to_string(),
            sender_type: "user".to_string(),
            sender_id: "user".to_string(),
            content: "Deleted group message".to_string(),
            created_at: 2000,
            message_type: "text".to_string(),
            tool_call_data: None,
            generation_info: None,
            is_deleted: false,
            sender_name: "用户".to_string(),
            sender_avatar: None,
            page_index: 0,
        };
        insert_message(&conn, &deleted_msg);
        
        let prompt = PromptAssembler::assemble(&conn, "agent1", None, None, &[]).unwrap();
        assert!(prompt.contains("Active session message"), "Prompt should contain active session message");
        assert!(!prompt.contains("Deleted group message"), "Prompt should NOT contain deleted session message, but got:\n{}", prompt);
    }
}
