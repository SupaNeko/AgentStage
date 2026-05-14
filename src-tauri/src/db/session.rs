use rusqlite::{Connection, Result, Row};
use crate::models::session::SessionResponse;
use uuid::Uuid;

const SELECT_COLUMNS: &str = "s.id, s.session_type, s.last_message_at, s.last_message_preview, s.unread_count, ps.agent_id, a.name, a.avatar_path, gs.name, gs.avatar_path, gs.mute_enabled";

fn row_to_session_response(row: &Row) -> Result<SessionResponse> {
    Ok(SessionResponse {
        id: row.get(0)?,
        session_type: row.get(1)?,
        last_message_at: row.get(2)?,
        last_message_preview: row.get(3)?,
        unread_count: row.get(4)?,
        agent_id: row.get(5)?,
        agent_name: row.get(6)?,
        agent_avatar: row.get(7)?,
        group_name: row.get(8)?,
        group_avatar: row.get(9)?,
        mute_enabled: row.get::<_, Option<i32>>(10)?.map(|v| v != 0),
    })
}

pub fn get_private_session_by_agent_id(conn: &Connection, agent_id: &str) -> Result<Option<SessionResponse>> {
    let mut stmt = conn.prepare(
        &format!(
            "SELECT {} FROM sessions s \
             LEFT JOIN private_sessions ps ON s.id = ps.session_id \
             LEFT JOIN agents a ON ps.agent_id = a.id \
             LEFT JOIN group_sessions gs ON s.id = gs.session_id \
             WHERE s.is_deleted = 0 AND ps.agent_id = ?1 AND s.session_type = 'private'",
            SELECT_COLUMNS
        )
    )?;
    let mut rows = stmt.query_map([agent_id], row_to_session_response)?;
    rows.next().transpose()
}

pub fn create_private_session(conn: &Connection, agent_id: &str) -> Result<SessionResponse> {
    // 如果已有该角色的私聊会话，直接返回已有会话
    if let Some(existing) = get_private_session_by_agent_id(conn, agent_id)? {
        return Ok(existing);
    }

    let session_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    let tx = conn.unchecked_transaction()?;

    conn.execute(
        "INSERT INTO sessions (id, session_type, created_at, updated_at) VALUES (?1, 'private', ?2, ?3)",
        (&session_id, now, now),
    )?;

    conn.execute(
        "INSERT INTO private_sessions (session_id, agent_id, message_limit_enabled, created_at) VALUES (?1, ?2, 1, ?3)",
        (&session_id, agent_id, now),
    )?;

    init_session_settings(&conn, &session_id, "private")?;

    // 自动建立好友关系（该角色与用户）
    conn.execute(
        "INSERT INTO friendships (id, agent_id_1, participant_type_2, created_at, source_session_id) VALUES (?1, ?2, 'user', ?3, ?4)",
        (&Uuid::new_v4().to_string(), agent_id, now, &session_id),
    )?;

    tx.commit()?;

    get_session_by_id(conn, &session_id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn get_session_by_id(conn: &Connection, session_id: &str) -> Result<Option<SessionResponse>> {
    let mut stmt = conn.prepare(
        &format!(
            "SELECT {} FROM sessions s \
             LEFT JOIN private_sessions ps ON s.id = ps.session_id \
             LEFT JOIN agents a ON ps.agent_id = a.id \
             LEFT JOIN group_sessions gs ON s.id = gs.session_id \
             WHERE s.id = ?1 AND s.is_deleted = 0",
            SELECT_COLUMNS
        )
    )?;
    let mut rows = stmt.query_map([session_id], row_to_session_response)?;
    rows.next().transpose()
}

pub fn list_sessions(conn: &Connection) -> Result<Vec<SessionResponse>> {
    let mut stmt = conn.prepare(
        &format!(
            "SELECT {} FROM sessions s \
             LEFT JOIN private_sessions ps ON s.id = ps.session_id \
             LEFT JOIN agents a ON ps.agent_id = a.id \
             LEFT JOIN group_sessions gs ON s.id = gs.session_id \
             WHERE s.is_deleted = 0 \
             ORDER BY s.last_message_at DESC",
            SELECT_COLUMNS
        )
    )?;
    let rows = stmt.query_map([], row_to_session_response)?;
    rows.collect()
}

pub fn soft_delete_session(conn: &Connection, session_id: &str) -> Result<bool> {
    let now = chrono::Utc::now().timestamp_millis();
    let rows = conn.execute(
        "UPDATE sessions SET is_deleted = 1, deleted_at = ?2 WHERE id = ?1 AND is_deleted = 0",
        (session_id, now),
    )?;
    Ok(rows > 0)
}

pub fn update_session_last_message(conn: &Connection, session_id: &str, preview: &str) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE sessions SET last_message_at = ?2, last_message_preview = ?3, updated_at = ?4 WHERE id = ?1",
        (session_id, now, preview, now),
    )?;
    Ok(())
}

