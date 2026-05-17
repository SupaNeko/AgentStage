use rusqlite::Connection;
use crate::models::user_persona::{UserPersona, CreateUserPersonaRequest, UpdateUserPersonaRequest, CurrentUserPersonaResponse};
use crate::constants::{DEFAULT_USER_NAME, DEFAULT_USER_PERSONA};

fn row_to_persona(row: &rusqlite::Row) -> Result<UserPersona, rusqlite::Error> {
    Ok(UserPersona {
        id: row.get("id")?,
        name: row.get("name")?,
        description: row.get("description")?,
        avatar_path: row.get("avatar_path")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn list_user_personas(conn: &Connection) -> Result<Vec<UserPersona>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, avatar_path, created_at, updated_at FROM user_personas ORDER BY updated_at DESC"
    )?;
    let rows = stmt.query_map([], row_to_persona)?;
    rows.collect()
}

pub fn create_user_persona(conn: &Connection, req: &CreateUserPersonaRequest) -> Result<UserPersona, rusqlite::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO user_personas (id, name, description, avatar_path, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        (&id, &req.name, &req.description, &req.avatar_path, &now),
    )?;
    Ok(UserPersona {
        id, name: req.name.clone(), description: req.description.clone(),
        avatar_path: req.avatar_path.clone(), created_at: now, updated_at: now,
    })
}

pub fn update_user_persona(conn: &Connection, req: &UpdateUserPersonaRequest) -> Result<UserPersona, rusqlite::Error> {
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE user_personas SET name = COALESCE(?2, name), description = COALESCE(?3, description), avatar_path = COALESCE(?4, avatar_path), updated_at = ?5 WHERE id = ?1",
        (&req.id, &req.name, &req.description, &req.avatar_path, &now),
    )?;
    get_user_persona_by_id(conn, &req.id)
}

pub fn get_user_persona_by_id(conn: &Connection, id: &str) -> Result<UserPersona, rusqlite::Error> {
    conn.query_row(
        "SELECT id, name, description, avatar_path, created_at, updated_at FROM user_personas WHERE id = ?1",
        [id], row_to_persona,
    )
}

pub fn delete_user_persona(conn: &Connection, id: &str) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM user_personas WHERE id = ?1", [id])?;
    conn.execute(
        "UPDATE app_settings SET active_persona_id = NULL WHERE id = 1 AND active_persona_id = ?1",
        [id],
    )?;
    Ok(())
}

pub fn get_current_user_persona(conn: &Connection) -> Result<CurrentUserPersonaResponse, rusqlite::Error> {
    let active_id: Option<String> = conn.query_row(
        "SELECT active_persona_id FROM app_settings WHERE id = 1", [], |row| row.get(0),
    ).ok();

    if let Some(id) = active_id {
        if let Ok(persona) = get_user_persona_by_id(conn, &id) {
            return Ok(CurrentUserPersonaResponse {
                id: Some(persona.id), name: persona.name,
                description: persona.description.unwrap_or_else(|| DEFAULT_USER_PERSONA.to_string()),
                avatar_path: persona.avatar_path, is_custom: true,
            });
        }
    }

    let default_avatar: Option<String> = conn.query_row(
        "SELECT default_avatar_path FROM app_settings WHERE id = 1", [], |row| row.get(0),
    ).ok().flatten();

    Ok(CurrentUserPersonaResponse {
        id: None, name: DEFAULT_USER_NAME.to_string(),
        description: DEFAULT_USER_PERSONA.to_string(),
        avatar_path: default_avatar, is_custom: false,
    })
}

pub fn activate_user_persona(conn: &Connection, id: Option<&str>) -> Result<(), rusqlite::Error> {
    conn.execute("UPDATE app_settings SET active_persona_id = ?1 WHERE id = 1", [id])?;
    Ok(())
}

pub fn update_default_avatar(conn: &Connection, path: &str) -> Result<(), rusqlite::Error> {
    conn.execute("UPDATE app_settings SET default_avatar_path = ?1 WHERE id = 1", [path])?;
    Ok(())
}
