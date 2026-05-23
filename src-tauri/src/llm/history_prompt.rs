use rusqlite::Connection;
use crate::llm::prompt_templates;
use crate::models::message::Message;

pub struct HistoryPromptAssembler;

impl HistoryPromptAssembler {
    pub fn assemble(
        conn: &Connection,
        agent_id: &str,
        session_id: &str,
        page_index: i32,
        history_messages: &[Message],
    ) -> Result<String, String> {
        // 1. 获取 Agent 自我设定
        let agent = crate::db::agent::get_by_id(conn, agent_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Agent not found: {}", agent_id))?;

        // 2. 格式化当前 session + page 的消息历史（反转使旧消息在上，新消息在下）
        let mut context = String::new();
        for msg in history_messages.iter().rev() {
            let time = crate::llm::prompt::PromptAssembler::format_time(msg.created_at);
            let sender = if msg.sender_type == "agent" && msg.sender_id == agent_id {
                agent.name.clone()
            } else {
                crate::llm::prompt::PromptAssembler::get_sender_name(conn, &msg.sender_type, &msg.sender_id)?
            };
            context.push_str(&format!("[{}] {}: {}\n", time, sender, msg.content));
        }

        // 3. 获取会话类型
        let session_type = crate::db::session::get_session_by_id(conn, session_id)
            .map_err(|e| e.to_string())?
            .map(|s| s.session_type)
            .unwrap_or_else(|| "unknown".to_string());

        // 4. 组装完整 prompt（注入工具使用说明，要求模型必须使用 send_message 工具回复）
        let instruction = format!(
            "请基于以上对话上下文继续回复。\n\n【工具使用说明】\n你可以使用 send_message 工具向指定会话发送消息。\n当前你正在以下会话中聊天：\n- session_id: {}, 类型: {}\n\n请根据上下文决定是否需要回复。如果需要回复，请调用 send_message 工具，参数如下：\n- target_type: \"{}\"\n- target_id: \"{}\"\n- content: 你要发送的消息内容\n\n注意：target_id 必须是上面列出的 session_id，不能使用名称或其他 ID。",
            session_id, session_type, session_type, session_id
        );

        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let full_prompt = format!(
            "{}\n\n{}\n\n{}\n\n{}",
            prompt_templates::SYSTEM_PROMPT.replace("{current_time}", &now),
            agent.detailed_persona,
            context,
            instruction
        );

        // 4. 记录完整 prompt 到日志
        crate::logger::info(&format!(
            "[HistoryPromptAssembler] Full prompt for agent {} | session={} | page={} | prompt_length={}\n---PROMPT START---\n{}\n---PROMPT END---",
            agent_id, session_id, page_index, full_prompt.len(), full_prompt
        ));

        Ok(full_prompt)
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
            "INSERT INTO private_sessions (session_id, participant_1_type, participant_1_id, participant_2_type, participant_2_id, created_at, current_chat_page) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (session_id, "user", "user", "agent", agent_id, 0i64, page),
        ).unwrap();
    }

    #[test]
    fn test_history_prompt_assemble_contains_agent_persona() {
        let conn = init_test_db();
        insert_agent(&conn, "agent1", "Test Agent", "I am a helpful assistant.");
        insert_session(&conn, "sess1", "private");
        insert_private_session(&conn, "sess1", "agent1", 0);

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

        let prompt = HistoryPromptAssembler::assemble(&conn, "agent1", "sess1", 0, &[msg]).unwrap();
        assert!(prompt.contains("I am a helpful assistant."), "Prompt should contain agent persona");
        assert!(prompt.contains("Hello"), "Prompt should contain message content");
        assert!(prompt.contains("请基于以上对话上下文继续回复"), "Prompt should contain instruction");
    }

    #[test]
    fn test_history_prompt_assemble_uses_sender_names() {
        let conn = init_test_db();
        insert_agent(&conn, "agent1", "Alice", "Persona 1");
        insert_agent(&conn, "agent2", "Bob", "Persona 2");
        insert_session(&conn, "sess1", "group");
        insert_private_session(&conn, "sess1", "agent1", 0);

        let msgs = vec![
            Message {
                id: "msg1".to_string(), session_id: "sess1".to_string(),
                sender_type: "user".to_string(), sender_id: "user".to_string(),
                content: "Hi everyone".to_string(), created_at: 1000,
                message_type: "text".to_string(), tool_call_data: None,
                generation_info: None, is_deleted: false,
                sender_name: "用户".to_string(), sender_avatar: None, page_index: 0,
            },
            Message {
                id: "msg2".to_string(), session_id: "sess1".to_string(),
                sender_type: "agent".to_string(), sender_id: "agent2".to_string(),
                content: "Hello from Bob".to_string(), created_at: 2000,
                message_type: "text".to_string(), tool_call_data: None,
                generation_info: None, is_deleted: false,
                sender_name: "Bob".to_string(), sender_avatar: None, page_index: 0,
            },
        ];

        let prompt = HistoryPromptAssembler::assemble(&conn, "agent1", "sess1", 0, &msgs).unwrap();
        assert!(prompt.contains("用户: Hi everyone"), "Prompt should contain user name");
        assert!(prompt.contains("Bob: Hello from Bob"), "Prompt should contain Bob's name");
    }

