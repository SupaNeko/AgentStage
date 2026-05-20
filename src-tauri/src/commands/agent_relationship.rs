use tauri::State;
use crate::db::connection::{get_db, DbState};
use crate::db::agent_relationship;
use crate::models::agent_relationship::RelationshipItem;

#[tauri::command]
pub async fn list_agent_relationships(
    state: State<'_, DbState>,
    agent_id: String,
) -> Result<Vec<RelationshipItem>, String> {
    let conn = get_db(&state).await?;
    agent_relationship::list_relationships_by_observer(&conn, &agent_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_agent_relationship(
    state: State<'_, DbState>,
    observer_id: String,
    target_id: String,
    target_type: String,
    relationship_text: String,
) -> Result<(), String> {
    let conn = get_db(&state).await?;
    agent_relationship::upsert_relationship(&conn, &observer_id, &target_id, &target_type, &relationship_text)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_friendships(
    state: State<'_, DbState>,
    observer_id: String,
    target_ids: Vec<String>,
) -> Result<(), String> {
    let conn = get_db(&state).await?;
    for target_id in target_ids {
        agent_relationship::add_friendship(&conn, &observer_id, &target_id)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn remove_friendship(
    state: State<'_, DbState>,
    observer_id: String,
    target_id: String,
) -> Result<(), String> {
    let conn = get_db(&state).await?;
    agent_relationship::remove_friendship(&conn, &observer_id, &target_id)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn update_agent_memory(
    state: State<'_, DbState>,
    observer_id: String,
    target_id: String,
    target_type: String,
    memory_text: String,
) -> Result<(), String> {
    crate::logger::backend("DEBUG", &format!(
        "[DEBUG update_agent_memory] observer_id={}, target_id={}, target_type={}, text_len={}",
        observer_id, target_id, target_type, memory_text.len()
    ));

    if memory_text.chars().count() > 500 {
        return Err(format!("记忆内容超过 500 字限制（当前 {} 字）", memory_text.chars().count()));
    }

    let conn = get_db(&state).await?;
    agent_relationship::upsert_memory(&conn, &observer_id, &target_id, &target_type, &memory_text)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use rusqlite::Connection;
    use crate::db::connection::DbState;
    use crate::db::schema::{MIGRATION_V1, MIGRATION_V2, MIGRATION_V3, MIGRATION_V4, MIGRATION_V5, MIGRATION_V6, MIGRATION_V7, MIGRATION_V8, MIGRATION_V9, MIGRATION_V11, MIGRATION_V12, MIGRATION_V13};

    fn init_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = OFF;", []).unwrap();
        conn.execute_batch(MIGRATION_V1).unwrap();
        conn.execute_batch(MIGRATION_V2).unwrap();
        conn.execute_batch(MIGRATION_V3).unwrap();
        conn.execute_batch(MIGRATION_V4).unwrap();
        conn.execute_batch(MIGRATION_V5).unwrap();
        conn.execute_batch(MIGRATION_V6).unwrap();
        conn.execute_batch(MIGRATION_V7).unwrap();
        conn.execute_batch(MIGRATION_V8).unwrap();
        conn.execute_batch(MIGRATION_V9).unwrap();
        conn.execute_batch(MIGRATION_V11).unwrap();
        conn.execute_batch(MIGRATION_V12).unwrap();
        conn.execute_batch(MIGRATION_V13).unwrap();
        conn
    }

    fn make_db_state(conn: Connection) -> DbState {
        DbState(Arc::new(Mutex::new(conn)))
    }

    fn make_state(db_state: &DbState) -> tauri::State<'_, DbState> {
        unsafe { std::mem::transmute(db_state) }
    }

    fn create_test_agent(conn: &Connection, agent_id: &str, name: &str) {
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            (agent_id, name, 0i64),
        ).unwrap();
    }

    #[tokio::test]
    async fn test_update_agent_memory_enforces_500_char_limit() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent1", "Alice");
        create_test_agent(&conn, "agent2", "Bob");
        let db_state = make_db_state(conn);

        let long_text = "a".repeat(501);
        let result = update_agent_memory(
            make_state(&db_state), "agent1".to_string(), "agent2".to_string(), "agent".to_string(), long_text,
        ).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("500"));
    }

    #[tokio::test]
    async fn test_update_agent_memory_saves_within_limit() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent1", "Alice");
        create_test_agent(&conn, "agent2", "Bob");
        let db_state = make_db_state(conn);

        let result = update_agent_memory(
            make_state(&db_state), "agent1".to_string(), "agent2".to_string(), "agent".to_string(), "他喜欢吃苹果".to_string(),
        ).await;

        assert!(result.is_ok());
    }
}
