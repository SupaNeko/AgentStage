use rusqlite::{Connection, Result, Row};
use crate::models::message::Message;
use uuid::Uuid;

const SELECT_COLUMNS: &str = "id, session_id, sender_type, sender_id, content, created_at, message_type, tool_call_data, generation_info, is_deleted, page_index";

fn row_to_message(row: &Row) -> Result<Message> {
    Ok(Message {
        id: row.get(0)?,
        session_id: row.get(1)?,
        sender_type: row.get(2)?,
        sender_id: row.get(3)?,
        sender_name: String::new(),
        sender_avatar: None,
        content: row.get(4)?,
        created_at: row.get(5)?,
        message_type: row.get(6)?,
        tool_call_data: row.get(7)?,
        generation_info: row.get(8)?,
        is_deleted: row.get::<_, i32>(9)? != 0,
        page_index: row.get(10)?,
    })
}

pub fn insert_message(
    conn: &Connection,
    session_id: &str,
    sender_type: &str,
    sender_id: &str,
    content: &str,
    message_type: &str,
    page_index: Option<i32>,
) -> Result<Message> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    let page = match page_index {
        Some(p) => p,
        None => conn.query_row(
            "SELECT COALESCE(current_chat_page, 0) FROM private_sessions WHERE session_id = ?1
             UNION ALL
             SELECT COALESCE(current_chat_page, 0) FROM group_sessions WHERE session_id = ?1
             LIMIT 1",
            [session_id],
            |row| row.get(0),
        ).unwrap_or(0),
    };

    conn.execute(
        r#"INSERT INTO messages (
            id, session_id, sender_type, sender_id, content, created_at, message_type, page_index
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
        (id.clone(), session_id, sender_type, sender_id, content, now, message_type, page),
    )?;

    get_message_by_id(conn, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn get_message_by_id(conn: &Connection, id: &str) -> Result<Option<Message>> {
    let mut stmt = conn.prepare(
        &format!("SELECT {} FROM messages WHERE id = ?1 AND is_deleted = 0", SELECT_COLUMNS)
    )?;
    let mut rows = stmt.query_map([id], row_to_message)?;
    rows.next().transpose()
}

pub fn get_messages_by_session(
    conn: &Connection,
    session_id: &str,
    page_index: i32,
    limit: i32,
    offset: i32,
) -> Result<Vec<Message>> {
    let mut stmt = conn.prepare(
        "SELECT m.id, m.session_id, m.sender_type, m.sender_id, 
                COALESCE(a.name, CASE WHEN m.sender_type = 'user' THEN '用户' ELSE '未知' END) as sender_name,
                a.avatar_path as sender_avatar,
                m.content, m.created_at, m.message_type, m.tool_call_data, m.generation_info, m.is_deleted, m.page_index
         FROM messages m
         LEFT JOIN agents a ON m.sender_type = 'agent' AND m.sender_id = a.id AND a.is_deleted = 0
         WHERE m.session_id = ?1 AND m.is_deleted = 0 AND m.page_index = ?2
         ORDER BY m.created_at DESC LIMIT ?3 OFFSET ?4"
    )?;
    let rows = stmt.query_map(rusqlite::params![session_id, page_index, limit, offset], |row| {
        Ok(Message {
            id: row.get(0)?,
            session_id: row.get(1)?,
            sender_type: row.get(2)?,
            sender_id: row.get(3)?,
            sender_name: row.get(4)?,
            sender_avatar: row.get(5)?,
            content: row.get(6)?,
            created_at: row.get(7)?,
            message_type: row.get(8)?,
            tool_call_data: row.get(9)?,
            generation_info: row.get(10)?,
            is_deleted: row.get::<_, i32>(11)? != 0,
            page_index: row.get(12)?,
        })
    })?;
    rows.collect()
}

pub fn get_visible_messages_for_agent(conn: &Connection, agent_id: &str) -> Result<Vec<Message>> {
    let sql = format!(
        "SELECT {} FROM messages \
         WHERE is_deleted = 0 \
         AND session_id IN ( \
             SELECT session_id FROM private_sessions WHERE agent_id = ?1 \
             UNION \
             SELECT session_id FROM group_members WHERE participant_id = ?1 AND participant_type = 'agent' \
         ) \
         ORDER BY created_at ASC",
        SELECT_COLUMNS
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([agent_id], row_to_message)?;
    rows.collect()
}

pub fn get_pending_messages_for_agent(
    conn: &Connection,
    agent_id: &str,
    last_trigger_time: i64,
) -> Result<Vec<Message>> {
    let sql = format!(
        "SELECT {} FROM messages \
         WHERE is_deleted = 0 \
         AND session_id IN ( \
             SELECT session_id FROM private_sessions WHERE agent_id = ?1 \
             UNION \
             SELECT session_id FROM group_members WHERE participant_id = ?1 AND participant_type = 'agent' \
         ) \
         AND created_at > ?2 \
         AND NOT (sender_type = 'agent' AND sender_id = ?1) \
         ORDER BY created_at ASC",
        SELECT_COLUMNS
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![agent_id, last_trigger_time], row_to_message)?;
    rows.collect()
}