pub fn create_group_session(
    conn: &Connection,
    name: &str,
    agent_ids: &[String],
) -> Result<SessionResponse> {
    if agent_ids.len() < 2 {
        return Err(rusqlite::Error::InvalidParameterName(
            "群聊至少需要选择 2 个角色".into()
        ));
    }

    let session_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let tx = conn.unchecked_transaction()?;

    conn.execute(
        "INSERT INTO sessions (id, session_type, created_at, updated_at) VALUES (?1, 'group', ?2, ?3)",
        (&session_id, now, now),
    )?;

    conn.execute(
        "INSERT INTO group_sessions (session_id, name, mute_enabled, created_at) VALUES (?1, ?2, 0, ?3)",
        (&session_id, name, now),
    )?;

    init_session_settings(&conn, &session_id, "group")?;

    conn.execute(
        "INSERT INTO group_members (session_id, participant_type, participant_id, joined_at) VALUES (?1, 'user', 'user', ?2)",
        (&session_id, now),
    )?;

    for agent_id in agent_ids {
        conn.execute(
            "INSERT INTO group_members (session_id, participant_type, participant_id, joined_at) VALUES (?1, 'agent', ?2, ?3)",
            (&session_id, agent_id, now),
        )?;
    }

    tx.commit()?;
    get_session_by_id(conn, &session_id)?
        .ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn get_group_members(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<crate::models::session::GroupMemberResponse>> {
    let mut stmt = conn.prepare(
        "SELECT gm.participant_type, gm.participant_id,
                CASE WHEN gm.participant_type = 'user' THEN '用户' ELSE COALESCE(a.name, '未知角色') END as name,
                a.avatar_path
         FROM group_members gm
         LEFT JOIN agents a ON gm.participant_type = 'agent' AND gm.participant_id = a.id
         WHERE gm.session_id = ?1 AND gm.is_active = 1
         ORDER BY gm.participant_type DESC, name ASC"
    )?;
    let rows = stmt.query_map([session_id], |row| {
        Ok(crate::models::session::GroupMemberResponse {
            participant_type: row.get(0)?,
            participant_id: row.get(1)?,
            name: row.get(2)?,
            avatar_path: row.get(3)?,
        })
    })?;
    rows.collect()
}

pub fn get_session_config(conn: &Connection, session_id: &str, session_type: &str) -> Result<crate::models::session::SessionConfig> {
    let defaults = if session_type == "private" {
        (30, 10)
    } else {
        (80, 30)
    };

    conn.query_row(
        "SELECT ss.session_id, COALESCE(ss.history_limit, ?2), COALESCE(ss.message_limit, ?3),
                ss.message_limit_enabled, ss.mute_enabled,
                COALESCE(ps.agent_message_count, gs.agent_message_count, 0)
         FROM session_settings ss
         LEFT JOIN private_sessions ps ON ss.session_id = ps.session_id
         LEFT JOIN group_sessions gs ON ss.session_id = gs.session_id
         WHERE ss.session_id = ?1",
        rusqlite::params![session_id, defaults.0, defaults.1],
        |row| {
            Ok(crate::models::session::SessionConfig {
                session_id: row.get(0)?,
                history_limit: row.get(1)?,
                message_limit: row.get(2)?,
                message_limit_enabled: row.get::<_, i32>(3)? != 0,
                mute_enabled: row.get::<_, i32>(4)? != 0,
                agent_message_count: row.get(5)?,
            })
        },
    )
}

pub fn reset_message_count(conn: &Connection, session_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE private_sessions SET agent_message_count = 0 WHERE session_id = ?1",
        [session_id],
    )?;
    conn.execute(
        "UPDATE group_sessions SET agent_message_count = 0 WHERE session_id = ?1",
        [session_id],
    )?;
    Ok(())
}

pub fn init_session_settings(conn: &Connection, session_id: &str, session_type: &str) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let (history_limit, message_limit) = if session_type == "private" {
        (30, 10)
    } else {
        (80, 30)
    };
    conn.execute(
        "INSERT OR IGNORE INTO session_settings (session_id, history_limit, message_limit, message_limit_enabled, mute_enabled, created_at, updated_at)
         VALUES (?1, ?2, ?3, 1, 0, ?4, ?4)",
        rusqlite::params![session_id, history_limit, message_limit, now],
    )?;
    Ok(())
}

