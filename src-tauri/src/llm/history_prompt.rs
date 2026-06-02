use std::collections::HashMap;
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

        // 2. 加载快照参与者
        let snapshot_participants = if let Ok(Some(chat_page_id)) = 
            crate::db::chat_page_participant::get_chat_page_id(conn, session_id, page_index) {
            crate::db::chat_page_participant::list_by_chat_page(conn, &chat_page_id)
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let snapshot_map: HashMap<(String, String), crate::db::chat_page_participant::ChatPageParticipant> = 
            snapshot_participants.iter()
                .map(|p| ((p.participant_id.clone(), p.participant_type.clone()), p.clone()))
                .collect();

        // 3. 构建参与者介绍文本（基于快照）
        let mut participants_text = String::new();
        for p in &snapshot_participants {
            if p.participant_id == agent_id && p.participant_type == "agent" {
                continue; // 跳过当前 agent 自身
            }
            // 标签实时推导：优先查 friendships（好友），否则群友/用户
            // friendships 表对 agent-agent 友谊是双向存储的（add_friendship 插入两条记录），
            // 因此只需查 agent_id_1 = observer AND agent_id_2 = target 即可
            let label = if p.participant_type == "agent" {
                let is_friend: bool = conn.query_row(
                    "SELECT 1 FROM friendships WHERE agent_id_1 = ?1 AND agent_id_2 = ?2 AND participant_type_2 = 'agent'",
                    (agent_id, &p.participant_id),
                    |_| Ok(true),
                ).unwrap_or(false);
                if is_friend { "好友" } else { "群友" }
            } else {
                "用户"
            };
            let persona = p.participant_simplified_persona.as_deref().unwrap_or("");
            participants_text.push_str(&format!("- {}（{}）：{}\n", p.participant_name, label, persona));
            
            if p.participant_type == "agent" {
                // 查询 relationship_text 和 memory_text（实时）
                let (rel_text, mem_text): (String, String) = conn.query_row(
                    "SELECT COALESCE(relationship_text, ''), COALESCE(memory_text, '') 
                     FROM agent_relationships 
                     WHERE observer_id = ?1 AND target_id = ?2 AND target_type = 'agent'",
                    (agent_id, &p.participant_id),
                    |row| Ok((row.get(0)?, row.get(1)?)),
                ).unwrap_or_default();
                
                // 注：主 PromptAssembler 即使为空也会输出 [ impression ]：""
                // 历史模式下选择省略空值，减少 prompt 长度，不影响语义
                if !rel_text.is_empty() {
                    participants_text.push_str(&format!("  [印象]：{}\n", rel_text));
                }
                if agent.memory_enabled && !mem_text.is_empty() {
                    participants_text.push_str(&format!("  [记忆]：{}\n", mem_text));
                }
            }
        }

        // 4. 格式化当前 session + page 的消息历史（反转使旧消息在上，新消息在下）
        let mut context = String::new();
        for msg in history_messages.iter().rev() {
            let time = crate::llm::prompt::PromptAssembler::format_time(msg.created_at);
            let sender = if msg.sender_type == "agent" && msg.sender_id == agent_id {
                agent.name.clone()
            } else if let Some(snapshot) = snapshot_map.get(&(msg.sender_id.clone(), msg.sender_type.clone())) {
                snapshot.participant_name.clone()
            } else {
                crate::llm::prompt::PromptAssembler::get_sender_name(conn, &msg.sender_type, &msg.sender_id)?
            };
            context.push_str(&format!("[{}] {}: {}\n", time, sender, msg.content));
        }

        // 5. 获取会话类型
        let session_type = crate::db::session::get_session_by_id(conn, session_id)
            .map_err(|e| e.to_string())?
            .map(|s| s.session_type)
            .unwrap_or_else(|| "unknown".to_string());

        // 6. 组装完整 prompt
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let participants_section = if participants_text.is_empty() {
            String::new()
        } else {
            format!("【参与者】\n{}\n", participants_text)
        };

        let instruction = format!(
            "请基于以上对话上下文继续回复。\n\n【工具使用说明】\n你可以使用 send_message 工具向指定会话发送消息。\n当前你正在以下会话中聊天：\n- session_id: {}, 类型: {}\n\n请根据上下文决定是否需要回复。如果需要回复，请调用 send_message 工具，参数如下：\n- target_type: \"{}\"\n- target_id: \"{}\"\n- content: 你要发送的消息内容\n\n注意：target_id 必须是上面列出的 session_id，不能使用名称或其他 ID。",
            session_id, session_type, session_type, session_id
        );

        let full_prompt = format!(
            "{}\n\n{}\n\n{}{}\n\n{}",
            prompt_templates::SYSTEM_PROMPT.replace("{current_time}", &now),
            agent.detailed_persona,
            participants_section,
            context,
            instruction
        );

        // 7. 记录完整 prompt 到日志（仅 debug 级别）
        crate::logger::debug(&format!(
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
        conn.execute_batch(crate::db::schema::BASE_SCHEMA).unwrap();
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

    #[test]
    fn test_history_prompt_uses_snapshot_participants() {
        let conn = init_test_db();
        insert_agent(&conn, "agent1", "Alice", "Persona 1");
        insert_agent(&conn, "agent2", "Bob", "Persona 2");
        insert_session(&conn, "sess1", "group");
        
        // Insert chat_page and snapshot
        conn.execute(
            "INSERT INTO chat_pages (id, session_id, page_index, name, is_active, message_count, created_at, updated_at) VALUES ('cp-0', 'sess1', 0, 'Page 0', 1, 0, 1000, 1000)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO chat_page_participants (chat_page_id, participant_id, participant_type, participant_name, participant_avatar, participant_simplified_persona) VALUES ('cp-0', 'agent2', 'agent', 'Snapshot Bob', NULL, 'Bob snapshot persona')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO chat_page_participants (chat_page_id, participant_id, participant_type, participant_name, participant_avatar, participant_simplified_persona) VALUES ('cp-0', 'user-1', 'user', 'Snapshot User', NULL, NULL)",
            [],
        ).unwrap();

        let msgs = vec![
            Message {
                id: "msg1".to_string(), session_id: "sess1".to_string(),
                sender_type: "user".to_string(), sender_id: "user-1".to_string(),
                content: "Hi".to_string(), created_at: 1000,
                message_type: "text".to_string(), tool_call_data: None,
                generation_info: None, is_deleted: false,
                sender_name: "用户".to_string(), sender_avatar: None, page_index: 0,
            },
            Message {
                id: "msg2".to_string(), session_id: "sess1".to_string(),
                sender_type: "agent".to_string(), sender_id: "agent2".to_string(),
                content: "Hello".to_string(), created_at: 2000,
                message_type: "text".to_string(), tool_call_data: None,
                generation_info: None, is_deleted: false,
                sender_name: "Bob".to_string(), sender_avatar: None, page_index: 0,
            },
        ];

        let prompt = HistoryPromptAssembler::assemble(&conn, "agent1", "sess1", 0, &msgs).unwrap();
        assert!(prompt.contains("Snapshot Bob"), "Prompt should use snapshot name for agent participant");
        assert!(prompt.contains("Snapshot User"), "Prompt should use snapshot name for user participant");
        assert!(prompt.contains("Snapshot User: Hi"), "Message sender name should come from snapshot");
        assert!(prompt.contains("Snapshot Bob: Hello"), "Message sender name should come from snapshot");
    }
}
