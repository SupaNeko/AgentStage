use rusqlite::{Connection, Result};

pub fn get_frozen_sessions(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT session_id FROM session_frozen_states WHERE is_frozen = 1"
    )?;
    let rows = stmt.query_map([], |row| {
        row.get::<_, String>(0)
    })?;
    rows.collect()
}

pub fn set_frozen(conn: &Connection, session_id: &str) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO session_frozen_states (session_id, is_frozen, frozen_at, updated_at) \
         VALUES (?1, 1, ?2, ?2) \
         ON CONFLICT(session_id) DO UPDATE SET \
         is_frozen = 1, frozen_at = excluded.frozen_at, updated_at = excluded.updated_at",
        (session_id, now),
    )?;
    Ok(())
}

pub fn remove_frozen(conn: &Connection, session_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM session_frozen_states WHERE session_id = ?1",
        [session_id],
    )?;
    Ok(())
}

pub fn is_frozen(conn: &Connection, session_id: &str) -> Result<bool> {
    let result: Result<i32> = conn.query_row(
        "SELECT is_frozen FROM session_frozen_states WHERE session_id = ?1",
        [session_id],
        |row| row.get(0),
    );
    match result {
        Ok(flag) => Ok(flag != 0),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::MIGRATION_V6;

    fn init_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = OFF;", []).unwrap();
        conn.execute_batch(MIGRATION_V6).unwrap();
        conn
    }

    #[test]
    fn test_set_and_get_frozen() {
        let conn = init_test_db();
        set_frozen(&conn, "session-1").unwrap();
        assert!(is_frozen(&conn, "session-1").unwrap());
    }

    #[test]
    fn test_remove_frozen() {
        let conn = init_test_db();
        set_frozen(&conn, "session-1").unwrap();
        assert!(is_frozen(&conn, "session-1").unwrap());
        remove_frozen(&conn, "session-1").unwrap();
        let frozen = get_frozen_sessions(&conn).unwrap();
        assert!(frozen.is_empty());
    }

    #[test]
    fn test_get_frozen_sessions_multiple() {
        let conn = init_test_db();
        set_frozen(&conn, "session-a").unwrap();
        set_frozen(&conn, "session-b").unwrap();
        set_frozen(&conn, "session-c").unwrap();
        let frozen = get_frozen_sessions(&conn).unwrap();
        assert_eq!(frozen.len(), 3);
        assert!(frozen.contains(&"session-a".to_string()));
        assert!(frozen.contains(&"session-b".to_string()));
        assert!(frozen.contains(&"session-c".to_string()));
    }
}
