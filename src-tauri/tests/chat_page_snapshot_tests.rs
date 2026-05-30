use agentstage_lib::db::connection::DbState;
use agentstage_lib::db::session as session_repo;
use agentstage_lib::db::chat_page_participant;
use agentstage_lib::commands::message::resolve_history_target_agents;
use rusqlite::Connection;
use std::sync::Arc;
use tokio::sync::Mutex;

fn init_test_db() -> DbState {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(agentstage_lib::db::schema::BASE_SCHEMA).unwrap();
    DbState(Arc::new(Mutex::new(conn)))
}

async fn setup_test_session(db: &DbState) -> String {
    let conn = db.0.lock().await;
    // Insert model config (must precede agent due to FK)
    conn.execute(
        "INSERT INTO model_configs (id, name, provider, model_name, base_url, api_key_encrypted, created_at, updated_at)
         VALUES ('model-1', 'Test Model', 'openai', 'gpt-4', 'https://api.openai.com', 'key', 1000, 1000)",
        [],
    ).unwrap();
    // Insert agent
    conn.execute(
        "INSERT INTO agents (id, name, detailed_persona, simplified_persona, model_config_id, created_at, updated_at)
         VALUES ('agent-1', 'Test Agent', 'detailed', 'A helpful test agent', 'model-1', 1000, 1000)",
        [],
    ).unwrap();
    // Insert session
    conn.execute(
        "INSERT INTO sessions (id, session_type, created_at, updated_at, last_message_at, is_deleted)
         VALUES ('session-1', 'private', 1000, 1000, 1000, 0)",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO private_sessions (session_id, participant_1_type, participant_1_id, participant_2_type, participant_2_id, agent_message_count, created_at)
         VALUES ('session-1', 'user', 'user-1', 'agent', 'agent-1', 0, 1000)",
        [],
    ).unwrap();
    // Insert chat_page
    conn.execute(
        "INSERT INTO chat_pages (id, session_id, page_index, name, is_active, message_count, created_at, updated_at)
         VALUES ('cp-0', 'session-1', 0, 'Page 0', 1, 0, 1000, 1000)",
        [],
    ).unwrap();
    "session-1".to_string()
}

#[tokio::test]
async fn test_reset_session_creates_snapshot() {
    let db = init_test_db();
    let session_id = setup_test_session(&db).await;

    let conn = db.0.lock().await;
    let (_page_id, _new_index) = session_repo::reset_session(&conn, &session_id).unwrap();
    drop(conn);

    // Check snapshot was created for old page (cp-0)
    let conn = db.0.lock().await;
    let participants = chat_page_participant::list_by_chat_page(&conn, "cp-0").unwrap();
    assert_eq!(participants.len(), 2, "Snapshot should contain 2 participants");

    let agent = participants.iter().find(|p| p.participant_type == "agent").unwrap();
    assert_eq!(agent.participant_id, "agent-1");
    assert_eq!(agent.participant_name, "Test Agent");
    assert_eq!(agent.participant_simplified_persona.as_deref(), Some("A helpful test agent"));

    let user = participants.iter().find(|p| p.participant_type == "user").unwrap();
    assert_eq!(user.participant_id, "user-1");
    assert_eq!(user.participant_simplified_persona, None);
}

#[tokio::test]
async fn test_deleted_agent_not_in_history_targets() {
    let db = init_test_db();
    let session_id = setup_test_session(&db).await;

    let conn = db.0.lock().await;
    session_repo::reset_session(&conn, &session_id).unwrap();
    // Snapshot for cp-0 should now contain agent-1
    let participants = chat_page_participant::list_by_chat_page(&conn, "cp-0").unwrap();
    assert_eq!(participants.len(), 2);
    
    // Soft delete agent
    conn.execute("UPDATE agents SET is_deleted = 1 WHERE id = 'agent-1'", []).unwrap();
    drop(conn);

    // resolve_history_target_agents should return empty because agent is deleted
    let conn = db.0.lock().await;
    let agents = resolve_history_target_agents(&conn, &session_id, 0).unwrap();
    assert!(agents.is_empty(), "Deleted agent should be excluded from history targets even when in snapshot");
}

#[tokio::test]
async fn test_resolve_history_fallback_for_pre_migration_page() {
    let db = init_test_db();
    let session_id = setup_test_session(&db).await;
    
    // Delete the chat_page so get_chat_page_id returns None, simulating a pre-migration state
    let conn = db.0.lock().await;
    conn.execute("DELETE FROM chat_pages WHERE id = 'cp-0'", []).unwrap();
    drop(conn);
    
    // Without a chat_page, resolve_history_target_agents should fallback to current session members
    let conn = db.0.lock().await;
    let agents = resolve_history_target_agents(&conn, &session_id, 0).unwrap();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0], "agent-1");
}
