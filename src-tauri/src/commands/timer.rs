use chrono::TimeZone;
use tauri::State;
use crate::db::connection::DbState;
use crate::db::scheduled_task as scheduled_task_repo;
use crate::db::settings as settings_repo;
use crate::models::scheduled_task::{ScheduledTask, CreateTimerRequest, UpdateTimerRequest};

#[tauri::command]
pub async fn list_agent_timers(
    db_state: State<'_, DbState>,
    agent_id: String,
) -> Result<Vec<ScheduledTask>, String> {
    let conn = db_state.0.lock().await;
    scheduled_task_repo::list_by_agent(&conn, &agent_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_timer_command(
    db_state: State<'_, DbState>,
    agent_id: String,
    req: CreateTimerRequest,
) -> Result<String, String> {
    let conn = db_state.0.lock().await;
    let now = chrono::Utc::now().timestamp_millis();
    let next_trigger_at = if req.task_type == "single" {
        let trigger_mode = req.trigger_mode.as_deref().ok_or("missing trigger_mode")?;
        if trigger_mode == "after_minutes" {
            let minutes = req.after_minutes.ok_or("missing after_minutes")? as i64;
            if minutes <= 0 { return Err("after_minutes must be > 0".to_string()); }
            now + minutes * 60 * 1000
        } else if trigger_mode == "datetime" {
            let year = req.year.ok_or("missing year")?;
            let month = req.month.ok_or("missing month")? as u32;
            let day = req.day.ok_or("missing day")? as u32;
            let hour = req.hour.ok_or("missing hour")? as u32;
            let minute = req.minute.ok_or("missing minute")? as u32;
            let dt = chrono::Local.with_ymd_and_hms(year, month, day, hour, minute, 0)
                .single().ok_or("invalid datetime")?;
            let ts = dt.timestamp_millis();
            if ts <= now { return Err("datetime must be in the future".to_string()); }
            ts
        } else {
            return Err("invalid trigger_mode".to_string());
        }
    } else if req.task_type == "recurring" {
        let interval = req.interval_minutes.ok_or("missing interval_minutes")? as i64;
        if interval <= 0 { return Err("interval_minutes must be > 0".to_string()); }
        now + interval * 60 * 1000
    } else {
        return Err("invalid task_type".to_string());
    };
    
    let mut req_with_next = req;
    req_with_next.next_trigger_at = Some(next_trigger_at);
    let id = scheduled_task_repo::insert_task(&conn, &req_with_next, &agent_id).map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub async fn update_timer_command(
    db_state: State<'_, DbState>,
    agent_id: String,
    req: UpdateTimerRequest,
) -> Result<(), String> {
    let conn = db_state.0.lock().await;
    let tasks = scheduled_task_repo::list_by_agent(&conn, &agent_id).map_err(|e| e.to_string())?;
    if !tasks.iter().any(|t| t.id == req.id) {
        return Err("任务不存在或不属于该角色".to_string());
    }
    scheduled_task_repo::update_task(&conn, &req.id, req.description.as_deref(), req.next_trigger_at, req.target_session_id.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_timer_command(
    db_state: State<'_, DbState>,
    agent_id: String,
    task_id: String,
) -> Result<(), String> {
    let conn = db_state.0.lock().await;
    let tasks = scheduled_task_repo::list_by_agent(&conn, &agent_id).map_err(|e| e.to_string())?;
    if !tasks.iter().any(|t| t.id == task_id) {
        return Err("任务不存在或不属于该角色".to_string());
    }
    scheduled_task_repo::delete_task(&conn, &task_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_timer(
    db_state: State<'_, DbState>,
    agent_id: String,
    task_id: String,
    is_active: i32,
) -> Result<(), String> {
    let conn = db_state.0.lock().await;
    let tasks = scheduled_task_repo::list_by_agent(&conn, &agent_id).map_err(|e| e.to_string())?;
    if !tasks.iter().any(|t| t.id == task_id) {
        return Err("任务不存在或不属于该角色".to_string());
    }
    scheduled_task_repo::toggle_task(&conn, &task_id, is_active).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_agent_proactive(
    db_state: State<'_, DbState>,
    agent_id: String,
    proactive_enabled: i32,
    proactive_min_minutes: i32,
    proactive_max_minutes: i32,
) -> Result<(), String> {
    crate::logger::backend("DEBUG", &format!(
        "[update_agent_proactive] agent_id={}, enabled={}, min={}, max={}",
        agent_id, proactive_enabled, proactive_min_minutes, proactive_max_minutes
    ));
    let conn = db_state.0.lock().await;
    let rows = conn.execute(
        "UPDATE agents SET proactive_enabled = ?1, proactive_min_minutes = ?2, proactive_max_minutes = ?3, updated_at = ?4 WHERE id = ?5",
        rusqlite::params![proactive_enabled, proactive_min_minutes, proactive_max_minutes, chrono::Utc::now().timestamp_millis(), agent_id],
    ).map_err(|e| e.to_string())?;
    crate::logger::backend("DEBUG", &format!(
        "[update_agent_proactive] rows affected={}", rows
    ));
    Ok(())
}

#[tauri::command]
pub async fn update_quiet_hours(
    db_state: State<'_, DbState>,
    quiet_hours_start: i32,
    quiet_hours_end: i32,
) -> Result<(), String> {
    let conn = db_state.0.lock().await;
    settings_repo::update_quiet_hours(&conn, quiet_hours_start, quiet_hours_end)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_quiet_hours(
    db_state: State<'_, DbState>,
) -> Result<(i32, i32), String> {
    let conn = db_state.0.lock().await;
    let settings = settings_repo::get_or_create_settings(&conn).map_err(|e| e.to_string())?;
    Ok((settings.quiet_hours_start, settings.quiet_hours_end))
}
