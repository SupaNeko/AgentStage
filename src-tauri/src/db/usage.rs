use rusqlite::params;
use crate::db::connection::DbState;
use crate::models::usage::*;

pub async fn insert_usage_record(
    db: &DbState,
    record: &LlmUsageRecord,
) -> Result<(), String> {
    let conn = db.0.lock().await;
    conn.execute(
        "INSERT INTO llm_usage_records (
            id, agent_id, model_config_id, session_id, trigger_type,
            call_round, prompt_tokens, completion_tokens, total_tokens,
            message_id, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            record.id,
            record.agent_id,
            record.model_config_id,
            record.session_id,
            record.trigger_type,
            record.call_round,
            record.prompt_tokens,
            record.completion_tokens,
            record.total_tokens,
            record.message_id,
            record.created_at,
        ],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub async fn get_usage_overview(
    db: &DbState,
    time_range: &TimeRange,
) -> Result<UsageOverview, String> {
    let conn = db.0.lock().await;

    let (total_calls, total_prompt, total_completion, total_tokens): (i64, i64, i64, i64) = conn.query_row(
        "SELECT
            COALESCE(COUNT(*), 0),
            COALESCE(SUM(prompt_tokens), 0),
            COALESCE(SUM(completion_tokens), 0),
            COALESCE(SUM(total_tokens), 0)
         FROM llm_usage_records
         WHERE created_at >= ?1 AND created_at <= ?2",
        params![time_range.start_time, time_range.end_time],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    ).map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT
            date(created_at / 1000, 'unixepoch', 'localtime') as day,
            COUNT(*) as calls,
            SUM(total_tokens) as tokens
         FROM llm_usage_records
         WHERE created_at >= ?1 AND created_at <= ?2
         GROUP BY day
         ORDER BY day"
    ).map_err(|e| e.to_string())?;

    let daily_trend = stmt.query_map(
        params![time_range.start_time, time_range.end_time],
        |row| {
            Ok(DailyTrend {
                date: row.get(0)?,
                calls: row.get(1)?,
                tokens: row.get(2)?,
            })
        }
    ).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;

    Ok(UsageOverview {
        total_calls,
        total_prompt_tokens: total_prompt,
        total_completion_tokens: total_completion,
        total_tokens,
        daily_trend,
    })
}

pub async fn get_usage_by_model(
    db: &DbState,
    time_range: &TimeRange,
) -> Result<Vec<ModelUsageItem>, String> {
    let conn = db.0.lock().await;
    let mut stmt = conn.prepare(
        "SELECT
            mc.id as model_config_id,
            mc.name as model_name,
            mc.provider,
            COUNT(*) as calls,
            SUM(lur.prompt_tokens) as prompt_tokens,
            SUM(lur.completion_tokens) as completion_tokens,
            SUM(lur.total_tokens) as total_tokens
         FROM llm_usage_records lur
         JOIN model_configs mc ON lur.model_config_id = mc.id
         WHERE lur.created_at >= ?1 AND lur.created_at <= ?2
         GROUP BY mc.id, mc.model_name, mc.provider
         ORDER BY total_tokens DESC"
    ).map_err(|e| e.to_string())?;

    let items = stmt.query_map(
        params![time_range.start_time, time_range.end_time],
        |row| {
            Ok(ModelUsageItem {
                model_config_id: row.get(0)?,
                model_name: row.get(1)?,
                provider: row.get(2)?,
                calls: row.get(3)?,
                prompt_tokens: row.get(4)?,
                completion_tokens: row.get(5)?,
                total_tokens: row.get(6)?,
            })
        }
    ).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    Ok(items)
}

pub async fn get_usage_by_agent(
    db: &DbState,
    time_range: &TimeRange,
) -> Result<Vec<AgentUsageItem>, String> {
    let conn = db.0.lock().await;
    let mut stmt = conn.prepare(
        "SELECT
            a.id as agent_id,
            a.name as agent_name,
            a.avatar_path,
            COUNT(*) as calls,
            SUM(lur.prompt_tokens) as prompt_tokens,
            SUM(lur.completion_tokens) as completion_tokens,
            SUM(lur.total_tokens) as total_tokens
         FROM llm_usage_records lur
         JOIN agents a ON lur.agent_id = a.id
         WHERE lur.created_at >= ?1 AND lur.created_at <= ?2
         GROUP BY a.id, a.name, a.avatar_path
         ORDER BY total_tokens DESC"
    ).map_err(|e| e.to_string())?;

    let items = stmt.query_map(
        params![time_range.start_time, time_range.end_time],
        |row| {
            Ok(AgentUsageItem {
                agent_id: row.get(0)?,
                agent_name: row.get(1)?,
                avatar_path: row.get(2)?,
                calls: row.get(3)?,
                prompt_tokens: row.get(4)?,
                completion_tokens: row.get(5)?,
                total_tokens: row.get(6)?,
            })
        }
    ).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    Ok(items)
}

