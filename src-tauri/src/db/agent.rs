use rusqlite::{Connection, Result, Row};
use crate::models::agent::{Agent, AgentResponse, CreateAgentRequest, UpdateAgentRequest};
use uuid::Uuid;

pub struct AgentLlmConfig {
    pub api_key: String,
    pub base_url: Option<String>,
    pub model_name: String,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i32>,
}

pub fn resolve_llm_config(conn: &Connection, agent: &Agent) -> Result<AgentLlmConfig, String> {
    let model_config_id = agent.model_config_id.as_ref()
        .ok_or_else(|| "Agent has no model config".to_string())?;
    let mc = crate::db::model_config::get_by_id(conn, model_config_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Model config not found".to_string())?;

    let api_key = mc.api_key_encrypted
        .as_ref()
        .and_then(|enc| crate::crypto::decrypt(enc).ok())
        .unwrap_or_default();

    if api_key.is_empty() {
        return Err("API key is empty".to_string());
    }

    let temperature = agent.temperature.or(mc.temperature);

    Ok(AgentLlmConfig {
        api_key,
        base_url: mc.base_url,
        model_name: mc.model_name,
        temperature,
        max_tokens: mc.max_tokens,
    })
}

const SELECT_COLUMNS: &str = "id, name, avatar_path, detailed_persona, simplified_persona, personality, scenario, example_messages, first_message, creator_notes, tags, model_config_id, agent_temperature, long_term_memory, memory_enabled, proactive_enabled, proactive_min_minutes, proactive_max_minutes, is_deleted, deleted_at, created_at, updated_at";

const SELECT_COLUMNS_PREFIXED: &str = "a.id, a.name, a.avatar_path, a.detailed_persona, a.simplified_persona, a.personality, a.scenario, a.example_messages, a.first_message, a.creator_notes, a.tags, a.model_config_id, a.agent_temperature, a.long_term_memory, a.memory_enabled, a.proactive_enabled, a.proactive_min_minutes, a.proactive_max_minutes, a.is_deleted, a.deleted_at, a.created_at, a.updated_at";

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
        model_config_id: row.get(11)?,
        temperature: row.get(12)?,
        long_term_memory: row.get(13)?,
        memory_enabled: row.get::<_, i32>(14)? != 0,
        proactive_enabled: row.get::<_, i32>(15)? != 0,
        proactive_min_minutes: row.get(16)?,
        proactive_max_minutes: row.get(17)?,
        is_deleted: row.get::<_, i32>(18)? != 0,
        deleted_at: row.get(19)?,
        created_at: row.get(20)?,
        updated_at: row.get(21)?,
    })
}