pub fn update_session_config(conn: &Connection, req: &crate::models::session::UpdateSessionConfigRequest) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();

    let history_limit = req.history_limit;
    let message_limit = req.message_limit;
    let message_limit_enabled = req.message_limit_enabled.map(|v| v as i32);
    let mute_enabled = req.mute_enabled.map(|v| v as i32);

    let mut sets = Vec::new();
    let mut params: Vec<&dyn rusqlite::ToSql> = Vec::new();

    if let Some(ref v) = history_limit {
        sets.push("history_limit = ?");
        params.push(v as &dyn rusqlite::ToSql);
    }
    if let Some(ref v) = message_limit {
        sets.push("message_limit = ?");
        params.push(v as &dyn rusqlite::ToSql);
    }
    if let Some(ref v) = message_limit_enabled {
        sets.push("message_limit_enabled = ?");
        params.push(v as &dyn rusqlite::ToSql);
    }
    if let Some(ref v) = mute_enabled {
        sets.push("mute_enabled = ?");
        params.push(v as &dyn rusqlite::ToSql);
    }

    if sets.is_empty() {
        return Ok(());
    }

    sets.push("updated_at = ?");
    params.push(&now as &dyn rusqlite::ToSql);
    params.push(&req.session_id as &dyn rusqlite::ToSql);

    let sql = format!("UPDATE session_settings SET {} WHERE session_id = ?", sets.join(", "));
    conn.execute(&sql, rusqlite::params_from_iter(params))?;
    Ok(())
}

pub fn reset_session(conn: &Connection, session_id: &str) -> Result<String> {
    let now = chrono::Utc::now().timestamp_millis();
    let tx = conn.unchecked_transaction()?;

    let max_page: i32 = conn.query_row(
        "SELECT COALESCE(MAX(page_index), 0) FROM chat_pages WHERE session_id = ?1",
        [session_id],
        |row| row.get(0),
    ).unwrap_or(0);

    let new_page_index = max_page + 1;
    let page_id = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO chat_pages (id, session_id, page_index, name, is_active, message_count, created_at, updated_at)
         VALUES (?1, ?2, ?3, '续开', 1, 0, ?4, ?4)",
        rusqlite::params![&page_id, session_id, new_page_index, now],
    )?;

    let session_type: String = conn.query_row(
        "SELECT session_type FROM sessions WHERE id = ?1",
        [session_id],
        |row| row.get(0),
    )?;

    if session_type == "private" {
        conn.execute(
            "UPDATE private_sessions SET current_chat_page = ?1, agent_message_count = 0 WHERE session_id = ?2",
            rusqlite::params![new_page_index, session_id],
        )?;
    } else {
        conn.execute(
            "UPDATE group_sessions SET current_chat_page = ?1, agent_message_count = 0 WHERE session_id = ?2",
            rusqlite::params![new_page_index, session_id],
        )?;
    }

    // 清空未读消息
    conn.execute("DELETE FROM agent_unread_queue WHERE session_id = ?1", [session_id])?;

    // 解除冻结
    conn.execute("DELETE FROM session_frozen_states WHERE session_id = ?1", [session_id])?;

    tx.commit()?;
    Ok(page_id)
}

pub fn disband_group(conn: &Connection, session_id: &str) -> Result<bool> {
    let now = chrono::Utc::now().timestamp_millis();
    let rows = conn.execute(
        "UPDATE sessions SET is_deleted = 1, deleted_at = ?2 WHERE id = ?1 AND session_type = 'group'",
        (session_id, now),
    )?;
    Ok(rows > 0)
}

pub fn add_group_member(conn: &Connection, session_id: &str, agent_id: &str) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let tx = conn.unchecked_transaction()?;

    conn.execute(
        "INSERT OR IGNORE INTO group_members (session_id, participant_type, participant_id, joined_at) VALUES (?1, 'agent', ?2, ?3)",
        (session_id, agent_id, now),
    )?;

    let other_agents: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT participant_id FROM group_members
             WHERE session_id = ?1 AND participant_type = 'agent' AND participant_id != ?2"
        )?;
        let rows = stmt.query_map([session_id, agent_id], |row| row.get(0))?;
        rows.filter_map(|r| r.ok()).collect()
    };

    for other_id in other_agents {
        conn.execute(
            "INSERT OR IGNORE INTO friendships (id, agent_id_1, agent_id_2, participant_type_2, created_at, source_session_id)
             VALUES (?1, ?2, ?3, 'agent', ?4, ?5)",
            rusqlite::params![uuid::Uuid::new_v4().to_string(), agent_id, &other_id, now, session_id],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO friendships (id, agent_id_1, agent_id_2, participant_type_2, created_at, source_session_id)
             VALUES (?1, ?2, ?3, 'agent', ?4, ?5)",
            rusqlite::params![uuid::Uuid::new_v4().to_string(), &other_id, agent_id, now, session_id],
        )?;
    }

    tx.commit()?;
    Ok(())
}

