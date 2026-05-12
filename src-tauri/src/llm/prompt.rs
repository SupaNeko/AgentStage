use chrono::{Local, LocalResult, TimeZone};
use rusqlite::Connection;
use std::collections::{HashMap, HashSet};

use crate::models::message::Message;

pub struct PromptAssembler;

impl PromptAssembler {
    const SYSTEM_PROMPT: &'static str = "你是一个正在参与即时通讯聊天的 AI 角色。请根据上下文自然地回应。\n你可以同时参与多个私聊和群聊，在回复时请根据上下文判断应该回复哪个会话。\n如果需要回复多个会话，可以多次调用 send_message 工具。\n请注意：你每次被调用时，都会看到自上次回复以来积累的所有新消息。请综合考虑这些消息后再决定如何回应。";

    pub fn assemble(
        conn: &Connection,
        agent_id: &str,
        pending_messages: &[Message],
    ) -> Result<String, String> {
        crate::logger::backend("DEBUG", &format!("[DEBUG prompt::assemble] agent_id={}, pending_messages={}", agent_id, pending_messages.len()));

        let mut layers: Vec<String> = Vec::new();

        // Layer 1: System Prompt
        layers.push(Self::SYSTEM_PROMPT.to_string());

        // Layer 2: Self Persona
        let agent = Self::get_agent(conn, agent_id)?;
        layers.push(format!("【你的角色设定】\n{}", agent.detailed_persona));

        // Layer 3: Participants Introduction
        let participants = Self::get_participants(conn, agent_id)?;
        if !participants.is_empty() {
            let mut layer = String::from("【你认识的参与者】\n");
            for (name, relation, persona) in participants {
                layer.push_str(&format!("- {}（{}）：{}\n", name, relation, persona));
            }
            layer.push_str("- 用户（真实用户）：正在与你聊天的真实用户。");
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
                        a.avatar_path as sender_avatar
                 FROM messages m
                 JOIN (
                     SELECT session_id, COALESCE(current_chat_page, 0) as page FROM private_sessions WHERE agent_id = ?1
                     UNION
                     SELECT gs.session_id, COALESCE(gs.current_chat_page, 0) as page 
                     FROM group_sessions gs
                     JOIN group_members gm ON gs.session_id = gm.session_id
                     WHERE gm.participant_id = ?1 AND gm.participant_type = 'agent'
                 ) sp ON m.session_id = sp.session_id AND m.page_index = sp.page
                 LEFT JOIN agents a ON m.sender_type = 'agent' AND m.sender_id = a.id AND a.is_deleted = 0
                 WHERE m.is_deleted = 0
                 ORDER BY m.created_at DESC"
            ).map_err(|e| e.to_string())?;
            
            let rows = stmt.query_map([agent_id], |row| {
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
            let mut layer = String::from("【历史聊天记录】\n");
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
            layers.push(layer);
        }

        // Layer 5: Latest Messages
        if !pending_messages.is_empty() {
            let mut layer = String::from("【最新消息 - 需要你回应的消息】\n");
            for msg in pending_messages {
                let time = Self::format_time(msg.created_at);
                let sender = Self::get_sender_name(conn, &msg.sender_type, &msg.sender_id)?;
                let session_name = Self::get_session_name(conn, &msg.session_id)?;
                layer.push_str(&format!(
                    "[{}] {} 在 {} 中说：{}\n",
                    time, sender, session_name, msg.content
                ));
            }
            layers.push(layer);
        }

        // Layer 6: Instruction（工具使用说明 + 当前上下文 ID）
        let instruction = Self::build_instruction(conn, agent_id, &agent.name)?;
        layers.push(instruction);

        let prompt = layers.join("\n\n");
        let prompt_with_vars = Self::apply_variables(&prompt, &agent.name);

        crate::logger::backend("DEBUG", &format!("[DEBUG prompt::assemble] agent_id={}, total_chars={}", agent_id, prompt_with_vars.len()));
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
            r#"【工具使用说明】
你可以使用 send_message 工具向指定会话发送消息。
当前你正在以下会话中聊天：
{context_list}
请根据上下文决定是否需要回复，以及回复哪个会话。
如果需要回复，请调用 send_message 工具，参数如下：
- target_type: "private" 或 "group"
- target_id: 目标会话的 session_id（必须是上面列出的 ID 之一）
- content: 你要发送的消息内容

注意：你只能向上面列出的会话发送消息。target_id 必须是完整的 session_id，不能使用名称或其他 ID。"#,
            context_list = context_list
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

    /// 变量替换：{{char}} → agent_name, {{user}} → "用户"
    fn apply_variables(prompt: &str, agent_name: &str) -> String {
        prompt
            .replace("{{char}}", agent_name)
            .replace("{{user}}", "用户")
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

        Ok("未知会话".to_string())
    }

    fn get_sender_name(
        conn: &Connection,
        sender_type: &str,
        sender_id: &str,
    ) -> Result<String, String> {
        match sender_type {
            "user" => Ok("用户".to_string()),
            "system" => Ok("系统".to_string()),
            "agent" => {
                let result: Result<String, rusqlite::Error> = conn.query_row(
                    "SELECT name FROM agents WHERE id = ?1 AND is_deleted = 0",
                    [sender_id],
                    |row| row.get(0),
                );
                match result {
                    Ok(name) => Ok(name),
                    Err(rusqlite::Error::QueryReturnedNoRows) => {
                        Ok(format!("未知角色({})", sender_id))
                    }
                    Err(e) => Err(e.to_string()),
                }
            }
            _ => Ok(format!("未知({})", sender_type)),
        }
    }

    fn format_time(timestamp_ms: i64) -> String {
        match Local.timestamp_millis_opt(timestamp_ms) {
            LocalResult::Single(dt) => dt.format("%H:%M").to_string(),
            _ => "??".to_string(),
        }
    }
}
