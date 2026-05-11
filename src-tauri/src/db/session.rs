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
