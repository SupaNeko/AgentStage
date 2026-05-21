use rusqlite::{Connection, Result};
use crate::models::scheduled_task::{ScheduledTask, CreateTimerRequest};
use uuid::Uuid;

fn row_to_task(row: &rusqlite::Row) -> Result<ScheduledTask> {
    Ok(ScheduledTask {
        id: row.get(0)?,
        agent_id: row.get(1)?,
        description: row.get(2)?,
        task_type: row.get(3)?,
        trigger_mode: row.get(4)?,
        after_minutes: row.get(5)?,
        year: row.get(6)?,
        month: row.get(7)?,
        day: row.get(8)?,
        hour: row.get(9)?,
        minute: row.get(10)?,
        interval_minutes: row.get(11)?,
        next_trigger_at: row.get(12)?,
        created_at: row.get(13)?,
        is_active: row.get(14)?,
        target_session_id: row.get(15)?,
    })
}

pub fn insert_task(conn: &Connection, req: &CreateTimerRequest, agent_id: &str) -> Result<String, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let next_trigger_at = req.next_trigger_at.ok_or_else(|| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "next_trigger_at is required",
        )))
    })?;

    conn.execute(
        r#"INSERT INTO scheduled_tasks (
            id, agent_id, description, task_type, trigger_mode,
            after_minutes, year, month, day, hour, minute,
            interval_minutes, next_trigger_at, created_at, is_active, target_session_id
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)"#,
        rusqlite::params![
            &id, agent_id, &req.description, &req.task_type, &req.trigger_mode,
            req.after_minutes, req.year, req.month, req.day, req.hour, req.minute,
            req.interval_minutes, next_trigger_at, now, 1, &req.target_session_id,
        ],
    )?;

    Ok(id)
}

pub fn list_by_agent(conn: &Connection, agent_id: &str) -> Result<Vec<ScheduledTask>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        r#"SELECT id, agent_id, description, task_type, trigger_mode,
            after_minutes, year, month, day, hour, minute,
            interval_minutes, next_trigger_at, created_at, is_active, target_session_id
         FROM scheduled_tasks
         WHERE agent_id = ?1
         ORDER BY next_trigger_at ASC"#,
    )?;

    let rows = stmt.query_map([agent_id], row_to_task)?;
    rows.collect()
}

pub fn get_due_tasks(conn: &Connection, now: i64) -> Result<Vec<ScheduledTask>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        r#"SELECT id, agent_id, description, task_type, trigger_mode,
            after_minutes, year, month, day, hour, minute,
            interval_minutes, next_trigger_at, created_at, is_active, target_session_id
         FROM scheduled_tasks
         WHERE is_active = 1 AND next_trigger_at <= ?1
         ORDER BY next_trigger_at ASC"#,
    )?;

    let rows = stmt.query_map([now], row_to_task)?;
    rows.collect()
}

pub fn deactivate_task(conn: &Connection, task_id: &str) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE scheduled_tasks SET is_active = 0 WHERE id = ?1",
        [task_id],
    )?;
    Ok(())
}

pub fn update_next_trigger(conn: &Connection, task_id: &str, next_trigger_at: i64) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE scheduled_tasks SET next_trigger_at = ?2 WHERE id = ?1",
        (task_id, next_trigger_at),
    )?;
    Ok(())
}

pub fn delete_task(conn: &Connection, task_id: &str) -> Result<(), rusqlite::Error> {
    conn.execute(
        "DELETE FROM scheduled_tasks WHERE id = ?1",
        [task_id],
    )?;
    Ok(())
}

pub fn toggle_task(conn: &Connection, task_id: &str, is_active: i32) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE scheduled_tasks SET is_active = ?2 WHERE id = ?1",
        (task_id, is_active),
    )?;
    Ok(())
}

