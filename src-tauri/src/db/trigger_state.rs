use rusqlite::{Connection, Result};

pub fn get_last_trigger_time(conn: &Connection, agent_id: &str) -> Result<i64> {
    let result: Result<i64> = conn.query_row(
        "SELECT last_trigger_time FROM trigger_states WHERE agent_id = ?1",
        [agent_id],
        |row| row.get(0),
    );
    match result {
        Ok(time) => Ok(time),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(0),
        Err(e) => Err(e),
    }
}

pub fn update_trigger_time(conn: &Connection, agent_id: &str) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO trigger_states (agent_id, last_trigger_time, is_triggering, updated_at) \
         VALUES (?1, ?2, 0, ?2) \
         ON CONFLICT(agent_id) DO UPDATE SET \
         last_trigger_time = excluded.last_trigger_time, \
         is_triggering = 0, \
         updated_at = excluded.updated_at",
        (agent_id, now),
    )?;
    Ok(())
}

pub fn init_trigger_state(conn: &Connection, agent_id: &str) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT OR IGNORE INTO trigger_states (agent_id, last_trigger_time, is_triggering, updated_at) \
         VALUES (?1, 0, 0, ?2)",
        (agent_id, now),
    )?;
    Ok(())
}
