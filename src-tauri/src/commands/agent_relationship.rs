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
