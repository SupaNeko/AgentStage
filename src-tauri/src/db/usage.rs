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
