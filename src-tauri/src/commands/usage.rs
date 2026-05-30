use tauri::State;
use crate::db::connection::DbState;
use crate::db::usage as usage_repo;
use crate::models::usage::*;

fn parse_time_range(range: &str) -> TimeRange {
    let now = chrono::Utc::now().timestamp_millis();
    let start = match range {
        "today" => {
            let today = chrono::Utc::now();
            let midnight = today.date_naive().and_hms_opt(0, 0, 0).unwrap();
            chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(midnight, chrono::Utc)
                .timestamp_millis()
        }
        "last_7_days" => now - 7 * 24 * 60 * 60 * 1000,
        "last_30_days" => now - 30 * 24 * 60 * 60 * 1000,
        "this_month" => {
            use chrono::Datelike;
            let today = chrono::Utc::now();
            let first_day = today.date_naive().with_day(1).unwrap().and_hms_opt(0, 0, 0).unwrap();
            chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(first_day, chrono::Utc)
                .timestamp_millis()
        }
        "all" => 0,
        _ => 0,
    };
    TimeRange {
        start_time: start,
        end_time: now,
    }
}

#[tauri::command]
pub async fn get_usage_overview(
    db: State<'_, DbState>,
    time_range: String,
) -> Result<UsageOverview, String> {
    let range = parse_time_range(&time_range);
    usage_repo::get_usage_overview(&db, &range).await
}

#[tauri::command]
pub async fn get_usage_by_model(
    db: State<'_, DbState>,
    time_range: String,
) -> Result<Vec<ModelUsageItem>, String> {
    let range = parse_time_range(&time_range);
    usage_repo::get_usage_by_model(&db, &range).await
}

#[tauri::command]
pub async fn get_usage_by_agent(
    db: State<'_, DbState>,
    time_range: String,
) -> Result<Vec<AgentUsageItem>, String> {
    let range = parse_time_range(&time_range);
    usage_repo::get_usage_by_agent(&db, &range).await
}

#[tauri::command]
pub async fn get_agent_model_breakdown(
    db: State<'_, DbState>,
    agent_id: String,
    time_range: String,
) -> Result<Vec<AgentModelUsageItem>, String> {
    let range = parse_time_range(&time_range);
    usage_repo::get_agent_model_breakdown(&db, &agent_id, &range).await
}

#[tauri::command]
pub async fn get_model_agent_breakdown(
    db: State<'_, DbState>,
    model_config_id: String,
    time_range: String,
) -> Result<Vec<ModelAgentUsageItem>, String> {
    let range = parse_time_range(&time_range);
    usage_repo::get_model_agent_breakdown(&db, &model_config_id, &range).await
}

#[tauri::command]
pub async fn get_usage_by_session(
    db: State<'_, DbState>,
    time_range: String,
) -> Result<Vec<SessionUsageItem>, String> {
    let range = parse_time_range(&time_range);
    usage_repo::get_usage_by_session(&db, &range).await
}

#[tauri::command]
pub async fn get_session_agent_breakdown(
    db: State<'_, DbState>,
    session_id: String,
    time_range: String,
) -> Result<Vec<SessionAgentUsageItem>, String> {
    let range = parse_time_range(&time_range);
    usage_repo::get_session_agent_breakdown(&db, &session_id, &range).await
}

#[tauri::command]
pub async fn get_session_model_breakdown(
    db: State<'_, DbState>,
    session_id: String,
    time_range: String,
) -> Result<Vec<SessionModelUsageItem>, String> {
    let range = parse_time_range(&time_range);
    usage_repo::get_session_model_breakdown(&db, &session_id, &range).await
}

#[tauri::command]
pub async fn get_session_agent_model_breakdown(
    db: State<'_, DbState>,
    session_id: String,
    time_range: String,
) -> Result<Vec<SessionAgentModelUsageItem>, String> {
    let range = parse_time_range(&time_range);
    usage_repo::get_session_agent_model_breakdown(&db, &session_id, &range).await
}

#[tauri::command]
pub async fn get_usage_by_trigger(
    db: State<'_, DbState>,
    time_range: String,
) -> Result<Vec<TriggerUsageItem>, String> {
    let range = parse_time_range(&time_range);
    usage_repo::get_usage_by_trigger(&db, &range).await
}

#[tauri::command]
pub async fn get_usage_records(
    db: State<'_, DbState>,
    time_range: String,
    page: i32,
    page_size: i32,
    filters: Option<UsageFilters>,
) -> Result<PaginatedUsageRecords, String> {
    let range = parse_time_range(&time_range);
    let filters = filters.unwrap_or(UsageFilters {
        agent_id: None,
        model_config_id: None,
        session_id: None,
        trigger_type: None,
    });
    usage_repo::get_usage_records(&db, &range, page, page_size, &filters).await
}