    #[test]
    fn test_history_prompt_empty_messages() {
        let conn = init_test_db();
        insert_agent(&conn, "agent1", "Alice", "Persona 1");
        insert_session(&conn, "sess1", "private");
        insert_private_session(&conn, "sess1", "agent1", 0);

        let prompt = HistoryPromptAssembler::assemble(&conn, "agent1", "sess1", 0, &[]).unwrap();
        assert!(prompt.contains("Persona 1"), "Prompt should contain agent persona even with no messages");
        assert!(prompt.contains("请基于以上对话上下文继续回复"), "Prompt should contain instruction");
    }

    #[test]
    fn test_history_prompt_self_agent_name_replacement() {
        let conn = init_test_db();
        insert_agent(&conn, "agent1", "Alice", "Persona 1");
        insert_session(&conn, "sess1", "private");
        insert_private_session(&conn, "sess1", "agent1", 0);

        let msgs = vec![
            Message {
                id: "msg1".to_string(), session_id: "sess1".to_string(),
                sender_type: "agent".to_string(), sender_id: "agent1".to_string(),
                content: "My own message".to_string(), created_at: 1000,
                message_type: "text".to_string(), tool_call_data: None,
                generation_info: None, is_deleted: false,
                sender_name: "Alice".to_string(), sender_avatar: None, page_index: 0,
            },
        ];

        let prompt = HistoryPromptAssembler::assemble(&conn, "agent1", "sess1", 0, &msgs).unwrap();
        assert!(prompt.contains("Alice: My own message"), "Prompt should use agent name for self messages");
    }

    #[test]
    fn test_history_prompt_chronological_order() {
        let conn = init_test_db();
        insert_agent(&conn, "agent1", "Alice", "Persona 1");
        insert_session(&conn, "sess1", "private");
        insert_private_session(&conn, "sess1", "agent1", 0);

        let msgs = vec![
            Message {
                id: "msg1".to_string(), session_id: "sess1".to_string(),
                sender_type: "user".to_string(), sender_id: "user".to_string(),
                content: "First".to_string(), created_at: 1000,
                message_type: "text".to_string(), tool_call_data: None,
                generation_info: None, is_deleted: false,
                sender_name: "用户".to_string(), sender_avatar: None, page_index: 0,
            },
            Message {
                id: "msg2".to_string(), session_id: "sess1".to_string(),
                sender_type: "user".to_string(), sender_id: "user".to_string(),
                content: "Second".to_string(), created_at: 2000,
                message_type: "text".to_string(), tool_call_data: None,
                generation_info: None, is_deleted: false,
                sender_name: "用户".to_string(), sender_avatar: None, page_index: 0,
            },
        ];

        let prompt = HistoryPromptAssembler::assemble(&conn, "agent1", "sess1", 0, &msgs).unwrap();
        let pos1 = prompt.find("First").expect("First message not found");
        let pos2 = prompt.find("Second").expect("Second message not found");
        assert!(pos1 < pos2, "Messages should be in chronological order (oldest first)");
    }

    #[test]
    fn test_history_prompt_different_page_index() {
        let conn = init_test_db();
        insert_agent(&conn, "agent1", "Alice", "Persona 1");
        insert_session(&conn, "sess1", "private");
        insert_private_session(&conn, "sess1", "agent1", 0);

        let msgs = vec![
            Message {
                id: "msg1".to_string(), session_id: "sess1".to_string(),
                sender_type: "user".to_string(), sender_id: "user".to_string(),
                content: "Page0 message".to_string(), created_at: 1000,
                message_type: "text".to_string(), tool_call_data: None,
                generation_info: None, is_deleted: false,
                sender_name: "用户".to_string(), sender_avatar: None, page_index: 0,
            },
        ];

        let prompt = HistoryPromptAssembler::assemble(&conn, "agent1", "sess1", 0, &msgs).unwrap();
        assert!(prompt.contains("Page0 message"));
        // The prompt should contain the session and page info in the log, but the actual
        // prompt content doesn't differ by page_index for a given message list
    }

    #[test]
    fn test_history_prompt_contains_tool_instruction() {
        let conn = init_test_db();
        insert_agent(&conn, "agent1", "Alice", "Persona 1");
        insert_session(&conn, "sess1", "private");
        insert_private_session(&conn, "sess1", "agent1", 0);

        let prompt = HistoryPromptAssembler::assemble(&conn, "agent1", "sess1", 0, &[]).unwrap();
        assert!(prompt.contains("send_message"), "Prompt should contain send_message tool instruction");
        assert!(prompt.contains("sess1"), "Prompt should contain session_id");
        assert!(prompt.contains("private"), "Prompt should contain session_type");
        assert!(prompt.contains("target_id"), "Prompt should contain target_id instruction");
    }
}