pub fn create(conn: &Connection, req: &CreateAgentRequest) -> Result<Agent> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    conn.execute(
        r#"INSERT INTO agents (
            id, name, avatar_path, detailed_persona, simplified_persona,
            personality, scenario, example_messages, first_message, creator_notes, tags,
            model_config_id, agent_temperature, long_term_memory, memory_enabled, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)"#,
        rusqlite::params![
            &id, &req.name, &req.avatar_path, &req.detailed_persona, &req.simplified_persona,
            &req.personality, &req.scenario, &req.example_messages, &req.first_message, &req.creator_notes,
            &req.tags, &req.model_config_id, req.temperature,
            &req.long_term_memory, req.memory_enabled.unwrap_or(true) as i32,
            now, now,
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

    let temp_flag: Option<i32> = req.temperature.as_ref().map(|_| 1);
    let temp_value: Option<f64> = req.temperature.flatten();

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
            model_config_id = COALESCE(?12, model_config_id),
            agent_temperature = CASE WHEN ?13 IS NOT NULL THEN ?14 ELSE agent_temperature END,
            long_term_memory = COALESCE(?15, long_term_memory),
            memory_enabled = COALESCE(?16, memory_enabled),
            updated_at = ?17
        WHERE id = ?1 AND is_deleted = 0"#,
        rusqlite::params![
            &req.id, &req.name, &req.avatar_path, &req.detailed_persona, &req.simplified_persona,
            &req.personality, &req.scenario, &req.example_messages, &req.first_message, &req.creator_notes,
            &req.tags, &req.model_config_id,
            temp_flag, temp_value,
            req.long_term_memory, req.memory_enabled.map(|v| v as i32),
            now,
        ],
    )?;

    get_by_id(conn, &req.id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn update_proactive_config(
    conn: &Connection,
    agent_id: &str,
    enabled: i32,
    min_minutes: i32,
    max_minutes: i32,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE agents SET proactive_enabled = ?1, proactive_min_minutes = ?2, proactive_max_minutes = ?3, updated_at = ?4 WHERE id = ?5",
        rusqlite::params![enabled, min_minutes, max_minutes, chrono::Utc::now().timestamp_millis(), agent_id],
    )?;
    Ok(())
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

fn row_to_agent_response(row: &Row) -> Result<AgentResponse> {
    Ok(AgentResponse {
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
        model_config_id: row.get(11)?,
        model_name: row.get(22)?,
        temperature: row.get(12)?,
        long_term_memory: row.get(13)?,
        memory_enabled: row.get::<_, i32>(14)? != 0,
        proactive_enabled: row.get::<_, i32>(15)? != 0,
        proactive_min_minutes: row.get(16)?,
        proactive_max_minutes: row.get(17)?,
        is_deleted: row.get::<_, i32>(18)? != 0,
        deleted_at: row.get(19)?,
        created_at: row.get(20)?,
        updated_at: row.get(21)?,
    })
}

pub fn list_all_with_model_name(conn: &Connection) -> Result<Vec<AgentResponse>> {
    let sql = format!(
        "SELECT {}, mc.model_name as mc_model_name FROM agents a LEFT JOIN model_configs mc ON a.model_config_id = mc.id WHERE a.is_deleted = 0 ORDER BY a.created_at DESC",
        SELECT_COLUMNS_PREFIXED
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], row_to_agent_response)?;
    rows.collect()
}

pub fn get_by_id_with_model_name(conn: &Connection, id: &str) -> Result<Option<AgentResponse>> {
    let sql = format!(
        "SELECT {}, mc.model_name as mc_model_name FROM agents a LEFT JOIN model_configs mc ON a.model_config_id = mc.id WHERE a.id = ?1 AND a.is_deleted = 0",
        SELECT_COLUMNS_PREFIXED
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map([id], row_to_agent_response)?;
    rows.next().transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::agent::CreateAgentRequest;

    fn init_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V1).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V2).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V3).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V4).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V5).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V6).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V7).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V8).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V9).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V11).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V12).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V13).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V14).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V15).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V16).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V17).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V18).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V19).unwrap();
        conn
    }

    #[test]
    fn test_update_proactive_config() {
        let conn = init_test_db();
        let req = CreateAgentRequest {
            name: "Test Agent".to_string(),
            avatar_path: None,
            detailed_persona: "detailed".to_string(),
            simplified_persona: "simple".to_string(),
            personality: None,
            scenario: None,
            example_messages: None,
            first_message: None,
            creator_notes: None,
            tags: None,
            model_config_id: "test-config-id".to_string(),
            temperature: None,
            long_term_memory: None,
            memory_enabled: None,
        };
        let agent = create(&conn, &req).unwrap();
        assert!(!agent.proactive_enabled);
        assert_eq!(agent.proactive_min_minutes, 90);
        assert_eq!(agent.proactive_max_minutes, 180);

        update_proactive_config(&conn, &agent.id, 1, 30, 60).unwrap();

        let updated = get_by_id(&conn, &agent.id).unwrap().unwrap();
        assert!(updated.proactive_enabled);
        assert_eq!(updated.proactive_min_minutes, 30);
        assert_eq!(updated.proactive_max_minutes, 60);
    }
}