pub async fn get_agent_model_breakdown(
    db: &DbState,
    agent_id: &str,
    time_range: &TimeRange,
) -> Result<Vec<AgentModelUsageItem>, String> {
    let conn = db.0.lock().await;
    let mut stmt = conn.prepare(
        "SELECT
            mc.id as model_config_id,
            mc.model_name,
            COUNT(*) as calls,
            SUM(lur.prompt_tokens) as prompt_tokens,
            SUM(lur.completion_tokens) as completion_tokens,
            SUM(lur.total_tokens) as total_tokens
         FROM llm_usage_records lur
         JOIN model_configs mc ON lur.model_config_id = mc.id
         WHERE lur.agent_id = ?1 AND lur.created_at >= ?2 AND lur.created_at <= ?3
         GROUP BY mc.id, mc.model_name
         ORDER BY total_tokens DESC"
    ).map_err(|e| e.to_string())?;

    let items = stmt.query_map(
        params![agent_id, time_range.start_time, time_range.end_time],
        |row| {
            Ok(AgentModelUsageItem {
                model_config_id: row.get(0)?,
                model_name: row.get(1)?,
                calls: row.get(2)?,
                prompt_tokens: row.get(3)?,
                completion_tokens: row.get(4)?,
                total_tokens: row.get(5)?,
            })
        }
    ).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    Ok(items)
}

pub async fn get_model_agent_breakdown(
    db: &DbState,
    model_config_id: &str,
    time_range: &TimeRange,
) -> Result<Vec<ModelAgentUsageItem>, String> {
    let conn = db.0.lock().await;
    let mut stmt = conn.prepare(
        "SELECT
            a.id as agent_id,
            a.name as agent_name,
            COUNT(*) as calls,
            SUM(lur.prompt_tokens) as prompt_tokens,
            SUM(lur.completion_tokens) as completion_tokens,
            SUM(lur.total_tokens) as total_tokens
         FROM llm_usage_records lur
         JOIN agents a ON lur.agent_id = a.id
         WHERE lur.model_config_id = ?1 AND lur.created_at >= ?2 AND lur.created_at <= ?3
         GROUP BY a.id, a.name
         ORDER BY total_tokens DESC"
    ).map_err(|e| e.to_string())?;

    let items = stmt.query_map(
        params![model_config_id, time_range.start_time, time_range.end_time],
        |row| {
            Ok(ModelAgentUsageItem {
                agent_id: row.get(0)?,
                agent_name: row.get(1)?,
                calls: row.get(2)?,
                prompt_tokens: row.get(3)?,
                completion_tokens: row.get(4)?,
                total_tokens: row.get(5)?,
            })
        }
    ).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    Ok(items)
}

pub async fn get_usage_by_session(
    db: &DbState,
    time_range: &TimeRange,
) -> Result<Vec<SessionUsageItem>, String> {
    let conn = db.0.lock().await;
    let mut stmt = conn.prepare(
        "SELECT
            s.id as session_id,
            COALESCE(gs.name, '私聊') as session_name,
            s.session_type,
            COUNT(*) as calls,
            SUM(lur.prompt_tokens) as prompt_tokens,
            SUM(lur.completion_tokens) as completion_tokens,
            SUM(lur.total_tokens) as total_tokens
         FROM llm_usage_records lur
         JOIN sessions s ON lur.session_id = s.id
         LEFT JOIN group_sessions gs ON s.id = gs.session_id
         WHERE lur.created_at >= ?1 AND lur.created_at <= ?2 AND lur.session_id IS NOT NULL
         GROUP BY s.id, s.session_type
         ORDER BY total_tokens DESC"
    ).map_err(|e| e.to_string())?;

    let items = stmt.query_map(
        params![time_range.start_time, time_range.end_time],
        |row| {
            Ok(SessionUsageItem {
                session_id: row.get(0)?,
                session_name: row.get(1)?,
                session_type: row.get(2)?,
                calls: row.get(3)?,
                prompt_tokens: row.get(4)?,
                completion_tokens: row.get(5)?,
                total_tokens: row.get(6)?,
            })
        }
    ).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    Ok(items)
}

