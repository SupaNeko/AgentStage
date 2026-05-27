use rusqlite::{Connection, Result};
use std::collections::HashMap;

pub fn insert_unread(conn: &Connection, session_id: &str, agent_id: &str, message_id: &str, created_at: i64) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO agent_unread_queue (session_id, agent_id, message_id, created_at) VALUES (?1, ?2, ?3, ?4)",
        (session_id, agent_id, message_id, created_at),
    )?;
    Ok(())
}

pub fn get_unread_by_agent(conn: &Connection, agent_id: &str) -> Result<HashMap<String, Vec<String>>> {
    let mut stmt = conn.prepare(
        "SELECT session_id, message_id FROM agent_unread_queue WHERE agent_id = ?1 ORDER BY created_at ASC"
    )?;
    let rows = stmt.query_map([agent_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows {
        let (session_id, message_id) = row?;
        map.entry(session_id).or_default().push(message_id);
    }
    Ok(map)
}

pub fn delete_unread_by_agent_session(conn: &Connection, agent_id: &str, session_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM agent_unread_queue WHERE agent_id = ?1 AND session_id = ?2",
        (agent_id, session_id),
    )?;
    Ok(())
}

pub fn clear_unread_by_session(conn: &Connection, session_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM agent_unread_queue WHERE session_id = ?1",
        [session_id],
    )?;
    Ok(())
}

pub fn get_agents_with_unread(conn: &Connection, session_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT agent_id FROM agent_unread_queue WHERE session_id = ?1"
    )?;
    let rows = stmt.query_map([session_id], |row| {
        row.get::<_, String>(0)
    })?;
    rows.collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn init_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = OFF;", []).unwrap();
        conn.execute_batch(crate::db::schema::BASE_SCHEMA).unwrap();
        conn
    }

    #[test]
    fn test_insert_and_get_unread() {
        let conn = init_test_db();
        insert_unread(&conn, "session-1", "agent-1", "msg-1", 1000).unwrap();
        insert_unread(&conn, "session-1", "agent-1", "msg-2", 2000).unwrap();
        insert_unread(&conn, "session-2", "agent-1", "msg-3", 3000).unwrap();
        insert_unread(&conn, "session-1", "agent-2", "msg-4", 4000).unwrap();

        let unread = get_unread_by_agent(&conn, "agent-1").unwrap();
        assert_eq!(unread.len(), 2);
        let agent1_session1 = unread.get("session-1").unwrap();
        assert_eq!(agent1_session1.len(), 2);
        assert!(agent1_session1.contains(&"msg-1".to_string()));
        assert!(agent1_session1.contains(&"msg-2".to_string()));
        let agent1_session2 = unread.get("session-2").unwrap();
        assert_eq!(agent1_session2.len(), 1);
        assert!(agent1_session2.contains(&"msg-3".to_string()));
    }

    #[test]
    fn test_delete_unread_by_agent_session() {
        let conn = init_test_db();
        insert_unread(&conn, "session-1", "agent-1", "msg-1", 1000).unwrap();
        insert_unread(&conn, "session-1", "agent-1", "msg-2", 2000).unwrap();
        insert_unread(&conn, "session-1", "agent-2", "msg-3", 3000).unwrap();

        delete_unread_by_agent_session(&conn, "agent-1", "session-1").unwrap();

        let unread = get_unread_by_agent(&conn, "agent-1").unwrap();
        assert!(unread.is_empty());

        let unread_agent2 = get_unread_by_agent(&conn, "agent-2").unwrap();
        assert_eq!(unread_agent2.len(), 1);
    }

    #[test]
    fn test_clear_unread_by_session() {
        let conn = init_test_db();
        insert_unread(&conn, "session-1", "agent-1", "msg-1", 1000).unwrap();
        insert_unread(&conn, "session-1", "agent-2", "msg-2", 2000).unwrap();
        insert_unread(&conn, "session-2", "agent-1", "msg-3", 3000).unwrap();

        clear_unread_by_session(&conn, "session-1").unwrap();

        let unread_agent1 = get_unread_by_agent(&conn, "agent-1").unwrap();
        assert_eq!(unread_agent1.len(), 1);
        assert!(unread_agent1.contains_key("session-2"));

        let unread_agent2 = get_unread_by_agent(&conn, "agent-2").unwrap();
        assert!(unread_agent2.is_empty());
    }

    #[test]
    fn test_get_agents_with_unread() {
        let conn = init_test_db();
        insert_unread(&conn, "session-1", "agent-1", "msg-1", 1000).unwrap();
        insert_unread(&conn, "session-1", "agent-2", "msg-2", 2000).unwrap();
        insert_unread(&conn, "session-1", "agent-3", "msg-3", 3000).unwrap();

        let agents = get_agents_with_unread(&conn, "session-1").unwrap();
        assert_eq!(agents.len(), 3);
        assert!(agents.contains(&"agent-1".to_string()));
        assert!(agents.contains(&"agent-2".to_string()));
        assert!(agents.contains(&"agent-3".to_string()));
    }
}
