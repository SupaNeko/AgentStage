use rusqlite::{Connection, Result, Row};
use crate::models::agent::{Agent, CreateAgentRequest, UpdateAgentRequest};
use uuid::Uuid;

const SELECT_COLUMNS: &str = "id, name, avatar_path, detailed_persona, simplified_persona, personality, scenario, example_messages, first_message, creator_notes, tags, model_provider, model_name, base_url, temperature, max_tokens, top_p, presence_penalty, frequency_penalty, long_term_memory, memory_enabled, api_key_encrypted, thinking_mode, proactive_enabled, proactive_min_minutes, proactive_max_minutes, is_deleted, deleted_at, created_at, updated_at";

fn row_to_agent(row: &Row) -> Result<Agent> {
    Ok(Agent {
        id: row.get(0)?,
        name: row.get(1)?,
        avatar_path: crate::db::resolve_avatar_path(row.get(2)?),
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
        long_term_memory: row.get(19)?,
        memory_enabled: row.get::<_, i32>(20)? != 0,
        api_key_encrypted: row.get(21)?,
        thinking_mode: row.get::<_, i32>(22)? != 0,
        proactive_enabled: row.get::<_, i32>(23)? != 0,
        proactive_min_minutes: row.get(24)?,
        proactive_max_minutes: row.get(25)?,
        is_deleted: row.get::<_, i32>(26)? != 0,
        deleted_at: row.get(27)?,
        created_at: row.get(28)?,
        updated_at: row.get(29)?,
    })
}

pub fn create(conn: &Connection, req: &CreateAgentRequest) -> Result<Agent> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let api_key_encrypted = crate::crypto::encrypt(&req.api_key)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e))))?;
    
    conn.execute(
        r#"INSERT INTO agents (
            id, name, avatar_path, detailed_persona, simplified_persona,
            personality, scenario, example_messages, first_message, creator_notes, tags,
            model_provider, model_name, base_url,
            temperature, max_tokens, long_term_memory, memory_enabled, api_key_encrypted, thinking_mode, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)"#,
        rusqlite::params![
            &id, &req.name, &req.avatar_path, &req.detailed_persona, &req.simplified_persona,
            &req.personality, &req.scenario, &req.example_messages, &req.first_message, &req.creator_notes,
            &req.tags, &req.model_provider, &req.model_name, &req.base_url,
            req.temperature.unwrap_or(0.7), req.max_tokens.unwrap_or(2048),
            &req.long_term_memory, req.memory_enabled.unwrap_or(true) as i32,
            &api_key_encrypted, req.thinking_mode.unwrap_or(false) as i32, now, now,
        ],
    )?;
    
    get_by_id(conn, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<Option<Agent>> {
    let mut stmt = conn.prepare(
        &format!("SELECT {} FROM agents WHERE id = ?1 AND is_deleted = 0", SELECT_COLUMNS)
    )?;
    let mut rows = stmt.query_map([id], row_to_agent)?;
    rows.next().transpose()
}

pub fn list_all(conn: &Connection) -> Result<Vec<Agent>> {
    let mut stmt = conn.prepare(
        &format!("SELECT {} FROM agents WHERE is_deleted = 0 ORDER BY created_at DESC", SELECT_COLUMNS)
    )?;
    let rows = stmt.query_map([], row_to_agent)?;
    rows.collect()
}

pub fn update(conn: &Connection, req: &UpdateAgentRequest) -> Result<Agent> {
    let now = chrono::Utc::now().timestamp_millis();
    let api_key_encrypted = req.api_key.as_ref()
        .map(|k| crate::crypto::encrypt(k))
        .transpose()
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e))))?;
    
    conn.execute(
        r#"UPDATE agents SET
            name = COALESCE(?2, name),
            avatar_path = COALESCE(?3, avatar_path),
            detailed_persona = COALESCE(?4, detailed_persona),
            simplified_persona = COALESCE(?5, simplified_persona),
            personality = COALESCE(?6, personality),
            scenario = COALESCE(?7, scenario),
            example_messages = COALESCE(?8, example_messages),
            first_message = COALESCE(?9, first_message),
            creator_notes = COALESCE(?10, creator_notes),
            tags = COALESCE(?11, tags),
            model_provider = COALESCE(?12, model_provider),
            model_name = COALESCE(?13, model_name),
            base_url = COALESCE(?14, base_url),
            temperature = COALESCE(?15, temperature),
            max_tokens = COALESCE(?16, max_tokens),
            long_term_memory = COALESCE(?17, long_term_memory),
            memory_enabled = COALESCE(?18, memory_enabled),
            api_key_encrypted = COALESCE(?19, api_key_encrypted),
            thinking_mode = COALESCE(?20, thinking_mode),
            updated_at = ?21
        WHERE id = ?1 AND is_deleted = 0"#,
        rusqlite::params![
            &req.id, &req.name, &req.avatar_path, &req.detailed_persona, &req.simplified_persona,
            &req.personality, &req.scenario, &req.example_messages, &req.first_message, &req.creator_notes,
            &req.tags, &req.model_provider, &req.model_name, &req.base_url,
            req.temperature, req.max_tokens,
            req.long_term_memory, req.memory_enabled.map(|v| v as i32),
            api_key_encrypted,
            req.thinking_mode.map(|v| v as i32),
            now,
        ],
    )?;
    
    get_by_id(conn, &req.id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn clear_long_term_memory(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE agents SET long_term_memory = '' WHERE id = ?1",
        [id],
    )?;
    Ok(())
}

pub fn soft_delete(conn: &Connection, id: &str) -> Result<bool> {
    let now = chrono::Utc::now().timestamp_millis();
    let rows = conn.execute(
        "UPDATE agents SET is_deleted = 1, deleted_at = ?2 WHERE id = ?1 AND is_deleted = 0",
        (id, now),
    )?;
    Ok(rows > 0)
}

pub fn get_agent_by_name(conn: &Connection, name: &str) -> Result<Option<Agent>> {
    let mut stmt = conn.prepare(
        &format!("SELECT {} FROM agents WHERE name = ?1 AND is_deleted = 0", SELECT_COLUMNS)
    )?;
    let mut rows = stmt.query_map([name], row_to_agent)?;
    rows.next().transpose()
}