pub async fn get_session_agent_breakdown(
    db: &DbState,
    session_id: &str,
    time_range: &TimeRange,
) -> Result<Vec<SessionAgentUsageItem>, String> {
    let conn = db.0.lock().await;
    let mut stmt = conn.prepare(
        "SELECT
            a.id as agent_id,
            a.name as agent_name,
            COUNT(*) as calls,
            SUM(lur.prompt_tokens) as prompt_tokens,
            SUM(lur.completion_tokens) as completion_tokens,
            SUM(lur.total_tokens) as total_tokens
         FROM llm_usage_records lur
         JOIN agents a ON lur.agent_id = a.id
         WHERE lur.session_id = ?1 AND lur.created_at >= ?2 AND lur.created_at <= ?3
         GROUP BY a.id, a.name
         ORDER BY total_tokens DESC"
    ).map_err(|e| e.to_string())?;

    let items = stmt.query_map(
        params![session_id, time_range.start_time, time_range.end_time],
        |row| {
            Ok(SessionAgentUsageItem {
                agent_id: row.get(0)?,
                agent_name: row.get(1)?,
                calls: row.get(2)?,
                prompt_tokens: row.get(3)?,
                completion_tokens: row.get(4)?,
                total_tokens: row.get(5)?,
            })
        }
    ).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    Ok(items)
}

pub async fn get_session_model_breakdown(
    db: &DbState,
    session_id: &str,
    time_range: &TimeRange,
) -> Result<Vec<SessionModelUsageItem>, String> {
    let conn = db.0.lock().await;
    let mut stmt = conn.prepare(
        "SELECT
            mc.id as model_config_id,
            mc.model_name,
            COUNT(*) as calls,
            SUM(lur.prompt_tokens) as prompt_tokens,
            SUM(lur.completion_tokens) as completion_tokens,
            SUM(lur.total_tokens) as total_tokens
         FROM llm_usage_records lur
         JOIN model_configs mc ON lur.model_config_id = mc.id
         WHERE lur.session_id = ?1 AND lur.created_at >= ?2 AND lur.created_at <= ?3
         GROUP BY mc.id, mc.model_name
         ORDER BY total_tokens DESC"
    ).map_err(|e| e.to_string())?;

    let items = stmt.query_map(
        params![session_id, time_range.start_time, time_range.end_time],
        |row| {
            Ok(SessionModelUsageItem {
                model_config_id: row.get(0)?,
                model_name: row.get(1)?,
                calls: row.get(2)?,
                prompt_tokens: row.get(3)?,
                completion_tokens: row.get(4)?,
                total_tokens: row.get(5)?,
            })
        }
    ).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    Ok(items)
}

pub async fn get_session_agent_model_breakdown(
    db: &DbState,
    session_id: &str,
    time_range: &TimeRange,
) -> Result<Vec<SessionAgentModelUsageItem>, String> {
    let conn = db.0.lock().await;
    let mut stmt = conn.prepare(
        "SELECT
            a.id as agent_id,
            a.name as agent_name,
            mc.id as model_config_id,
            mc.model_name,
            COUNT(*) as calls,
            SUM(lur.prompt_tokens) as prompt_tokens,
            SUM(lur.completion_tokens) as completion_tokens,
            SUM(lur.total_tokens) as total_tokens
         FROM llm_usage_records lur
         JOIN agents a ON lur.agent_id = a.id
         JOIN model_configs mc ON lur.model_config_id = mc.id
         WHERE lur.session_id = ?1 AND lur.created_at >= ?2 AND lur.created_at <= ?3
         GROUP BY a.id, a.name, mc.id, mc.model_name
         ORDER BY total_tokens DESC"
    ).map_err(|e| e.to_string())?;

    let items = stmt.query_map(
        params![session_id, time_range.start_time, time_range.end_time],
        |row| {
            Ok(SessionAgentModelUsageItem {
                agent_id: row.get(0)?,
                agent_name: row.get(1)?,
                model_config_id: row.get(2)?,
                model_name: row.get(3)?,
                calls: row.get(4)?,
                prompt_tokens: row.get(5)?,
                completion_tokens: row.get(6)?,
                total_tokens: row.get(7)?,
            })
        }
    ).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    Ok(items)
}

pub async fn get_usage_by_trigger(
    db: &DbState,
    time_range: &TimeRange,
) -> Result<Vec<TriggerUsageItem>, String> {
    let conn = db.0.lock().await;
    let mut stmt = conn.prepare(
        "SELECT
            trigger_type,
            COUNT(*) as calls,
            SUM(prompt_tokens) as prompt_tokens,
            SUM(completion_tokens) as completion_tokens,
            SUM(total_tokens) as total_tokens
         FROM llm_usage_records
         WHERE created_at >= ?1 AND created_at <= ?2
         GROUP BY trigger_type
         ORDER BY total_tokens DESC"
    ).map_err(|e| e.to_string())?;

    let items = stmt.query_map(
        params![time_range.start_time, time_range.end_time],
        |row| {
            Ok(TriggerUsageItem {
                trigger_type: row.get(0)?,
                calls: row.get(1)?,
                prompt_tokens: row.get(2)?,
                completion_tokens: row.get(3)?,
                total_tokens: row.get(4)?,
            })
        }
    ).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    Ok(items)
}