pub fn update_task(
    conn: &Connection,
    task_id: &str,
    description: Option<&str>,
    next_trigger_at: Option<i64>,
    target_session_id: Option<&str>,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        r#"UPDATE scheduled_tasks SET
            description = COALESCE(?2, description),
            next_trigger_at = COALESCE(?3, next_trigger_at),
            target_session_id = COALESCE(?4, target_session_id)
         WHERE id = ?1"#,
        (task_id, description, next_trigger_at, target_session_id),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn init_test_db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::db::migration::run_migrations(&mut conn).unwrap();
        conn
    }

    fn insert_test_agent(conn: &Connection, agent_id: &str) {
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            (agent_id, "Test Agent", now),
        ).unwrap();
    }

    #[test]
    fn test_insert_and_list() {
        let conn = init_test_db();
        insert_test_agent(&conn, "agent1");

        let req = CreateTimerRequest {
            description: "Recurring reminder".to_string(),
            task_type: "recurring".to_string(),
            trigger_mode: Some("after_minutes".to_string()),
            after_minutes: Some(10),
            year: None,
            month: None,
            day: None,
            hour: None,
            minute: None,
            interval_minutes: Some(30),
            next_trigger_at: Some(1234567890),
            target_session_id: Some("sess1".to_string()),
        };

        let task_id = insert_task(&conn, &req, "agent1").unwrap();
        let tasks = list_by_agent(&conn, "agent1").unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, task_id);
        assert_eq!(tasks[0].agent_id, "agent1");
        assert_eq!(tasks[0].description, "Recurring reminder");
        assert_eq!(tasks[0].task_type, "recurring");
        assert_eq!(tasks[0].interval_minutes, Some(30));
        assert_eq!(tasks[0].next_trigger_at, 1234567890);
        assert_eq!(tasks[0].is_active, 1);
    }

    #[test]
    fn test_get_due_tasks() {
        let conn = init_test_db();
        insert_test_agent(&conn, "agent1");

        let now = chrono::Utc::now().timestamp_millis();
        let req = CreateTimerRequest {
            description: "Single task".to_string(),
            task_type: "single".to_string(),
            trigger_mode: Some("after_minutes".to_string()),
            after_minutes: Some(1),
            year: None,
            month: None,
            day: None,
            hour: None,
            minute: None,
            interval_minutes: None,
            next_trigger_at: Some(now + 60_000), // 1 minute from now
            target_session_id: None,
        };

        insert_task(&conn, &req, "agent1").unwrap();

        // Not due yet
        let not_due = get_due_tasks(&conn, now).unwrap();
        assert_eq!(not_due.len(), 0);

        // Due after 2 minutes
        let due = get_due_tasks(&conn, now + 120_000).unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].description, "Single task");
    }

    #[test]
    fn test_deactivate_and_toggle() {
        let conn = init_test_db();
        insert_test_agent(&conn, "agent1");

        let req = CreateTimerRequest {
            description: "Test task".to_string(),
            task_type: "single".to_string(),
            trigger_mode: None,
            after_minutes: None,
            year: None,
            month: None,
            day: None,
            hour: None,
            minute: None,
            interval_minutes: None,
            next_trigger_at: Some(1000),
            target_session_id: None,
        };

        let task_id = insert_task(&conn, &req, "agent1").unwrap();

        // Deactivate
        deactivate_task(&conn, &task_id).unwrap();
        let tasks = list_by_agent(&conn, "agent1").unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].is_active, 0);

        // Toggle back to active
        toggle_task(&conn, &task_id, 1).unwrap();
        let tasks = list_by_agent(&conn, "agent1").unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].is_active, 1);
    }

    #[test]
    fn test_delete_task() {
        let conn = init_test_db();
        insert_test_agent(&conn, "agent1");

        let req = CreateTimerRequest {
            description: "To be deleted".to_string(),
            task_type: "single".to_string(),
            trigger_mode: None,
            after_minutes: None,
            year: None,
            month: None,
            day: None,
            hour: None,
            minute: None,
            interval_minutes: None,
            next_trigger_at: Some(1000),
            target_session_id: None,
        };

        let task_id = insert_task(&conn, &req, "agent1").unwrap();
        assert_eq!(list_by_agent(&conn, "agent1").unwrap().len(), 1);

        delete_task(&conn, &task_id).unwrap();
        assert_eq!(list_by_agent(&conn, "agent1").unwrap().len(), 0);
    }

    #[test]
    fn test_update_next_trigger() {
        let conn = init_test_db();
        insert_test_agent(&conn, "agent1");

        let req = CreateTimerRequest {
            description: "Update trigger".to_string(),
            task_type: "recurring".to_string(),
            trigger_mode: None,
            after_minutes: None,
            year: None,
            month: None,
            day: None,
            hour: None,
            minute: None,
            interval_minutes: Some(30),
            next_trigger_at: Some(1000),
            target_session_id: None,
        };

        let task_id = insert_task(&conn, &req, "agent1").unwrap();
        update_next_trigger(&conn, &task_id, 2000).unwrap();

        let tasks = list_by_agent(&conn, "agent1").unwrap();
        assert_eq!(tasks[0].next_trigger_at, 2000);
    }

    #[test]
    fn test_update_task_partial() {
        let conn = init_test_db();
        insert_test_agent(&conn, "agent1");

        let req = CreateTimerRequest {
            description: "Original desc".to_string(),
            task_type: "single".to_string(),
            trigger_mode: None,
            after_minutes: None,
            year: None,
            month: None,
            day: None,
            hour: None,
            minute: None,
            interval_minutes: None,
            next_trigger_at: Some(1000),
            target_session_id: Some("sess1".to_string()),
        };

        let task_id = insert_task(&conn, &req, "agent1").unwrap();

        // Update only description
        update_task(&conn, &task_id, Some("Updated desc"), None, None).unwrap();
        let tasks = list_by_agent(&conn, "agent1").unwrap();
        assert_eq!(tasks[0].description, "Updated desc");
        assert_eq!(tasks[0].next_trigger_at, 1000);
        assert_eq!(tasks[0].target_session_id, Some("sess1".to_string()));

        // Update only next_trigger_at
        update_task(&conn, &task_id, None, Some(2000), None).unwrap();
        let tasks = list_by_agent(&conn, "agent1").unwrap();
        assert_eq!(tasks[0].description, "Updated desc");
        assert_eq!(tasks[0].next_trigger_at, 2000);

        // Update only target_session_id
        update_task(&conn, &task_id, None, None, Some("sess2")).unwrap();
        let tasks = list_by_agent(&conn, "agent1").unwrap();
        assert_eq!(tasks[0].target_session_id, Some("sess2".to_string()));
    }

    #[test]
    fn test_insert_task_without_next_trigger_at_fails() {
        let conn = init_test_db();
        insert_test_agent(&conn, "agent1");

        let req = CreateTimerRequest {
            description: "No trigger".to_string(),
            task_type: "single".to_string(),
            trigger_mode: None,
            after_minutes: None,
            year: None,
            month: None,
            day: None,
            hour: None,
            minute: None,
            interval_minutes: None,
            next_trigger_at: None,
            target_session_id: None,
        };

        let result = insert_task(&conn, &req, "agent1");
        assert!(result.is_err());
    }
}
