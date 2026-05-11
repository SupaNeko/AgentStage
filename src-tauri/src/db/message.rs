use rusqlite::{Connection, Result, Row};
use crate::models::message::Message;
use uuid::Uuid;

const SELECT_COLUMNS: &str = "id, session_id, sender_type, sender_id, content, created_at, message_type, tool_call_data, generation_info, is_deleted";

fn row_to_message(row: &Row) -> Result<Message> {
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
    })
}

pub fn insert_message(
    conn: &Connection,
    session_id: &str,
    sender_type: &str,
    sender_id: &str,
    content: &str,
    message_type: &str,
) -> Result<Message> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    conn.execute(
        r#"INSERT INTO messages (
            id, session_id, sender_type, sender_id, content, created_at, message_type
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
        (id.clone(), session_id, sender_type, sender_id, content, now, message_type),
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
    limit: i32,
    offset: i32,
) -> Result<Vec<Message>> {
    let mut stmt = conn.prepare(
        &format!(
            "SELECT {} FROM messages WHERE session_id = ?1 AND is_deleted = 0 ORDER BY created_at DESC LIMIT ?2 OFFSET ?3",
            SELECT_COLUMNS
        )
    )?;
    let rows = stmt.query_map(rusqlite::params![session_id, limit, offset], row_to_message)?;
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