pub async fn get_usage_records(
    db: &DbState,
    time_range: &TimeRange,
    page: i32,
    page_size: i32,
    filters: &UsageFilters,
) -> Result<PaginatedUsageRecords, String> {
    let conn = db.0.lock().await;
    let offset = (page - 1) * page_size;

    let mut where_clauses = vec!["lur.created_at >= ?1", "lur.created_at <= ?2"];
    let mut query_params: Vec<Box<dyn rusqlite::ToSql>> = vec![
        Box::new(time_range.start_time),
        Box::new(time_range.end_time),
    ];

    if let Some(ref agent_id) = filters.agent_id {
        where_clauses.push("lur.agent_id = ?");
        query_params.push(Box::new(agent_id.clone()));
    }
    if let Some(ref model_id) = filters.model_config_id {
        where_clauses.push("lur.model_config_id = ?");
        query_params.push(Box::new(model_id.clone()));
    }
    if let Some(ref session_id) = filters.session_id {
        where_clauses.push("lur.session_id = ?");
        query_params.push(Box::new(session_id.clone()));
    }
    if let Some(ref trigger) = filters.trigger_type {
        where_clauses.push("lur.trigger_type = ?");
        query_params.push(Box::new(trigger.clone()));
    }

    let where_sql = where_clauses.join(" AND ");

    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM llm_usage_records lur WHERE {}", where_sql),
        rusqlite::params_from_iter(query_params.iter()),
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;

    let sql = format!(
        "SELECT
            lur.id,
            a.name as agent_name,
            mc.model_name,
            COALESCE(gs.name, ps.session_id) as session_name,
            lur.trigger_type,
            lur.call_round,
            lur.prompt_tokens,
            lur.completion_tokens,
            lur.total_tokens,
            lur.created_at
         FROM llm_usage_records lur
         JOIN agents a ON lur.agent_id = a.id
         JOIN model_configs mc ON lur.model_config_id = mc.id
         LEFT JOIN sessions s ON lur.session_id = s.id
         LEFT JOIN group_sessions gs ON s.id = gs.session_id
         LEFT JOIN private_sessions ps ON s.id = ps.session_id
         WHERE {}
         ORDER BY lur.created_at DESC
         LIMIT ? OFFSET ?",
        where_sql
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

    let mut all_params: Vec<Box<dyn rusqlite::ToSql>> = query_params;
    all_params.push(Box::new(page_size));
    all_params.push(Box::new(offset));

    let records = stmt.query_map(
        rusqlite::params_from_iter(all_params.iter()),
        |row| {
            Ok(UsageRecordDetail {
                id: row.get(0)?,
                agent_name: row.get(1)?,
                model_name: row.get(2)?,
                session_name: row.get(3)?,
                trigger_type: row.get(4)?,
                call_round: row.get(5)?,
                prompt_tokens: row.get(6)?,
                completion_tokens: row.get(7)?,
                total_tokens: row.get(8)?,
                created_at: row.get(9)?,
            })
        }
    ).map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;

    Ok(PaginatedUsageRecords {
        records,
        total,
        page,
        page_size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::DbState;
    use rusqlite::Connection;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    fn init_test_db() -> DbState {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::BASE_SCHEMA).unwrap();
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
        // Insert agent
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

        insert_usage_record(&db, &record).await.unwrap();

        let range = TimeRange { start_time: 0, end_time: 3000 };
        let overview = get_usage_overview(&db, &range).await.unwrap();

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
        insert_usage_record(&db, &record).await.unwrap();

        let range = TimeRange { start_time: 0, end_time: 3000 };
        let items = get_usage_by_model(&db, &range).await.unwrap();

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
        insert_usage_record(&db, &record).await.unwrap();

        let range = TimeRange { start_time: 0, end_time: 3000 };
        let items = get_usage_by_agent(&db, &range).await.unwrap();

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
        insert_usage_record(&db, &record).await.unwrap();

        let range = TimeRange { start_time: 0, end_time: 3000 };
        let items = get_usage_by_trigger(&db, &range).await.unwrap();

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
        insert_usage_record(&db, &record).await.unwrap();

        let range = TimeRange { start_time: 0, end_time: 3000 };
        let filters = UsageFilters {
            agent_id: Some("agent-1".to_string()),
            model_config_id: None,
            session_id: None,
            trigger_type: None,
        };
        let result = get_usage_records(&db, &range, 1, 50, &filters).await.unwrap();

        assert_eq!(result.total, 1);
        assert_eq!(result.records.len(), 1);
        assert_eq!(result.records[0].agent_name, "Test Agent");
    }

}