pub fn remove_group_member(conn: &Connection, session_id: &str, agent_id: &str) -> Result<bool> {
    let rows = conn.execute(
        "DELETE FROM group_members WHERE session_id = ?1 AND participant_type = 'agent' AND participant_id = ?2",
        (session_id, agent_id),
    )?;
    Ok(rows > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = OFF;", []).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V1).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V2).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V3).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V4).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V5).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V6).unwrap();
        conn
    }

    #[test]
    fn test_v6_migration_creates_frozen_states_and_unread_queue() {
        let conn = init_test_db();
        
        // Verify session_frozen_states exists
        conn.execute(
            "INSERT INTO session_frozen_states (session_id, is_frozen, frozen_at, updated_at) VALUES ('test', 1, 0, 0)",
            [],
        ).unwrap();
        
        // Verify agent_unread_queue exists
        conn.execute(
            "INSERT INTO agent_unread_queue (session_id, agent_id, message_id, created_at) VALUES ('test', 'agent1', 'msg1', 0)",
            [],
        ).unwrap();
        
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM agent_unread_queue WHERE session_id = 'test'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_create_group_session_min_2_agents() {
        let conn = init_test_db();
        let result = create_group_session(&conn, "Test Group", &["agent1".into()]);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_group_session_and_get_members() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Agent One", 0i64),
        ).unwrap();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent2", "Agent Two", 0i64),
        ).unwrap();

        let session = create_group_session(&conn, "Test Group", &["agent1".into(), "agent2".into()]).unwrap();
        assert_eq!(session.session_type, "group");
        assert_eq!(session.group_name, Some("Test Group".into()));

        let members = get_group_members(&conn, &session.id).unwrap();
        assert_eq!(members.len(), 3);
        assert_eq!(members[0].participant_type, "user");
        assert_eq!(members[0].name, "用户");
        assert_eq!(members[1].name, "Agent One");
        assert_eq!(members[2].name, "Agent Two");
    }

    #[test]
    fn test_session_config_defaults() {
        let conn = init_test_db();
        
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        let config = get_session_config(&conn, &session.id, "private").unwrap();
        assert_eq!(config.history_limit, 30);
        assert_eq!(config.message_limit, 10);
        assert!(config.message_limit_enabled);
        assert!(!config.mute_enabled);
    }

    #[test]
    fn test_update_session_config() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        update_session_config(&conn, &crate::models::session::UpdateSessionConfigRequest {
            session_id: session.id.clone(),
            history_limit: Some(50),
            message_limit: Some(20),
            message_limit_enabled: Some(false),
            mute_enabled: Some(true),
        }).unwrap();
        
        let config = get_session_config(&conn, &session.id, "private").unwrap();
        assert_eq!(config.history_limit, 50);
        assert_eq!(config.message_limit, 20);
        assert!(!config.message_limit_enabled);
        assert!(config.mute_enabled);
    }

    #[test]
    fn test_reset_session_creates_new_page() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        let page_id = reset_session(&conn, &session.id).unwrap();
        assert!(!page_id.is_empty());
        
        let page_index: i32 = conn.query_row(
            "SELECT page_index FROM chat_pages WHERE id = ?1",
            [&page_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(page_index, 1);
    }

    #[test]
    fn test_prompt_assemble_with_new_session() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, ' detailed', 'simple', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        // Insert a user message
        crate::db::message::insert_message(&conn, &session.id, "user", "user", "Hello!", "text", None).unwrap();
        
        // Assemble prompt
        let pending = vec![crate::models::message::Message {
            id: "test-msg".to_string(),
            session_id: session.id.clone(),
            sender_type: "user".to_string(),
            sender_id: "user".to_string(),
            sender_name: "用户".to_string(),
            sender_avatar: None,
            content: "Hello!".to_string(),
            created_at: chrono::Utc::now().timestamp_millis(),
            message_type: "text".to_string(),
            tool_call_data: None,
            generation_info: None,
            is_deleted: false,
            page_index: 0,
        }];
        
        let prompt = crate::llm::prompt::PromptAssembler::assemble(&conn, "agent1", None, None, &pending);
        assert!(prompt.is_ok(), "PromptAssembler failed: {:?}", prompt.err());
        let prompt_text = prompt.unwrap();
        assert!(prompt_text.contains("Hello!"));
        assert!(prompt_text.contains("Test Agent"));
    }

    #[test]
    fn test_get_messages_by_session_returns_agent_messages() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        // Insert user message
        let user_msg = crate::db::message::insert_message(&conn, &session.id, "user", "user", "Hello!", "text", None).unwrap();
        assert_eq!(user_msg.sender_type, "user");
        
        // Insert agent message
        let agent_msg = crate::db::message::insert_message(&conn, &session.id, "agent", "agent1", "Hi there!", "text", None).unwrap();
        assert_eq!(agent_msg.sender_type, "agent");
        
        // Query messages
        let messages = crate::db::message::get_messages_by_session(&conn, &session.id, 0, 100, 0).unwrap();
        assert_eq!(messages.len(), 2, "Expected 2 messages, got {}", messages.len());
        
        let agent_messages: Vec<_> = messages.iter().filter(|m| m.sender_type == "agent").collect();
        assert_eq!(agent_messages.len(), 1, "Agent message missing from query results");
        assert_eq!(agent_messages[0].content, "Hi there!");
    }

    #[test]
    fn test_session_config_includes_agent_message_count() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        conn.execute(
            "UPDATE private_sessions SET agent_message_count = 5 WHERE session_id = ?1",
            [&session.id],
        ).unwrap();
        
        let config = get_session_config(&conn, &session.id, "private").unwrap();
        assert_eq!(config.agent_message_count, 5);
    }

    #[test]
    fn test_reset_message_count() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        conn.execute(
            "UPDATE private_sessions SET agent_message_count = 5 WHERE session_id = ?1",
            [&session.id],
        ).unwrap();
        
        reset_message_count(&conn, &session.id).unwrap();
        
        let count: i32 = conn.query_row(
            "SELECT agent_message_count FROM private_sessions WHERE session_id = ?1",
            [&session.id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_list_chat_pages_returns_pages_in_desc_order() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        // Reset once to create page 1
        let _ = reset_session(&conn, &session.id).unwrap();
        
        let pages = crate::db::chat_page::list_chat_pages(&conn, &session.id).unwrap();
        assert_eq!(pages.len(), 2, "Expected 2 pages (default + reset), got {}", pages.len());
        assert_eq!(pages[0].page_index, 1); // DESC order: newest first
        assert_eq!(pages[1].page_index, 0);
    }

    #[test]
    fn test_list_chat_pages_aggregates_message_stats() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        // Insert message to page 0
        crate::db::message::insert_message(&conn, &session.id, "user", "user", "Hello", "text", Some(0)).unwrap();
        
        let pages = crate::db::chat_page::list_chat_pages(&conn, &session.id).unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].message_count, 1);
        assert!(pages[0].updated_at > pages[0].created_at, "updated_at should reflect last message time");
    }

    #[test]
    #[ignore]
    fn diagnose_real_db() {
        let conn = rusqlite::Connection::open(r"D:\code_project\AgentStage\data\agentstage.db").unwrap();
        let mut stmt = conn.prepare("SELECT id, session_id, sender_type, sender_id, content, page_index FROM messages ORDER BY created_at").unwrap();
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i32>(5)?,
            ))
        }).unwrap();
        for row in rows {
            let (id, sid, st, sid2, content, page): (String, String, String, String, String, i32) = row.unwrap();
            eprintln!("msg: id={} session={} sender_type={} sender_id={} page={} content={}", id, sid, st, sid2, page, content.chars().take(30).collect::<String>());
        }
        
        let mut stmt = conn.prepare("SELECT session_id, current_chat_page FROM private_sessions").unwrap();
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
        }).unwrap();
        for row in rows {
            let (sid, page): (String, i32) = row.unwrap();
            eprintln!("private_session: session={} current_chat_page={}", sid, page);
        }
        
        let mut stmt = conn.prepare("SELECT session_id, current_chat_page FROM group_sessions").unwrap();
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
        }).unwrap();
        for row in rows {
            let (sid, page): (String, i32) = row.unwrap();
            eprintln!("group_session: session={} current_chat_page={}", sid, page);
        }
    }
}
