use agentstage_lib::db::connection::DbState;
use agentstage_lib::db::usage as usage_repo;
use agentstage_lib::models::usage::{LlmUsageRecord, TimeRange, UsageFilters};
use rusqlite::Connection;
use std::sync::Arc;
use tokio::sync::Mutex;

fn init_test_db() -> DbState {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(agentstage_lib::db::schema::BASE_SCHEMA).unwrap();
    DbState(Arc::new(Mutex::new(conn)))
}

async fn setup_test_data(db: &DbState) {
    let conn = db.0.lock().await;
    // Insert model config first (referenced by agents)
    conn.execute(
        "INSERT INTO model_configs (id, name, provider, model_name, base_url, api_key_encrypted, created_at, updated_at)
         VALUES ('model-1', 'Test Model', 'openai', 'gpt-4', 'https://api.openai.com', 'key', 1000, 1000)",
        [],
    ).unwrap();
    // Insert agent (references model_configs)
    conn.execute(
        "INSERT INTO agents (id, name, detailed_persona, simplified_persona, model_config_id, created_at, updated_at)
         VALUES ('agent-1', 'Test Agent', 'detailed', 'simple', 'model-1', 1000, 1000)",
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
}

#[tokio::test]
async fn test_insert_and_overview() {
    let db = init_test_db();
    setup_test_data(&db).await;

    let record = LlmUsageRecord {
        id: "usage-1".to_string(),
        agent_id: "agent-1".to_string(),
        model_config_id: "model-1".to_string(),
        session_id: Some("session-1".to_string()),
        trigger_type: "user_message".to_string(),
        call_round: 1,
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: 150,
        message_id: None,
        created_at: 2000,
    };

    usage_repo::insert_usage_record(&db, &record).await.unwrap();

    let range = TimeRange { start_time: 0, end_time: 3000 };
    let overview = usage_repo::get_usage_overview(&db, &range).await.unwrap();

    assert_eq!(overview.total_calls, 1);
    assert_eq!(overview.total_prompt_tokens, 100);
    assert_eq!(overview.total_completion_tokens, 50);
    assert_eq!(overview.total_tokens, 150);
}

#[tokio::test]
async fn test_get_usage_by_model() {
    let db = init_test_db();
    setup_test_data(&db).await;

    let record = LlmUsageRecord {
        id: "usage-1".to_string(),
        agent_id: "agent-1".to_string(),
        model_config_id: "model-1".to_string(),
        session_id: Some("session-1".to_string()),
        trigger_type: "user_message".to_string(),
        call_round: 1,
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: 150,
        message_id: None,
        created_at: 2000,
    };
    usage_repo::insert_usage_record(&db, &record).await.unwrap();

    let range = TimeRange { start_time: 0, end_time: 3000 };
    let items = usage_repo::get_usage_by_model(&db, &range).await.unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].model_name, "Test Model");
    assert_eq!(items[0].calls, 1);
    assert_eq!(items[0].total_tokens, 150);
}

#[tokio::test]
async fn test_get_usage_by_agent() {
    let db = init_test_db();
    setup_test_data(&db).await;

    let record = LlmUsageRecord {
        id: "usage-1".to_string(),
        agent_id: "agent-1".to_string(),
        model_config_id: "model-1".to_string(),
        session_id: Some("session-1".to_string()),
        trigger_type: "user_message".to_string(),
        call_round: 1,
        prompt_tokens: 200,
        completion_tokens: 100,
        total_tokens: 300,
        message_id: None,
        created_at: 2000,
    };
    usage_repo::insert_usage_record(&db, &record).await.unwrap();

    let range = TimeRange { start_time: 0, end_time: 3000 };
    let items = usage_repo::get_usage_by_agent(&db, &range).await.unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].agent_name, "Test Agent");
    assert_eq!(items[0].total_tokens, 300);
}

#[tokio::test]
async fn test_get_usage_by_trigger() {
    let db = init_test_db();
    setup_test_data(&db).await;

    let record = LlmUsageRecord {
        id: "usage-1".to_string(),
        agent_id: "agent-1".to_string(),
        model_config_id: "model-1".to_string(),
        session_id: Some("session-1".to_string()),
        trigger_type: "user_message".to_string(),
        call_round: 1,
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: 150,
        message_id: None,
        created_at: 2000,
    };
    usage_repo::insert_usage_record(&db, &record).await.unwrap();

    let range = TimeRange { start_time: 0, end_time: 3000 };
    let items = usage_repo::get_usage_by_trigger(&db, &range).await.unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].trigger_type, "user_message");
    assert_eq!(items[0].calls, 1);
}

#[tokio::test]
async fn test_get_usage_records_with_filters() {
    let db = init_test_db();
    setup_test_data(&db).await;

    let record = LlmUsageRecord {
        id: "usage-1".to_string(),
        agent_id: "agent-1".to_string(),
        model_config_id: "model-1".to_string(),
        session_id: Some("session-1".to_string()),
        trigger_type: "user_message".to_string(),
        call_round: 1,
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: 150,
        message_id: None,
        created_at: 2000,
    };
    usage_repo::insert_usage_record(&db, &record).await.unwrap();

    let range = TimeRange { start_time: 0, end_time: 3000 };
    let filters = UsageFilters {
        agent_id: Some("agent-1".to_string()),
        model_config_id: None,
        session_id: None,
        trigger_type: None,
    };
    let result = usage_repo::get_usage_records(&db, &range, 1, 50, &filters).await.unwrap();

    assert_eq!(result.total, 1);
    assert_eq!(result.records.len(), 1);
    assert_eq!(result.records[0].agent_name, "Test Agent");
}

#[tokio::test]
async fn test_time_range_filter() {
    let db = init_test_db();
    setup_test_data(&db).await;

    let record = LlmUsageRecord {
        id: "usage-1".to_string(),
        agent_id: "agent-1".to_string(),
        model_config_id: "model-1".to_string(),
        session_id: Some("session-1".to_string()),
        trigger_type: "user_message".to_string(),
        call_round: 1,
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: 150,
        message_id: None,
        created_at: 2000,
    };
    usage_repo::insert_usage_record(&db, &record).await.unwrap();

    let range = TimeRange { start_time: 0, end_time: 3000 };
    let overview = usage_repo::get_usage_overview(&db, &range).await.unwrap();
    assert_eq!(overview.total_calls, 1);

    let range = TimeRange { start_time: 3000, end_time: 5000 };
    let overview = usage_repo::get_usage_overview(&db, &range).await.unwrap();
    assert_eq!(overview.total_calls, 0);
}
