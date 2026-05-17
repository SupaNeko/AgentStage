use chrono::{Local, LocalResult, TimeZone};
use rusqlite::Connection;
use std::collections::HashSet;

use crate::llm::prompt_templates;
use crate::models::message::Message;
use crate::db::user_persona;
use crate::constants::{DEFAULT_USER_NAME, DEFAULT_USER_PERSONA};

pub struct PromptAssembler;

impl PromptAssembler {
    pub fn assemble(
        conn: &Connection,
        agent_id: &str,
        trigger_session_id: Option<&str>,
        trigger_page_index: Option<i32>,
        _pending_messages: &[Message],
        pending_ids: &std::collections::HashSet<String>,
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
            layers.push(layer);
        }

        // Layer 4: Chat History — per session with individual history limits
        let mut session_order: Vec<String> = Vec::new();
        let mut grouped: std::collections::HashMap<String, Vec<Message>> = std::collections::HashMap::new();
        
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
                       WHERE (ps.participant_1_type = 'agent' AND ps.participant_1_id = ?1) OR (ps.participant_2_type = 'agent' AND ps.participant_2_id = ?1) AND s.is_deleted = 0
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
        
        if !filtered_messages.is_empty() {
            let mut layer = String::from(prompt_templates::LAYER_HISTORY_TITLE);
            layer.push('\n');
            let mut current_session = String::new();
            for msg in &filtered_messages {
                if msg.session_id != current_session {
                    current_session = msg.session_id.clone();
                    let session_name = Self::get_session_name(conn, &current_session, agent_id)?;

                    let new_count = filtered_messages.iter()
                        .filter(|m| m.session_id == current_session && pending_ids.contains(&m.id))
                        .count();

                    let new_label = if new_count > 0 {
                        format!(" ({} 条新消息)", new_count)
                    } else {
                        String::new()
                    };

                    layer.push_str(&format!("\n--- {}{} ---\n", session_name, new_label));
                }
                let time = Self::format_time(msg.created_at);
                let sender = Self::get_sender_name(conn, &msg.sender_type, &msg.sender_id)?;
                let is_new = pending_ids.contains(&msg.id);
                let new_mark = if is_new { " [新]" } else { "" };
                layer.push_str(&format!("[{}]{} {}: {}\n", time, new_mark, sender, msg.content));
            }
            layer.push('\n');
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

        // Private sessions
        let mut stmt = conn
            .prepare(
                "SELECT s.id, ps.participant_1_type, ps.participant_1_id, ps.participant_2_type, ps.participant_2_id \
                 FROM sessions s \
                 JOIN private_sessions ps ON s.id = ps.session_id \
                 WHERE s.is_deleted = 0 \
                 AND ((ps.participant_1_type = 'agent' AND ps.participant_1_id = ?1) \
                   OR (ps.participant_2_type = 'agent' AND ps.participant_2_id = ?1))"
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([agent_id], |row| {
                let sid: String = row.get(0)?;
                let p1_type: String = row.get(1)?;
                let p1_id: String = row.get(2)?;
                let p2_type: String = row.get(3)?;
                let p2_id: String = row.get(4)?;
                
                let other = if p1_type == "agent" && p1_id == agent_id {
                    (p2_type, p2_id)
                } else {
                    (p1_type, p1_id)
                };
                
                let other_name = if other.0 == "user" {
                    Self::get_user_persona(conn).0
                } else {
                    conn.query_row(
                        "SELECT name FROM agents WHERE id = ?1 AND is_deleted = 0",
                        [other.1],
                        |row| row.get(0),
                    ).unwrap_or_else(|_| prompt_templates::UNKNOWN_SESSION.to_string())
                };
                
                Ok((sid, other_name, "private".to_string()))
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            sessions.push(row.map_err(|e| e.to_string())?);
        }
        drop(stmt);

        // Group sessions (unchanged logic)
        let mut stmt = conn
            .prepare(
                "SELECT s.id, gs.name, 'group' \
                 FROM sessions s \
                 JOIN group_sessions gs ON s.id = gs.session_id \
                 JOIN group_members gm ON s.id = gm.session_id \
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

    fn get_user_persona(conn: &Connection) -> (String, String) {
        match user_persona::get_current_user_persona(conn) {
            Ok(p) => (p.name, p.description),
            Err(_) => (DEFAULT_USER_NAME.to_string(), DEFAULT_USER_PERSONA.to_string()),
        }
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
        let mut seen: HashSet<String> = HashSet::new();
        let mut participants: Vec<(String, String, String)> = Vec::new();

        // 1. Collect private chat partners
        let mut stmt = conn
            .prepare(
                "SELECT ps.participant_1_type, ps.participant_1_id, ps.participant_2_type, ps.participant_2_id \
                 FROM private_sessions ps \
                 JOIN sessions s ON ps.session_id = s.id \
                 WHERE s.is_deleted = 0 \
                 AND ((ps.participant_1_type = 'agent' AND ps.participant_1_id = ?1) \
                   OR (ps.participant_2_type = 'agent' AND ps.participant_2_id = ?1))"
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([agent_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| e.to_string())?;

        let (user_name, user_persona) = Self::get_user_persona(conn);

        for row in rows {
            let (p1_type, p1_id, p2_type, p2_id) = row.map_err(|e| e.to_string())?;
            let other = if p1_type == "agent" && p1_id == agent_id {
                (p2_type, p2_id)
            } else {
                (p1_type, p1_id)
            };

            if other.0 == "user" {
                if seen.insert("__user__".to_string()) {
                    participants.push((user_name.clone(), "好友".to_string(), user_persona.clone()));
                }
            } else if seen.insert(other.1.clone()) {
                let other_id = other.1.clone();
                let name: Result<String, rusqlite::Error> = conn.query_row(
                    "SELECT name FROM agents WHERE id = ?1 AND is_deleted = 0",
                    [&other_id],
                    |row| row.get(0),
                );
                if let Ok(name) = name {
                    let persona: Result<String, rusqlite::Error> = conn.query_row(
                        "SELECT simplified_persona FROM agents WHERE id = ?1 AND is_deleted = 0",
                        [&other_id],
                        |row| row.get(0),
                    );
                    participants.push((name, "好友".to_string(), persona.unwrap_or_default()));
                }
            }
        }
        drop(stmt);

        // 2. Collect group chat members
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT gm.participant_id \
                 FROM group_members gm \
                 JOIN sessions s ON gm.session_id = s.id \
                 WHERE gm.session_id IN ( \
                     SELECT session_id FROM group_members WHERE participant_id = ?1 AND participant_type = 'agent' \
                 ) AND gm.participant_type = 'agent' AND gm.participant_id != ?1 AND s.is_deleted = 0"
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([agent_id], |row| {
                Ok(row.get::<_, String>(0)?)
            })
            .map_err(|e| e.to_string())?;
        for row in rows {
            let id = row.map_err(|e| e.to_string())?;
            if seen.insert(id.clone()) {
                let name: Result<String, rusqlite::Error> = conn.query_row(
                    "SELECT name FROM agents WHERE id = ?1 AND is_deleted = 0",
                    [&id],
                    |row| row.get(0),
                );
                if let Ok(name) = name {
                    let persona: Result<String, rusqlite::Error> = conn.query_row(
                        "SELECT simplified_persona FROM agents WHERE id = ?1 AND is_deleted = 0",
                        [&id],
                        |row| row.get(0),
                    );
                    participants.push((name, "好友".to_string(), persona.unwrap_or_default()));
                }
            }
        }

        Ok(participants)
    }

    fn get_session_name(
        conn: &Connection,
        session_id: &str,
        viewer_agent_id: &str,
    ) -> Result<String, String> {
        // Try group session first
        let result: Result<String, rusqlite::Error> = conn.query_row(
            "SELECT name FROM group_sessions WHERE session_id = ?1",
            [session_id],
            |row| row.get(0),
        );
        if let Ok(name) = result {
            return Ok(name);
        }

        // Private session: show from viewer's perspective
        let result: Result<(String, String, String, String), rusqlite::Error> = conn.query_row(
            "SELECT participant_1_type, participant_1_id, participant_2_type, participant_2_id \
             FROM private_sessions WHERE session_id = ?1",
            [session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        );

        if let Ok((p1_type, p1_id, p2_type, p2_id)) = result {
            let other = if p1_type == "agent" && p1_id == viewer_agent_id {
                (p2_type, p2_id)
            } else if p2_type == "agent" && p2_id == viewer_agent_id {
                (p1_type, p1_id)
            } else {
                (p2_type, p2_id)
            };

            let other_name = if other.0 == "user" {
                Self::get_user_persona(conn).0
            } else {
                conn.query_row(
                    "SELECT name FROM agents WHERE id = ?1 AND is_deleted = 0",
                    [other.1],
                    |row| row.get(0),
                ).unwrap_or_else(|_| prompt_templates::UNKNOWN_SESSION.to_string())
            };

            return Ok(format!("和{}的私聊", other_name));
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
            LocalResult::Single(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
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
        conn.execute_batch(crate::db::schema::MIGRATION_V7).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V11).unwrap();
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
            "INSERT INTO private_sessions (session_id, participant_1_type, participant_1_id, participant_2_type, participant_2_id, created_at, current_chat_page) VALUES (?1, 'user', 'user', 'agent', ?2, ?3, ?4)",
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

        let prompt = PromptAssembler::assemble(&conn, "agent1", None, None, &[], &std::collections::HashSet::new()).unwrap();
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

        let prompt = PromptAssembler::assemble(&conn, "agent1", None, None, &[], &std::collections::HashSet::new()).unwrap();
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

        let prompt = PromptAssembler::assemble(&conn, "agent1", None, None, &[], &std::collections::HashSet::new()).unwrap();
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
        let prompt = PromptAssembler::assemble(&conn, "agent1", Some("sess1"), Some(0), &[], &std::collections::HashSet::new()).unwrap();
        assert!(prompt.contains("Page0 message"), "Prompt should contain page 0 message");
        assert!(!prompt.contains("Page1 message"), "Prompt should NOT contain page 1 message when triggered from page 0");

        // Trigger from page 1: prompt should contain "Page1 message" but NOT "Page0 message"
        let prompt = PromptAssembler::assemble(&conn, "agent1", Some("sess1"), Some(1), &[], &std::collections::HashSet::new()).unwrap();
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
        
        let prompt = PromptAssembler::assemble(&conn, "agent1", None, None, &[], &std::collections::HashSet::new()).unwrap();
        assert!(prompt.contains("Active session message"), "Prompt should contain active session message");
        assert!(!prompt.contains("Deleted group message"), "Prompt should NOT contain deleted session message, but got:\n{}", prompt);
    }

    #[test]
    fn test_private_session_name_from_agent_perspective() {
        let conn = init_test_db();
        insert_agent(&conn, "agent1", "远坂凛", "远坂家的继承人");
        insert_session(&conn, "sess1", "private");
        insert_private_session(&conn, "sess1", "agent1", 0);
        insert_session_settings(&conn, "sess1", 50);

        let msg = Message {
            id: "msg1".to_string(), session_id: "sess1".to_string(),
            sender_type: "user".to_string(), sender_id: "user".to_string(),
            content: "Hello".to_string(), created_at: 1000,
            message_type: "text".to_string(), tool_call_data: None,
            generation_info: None, is_deleted: false,
            sender_name: "用户".to_string(), sender_avatar: None, page_index: 0,
        };
        insert_message(&conn, &msg);

        let prompt = PromptAssembler::assemble(&conn, "agent1", None, None, &[], &std::collections::HashSet::new()).unwrap();
        assert!(prompt.contains("和用户的私聊"), "Private session name should be '和用户的私聊' from agent perspective, got:\n{}", prompt);
        assert!(!prompt.contains("远坂凛"), "Prompt should NOT contain agent name as session name");
    }

    #[test]
    fn test_user_persona_replacement() {
        let conn = init_test_db();
        insert_agent(&conn, "agent1", "远坂凛", "远坂家的继承人");
        insert_session(&conn, "sess1", "private");
        insert_private_session(&conn, "sess1", "agent1", 0);
        insert_session_settings(&conn, "sess1", 50);

        // Insert user persona
        conn.execute(
            "INSERT INTO user_personas (id, name, description, is_default, created_at, updated_at) VALUES (?1, ?2, ?3, 1, ?4, ?4)",
            ("persona1", "伊莉雅", "魔伊世界观中的小学生魔术师", 0i64),
        ).unwrap();
        conn.execute("INSERT INTO app_settings (id, updated_at) VALUES (1, 0)", []).unwrap();
        conn.execute("UPDATE app_settings SET active_persona_id = ?1 WHERE id = 1", ["persona1"]).unwrap();

        let msg = Message {
            id: "msg1".to_string(), session_id: "sess1".to_string(),
            sender_type: "user".to_string(), sender_id: "user".to_string(),
            content: "Hello".to_string(), created_at: 1000,
            message_type: "text".to_string(), tool_call_data: None,
            generation_info: None, is_deleted: false,
            sender_name: "用户".to_string(), sender_avatar: None, page_index: 0,
        };
        insert_message(&conn, &msg);

        let prompt = PromptAssembler::assemble(&conn, "agent1", None, None, &[], &std::collections::HashSet::new()).unwrap();
        assert!(prompt.contains("和伊莉雅的私聊"), "Private session name should use persona name");
        assert!(prompt.contains("伊莉雅（好友）：魔伊世界观中的小学生魔术师"), "Participant list should use persona");
    }

    #[test]
    fn test_no_duplicate_user_entry() {
        let conn = init_test_db();
        insert_agent(&conn, "agent1", "远坂凛", "远坂家的继承人");
        insert_session(&conn, "sess1", "private");
        insert_private_session(&conn, "sess1", "agent1", 0);
        insert_session_settings(&conn, "sess1", 50);

        let msg = Message {
            id: "msg1".to_string(), session_id: "sess1".to_string(),
            sender_type: "user".to_string(), sender_id: "user".to_string(),
            content: "Hello".to_string(), created_at: 1000,
            message_type: "text".to_string(), tool_call_data: None,
            generation_info: None, is_deleted: false,
            sender_name: "用户".to_string(), sender_avatar: None, page_index: 0,
        };
        insert_message(&conn, &msg);

        let prompt = PromptAssembler::assemble(&conn, "agent1", None, None, &[], &std::collections::HashSet::new()).unwrap();
        let user_count = prompt.matches("用户（好友）").count();
        assert_eq!(user_count, 1, "User entry should appear exactly once in participants, got {} occurrences", user_count);
    }
}
