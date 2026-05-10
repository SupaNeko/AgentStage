use rusqlite::{Connection, Result, Row};
use crate::models::agent::{Agent, CreateAgentRequest, UpdateAgentRequest};
use uuid::Uuid;

fn row_to_agent(row: &Row) -> Result<Agent> {
    Ok(Agent {
        id: row.get(0)?,
        name: row.get(1)?,
        avatar_path: row.get(2)?,
        detailed_persona: row.get(3)?,
        simplified_persona: row.get(4)?,
        personality: row.get(5)?,
        scenario: row.get(6)?,
        example_messages: row.get(7)?,
        first_message: row.get(8)?,
        creator_notes: row.get(9)?,
        tags: row.get(10)?,
        model_provider: row.get(11)?,
        model_name: row.get(12)?,
        base_url: row.get(13)?,
        temperature: row.get(14)?,
        max_tokens: row.get(15)?,
        top_p: row.get(16)?,
        presence_penalty: row.get(17)?,
        frequency_penalty: row.get(18)?,
        api_key_encrypted: row.get(19)?,
        is_deleted: row.get::<_, i32>(20)? != 0,
        deleted_at: row.get(21)?,
        created_at: row.get(22)?,
        updated_at: row.get(23)?,
    })
}

pub fn create(conn: &Connection, req: &CreateAgentRequest) -> Result<Agent> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let api_key_bytes = req.api_key.as_bytes().to_vec(); // TODO: encrypt with aes-gcm
    
    conn.execute(
        r#"INSERT INTO agents (
            id, name, avatar_path, detailed_persona, simplified_persona,
            personality, scenario, model_provider, model_name, base_url,
            temperature, max_tokens, api_key_encrypted, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"#,
        (
            &id, &req.name, &req.avatar_path, &req.detailed_persona, &req.simplified_persona,
            &req.personality, &req.scenario, &req.model_provider, &req.model_name, &req.base_url,
            req.temperature.unwrap_or(0.7), req.max_tokens.unwrap_or(2048),
            &api_key_bytes, now, now,
        ),
    )?;
    
    get_by_id(conn, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<Option<Agent>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM agents WHERE id = ?1 AND is_deleted = 0"
    )?;
    let mut rows = stmt.query_map([id], row_to_agent)?;
    rows.next().transpose()
}

pub fn list_all(conn: &Connection) -> Result<Vec<Agent>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM agents WHERE is_deleted = 0 ORDER BY created_at DESC"
    )?;
    let rows = stmt.query_map([], row_to_agent)?;
    rows.collect()
}

pub fn update(conn: &Connection, req: &UpdateAgentRequest) -> Result<Agent> {
    let now = chrono::Utc::now().timestamp_millis();
    
    conn.execute(
        r#"UPDATE agents SET
            name = COALESCE(?2, name),
            avatar_path = COALESCE(?3, avatar_path),
            detailed_persona = COALESCE(?4, detailed_persona),
            simplified_persona = COALESCE(?5, simplified_persona),
            personality = COALESCE(?6, personality),
            scenario = COALESCE(?7, scenario),
            model_provider = COALESCE(?8, model_provider),
            model_name = COALESCE(?9, model_name),
            base_url = COALESCE(?10, base_url),
            temperature = COALESCE(?11, temperature),
            max_tokens = COALESCE(?12, max_tokens),
            api_key_encrypted = COALESCE(?13, api_key_encrypted),
            updated_at = ?14
        WHERE id = ?1 AND is_deleted = 0"#,
        (
            &req.id, &req.name, &req.avatar_path, &req.detailed_persona, &req.simplified_persona,
            &req.personality, &req.scenario, &req.model_provider, &req.model_name, &req.base_url,
            req.temperature, req.max_tokens,
            req.api_key.as_ref().map(|k| k.as_bytes().to_vec()),
            now,
        ),
    )?;
    
    get_by_id(conn, &req.id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn soft_delete(conn: &Connection, id: &str) -> Result<bool> {
    let now = chrono::Utc::now().timestamp_millis();
    let rows = conn.execute(
        "UPDATE agents SET is_deleted = 1, deleted_at = ?2 WHERE id = ?1 AND is_deleted = 0",
        (id, now),
    )?;
    Ok(rows > 0)
}
