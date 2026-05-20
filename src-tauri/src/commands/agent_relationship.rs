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
