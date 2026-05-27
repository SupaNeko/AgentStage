use rusqlite::{Connection, Result, Row};
use crate::models::model_config::{ModelConfig, CreateModelConfigRequest, UpdateModelConfigRequest};
use uuid::Uuid;

const SELECT_COLUMNS: &str = "id, name, provider, model_name, base_url, api_key_encrypted, temperature, max_tokens, top_p, presence_penalty, frequency_penalty, created_at, updated_at";

fn row_to_model_config(row: &Row) -> Result<ModelConfig> {
    Ok(ModelConfig {
        id: row.get(0)?,
        name: row.get(1)?,
        provider: row.get(2)?,
        model_name: row.get(3)?,
        base_url: row.get(4)?,
        api_key_encrypted: row.get(5)?,
        temperature: row.get(6)?,
        max_tokens: row.get(7)?,
        top_p: row.get(8)?,
        presence_penalty: row.get(9)?,
        frequency_penalty: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

pub fn create(conn: &Connection, req: &CreateModelConfigRequest) -> Result<ModelConfig> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let api_key_encrypted = if req.api_key.is_empty() {
        None
    } else {
        Some(crate::crypto::encrypt(&req.api_key)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e))))?)
    };

    conn.execute(
        r#"INSERT INTO model_configs (
            id, name, provider, model_name, base_url, api_key_encrypted,
            temperature, max_tokens, top_p, presence_penalty, frequency_penalty, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"#,
        rusqlite::params![
            &id, &req.name, &req.provider, &req.model_name, &req.base_url, &api_key_encrypted,
            req.temperature, req.max_tokens, req.top_p,
            req.presence_penalty, req.frequency_penalty,
            now, now,
        ],
    )?;

    get_by_id(conn, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<Option<ModelConfig>> {
    let mut stmt = conn.prepare(
        &format!("SELECT {} FROM model_configs WHERE id = ?1", SELECT_COLUMNS)
    )?;
    let mut rows = stmt.query_map([id], row_to_model_config)?;
    rows.next().transpose()
}

pub fn list_all(conn: &Connection) -> Result<Vec<ModelConfig>> {
    let mut stmt = conn.prepare(
        &format!("SELECT {} FROM model_configs ORDER BY created_at DESC", SELECT_COLUMNS)
    )?;
    let rows = stmt.query_map([], row_to_model_config)?;
    rows.collect()
}

pub fn update(conn: &Connection, req: &UpdateModelConfigRequest) -> Result<ModelConfig> {
    let now = chrono::Utc::now().timestamp_millis();
    let api_key_encrypted = req.api_key.as_ref()
        .map(|k| {
            if k.is_empty() {
                Ok(None)
            } else {
                crate::crypto::encrypt(k).map(Some)
            }
        })
        .transpose()
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e))) )?;

    // Helper: build CASE WHEN expression for nullable fields
    // When outer Option is Some -> update to inner value (which may be None)
    // When outer Option is None -> don't update (keep existing)
    fn nullable_expr(param_idx: usize, field: &str) -> String {
        format!("CASE WHEN ?{} IS NOT NULL THEN ?{} ELSE {} END", param_idx, param_idx + 1, field)
    }

    let temp_expr = nullable_expr(7, "temperature");
    let max_tokens_expr = nullable_expr(9, "max_tokens");
    let top_p_expr = nullable_expr(11, "top_p");
    let presence_penalty_expr = nullable_expr(13, "presence_penalty");
    let frequency_penalty_expr = nullable_expr(15, "frequency_penalty");

    // Flag params (0 or 1) to indicate whether the field was provided
    let temp_flag: Option<i32> = req.temperature.map(|_| 1);
    let temp_value: Option<f64> = req.temperature.flatten();
    let max_tokens_flag: Option<i32> = req.max_tokens.map(|_| 1);
    let max_tokens_value: Option<i32> = req.max_tokens.flatten();
    let top_p_flag: Option<i32> = req.top_p.map(|_| 1);
    let top_p_value: Option<f64> = req.top_p.flatten();
    let presence_penalty_flag: Option<i32> = req.presence_penalty.map(|_| 1);
    let presence_penalty_value: Option<f64> = req.presence_penalty.flatten();
    let frequency_penalty_flag: Option<i32> = req.frequency_penalty.map(|_| 1);
    let frequency_penalty_value: Option<f64> = req.frequency_penalty.flatten();

    conn.execute(
        &format!(
            r#"UPDATE model_configs SET
                name = COALESCE(?2, name),
                provider = COALESCE(?3, provider),
                model_name = COALESCE(?4, model_name),
                base_url = COALESCE(?5, base_url),
                api_key_encrypted = COALESCE(?6, api_key_encrypted),
                temperature = {},
                max_tokens = {},
                top_p = {},
                presence_penalty = {},
                frequency_penalty = {},
                updated_at = ?17
            WHERE id = ?1"#,
            temp_expr, max_tokens_expr, top_p_expr, presence_penalty_expr, frequency_penalty_expr
        ),
        rusqlite::params![
            &req.id, &req.name, &req.provider, &req.model_name, &req.base_url,
            &api_key_encrypted,
            temp_flag, temp_value,
            max_tokens_flag, max_tokens_value,
            top_p_flag, top_p_value,
            presence_penalty_flag, presence_penalty_value,
            frequency_penalty_flag, frequency_penalty_value,
            now,
        ],
    )?;

    get_by_id(conn, &req.id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn delete(conn: &Connection, id: &str) -> Result<bool> {
    let count = count_referencing_agents(conn, id)?;
    if count > 0 {
        return Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT_TRIGGER),
            Some(format!("Cannot delete model config: {} agents are using it", count)),
        ));
    }

    let rows = conn.execute(
        "DELETE FROM model_configs WHERE id = ?1",
        [id],
    )?;
    Ok(rows > 0)
}

pub fn count_referencing_agents(conn: &Connection, id: &str) -> Result<i32> {
    let mut stmt = conn.prepare(
        "SELECT COUNT(*) FROM agents WHERE model_config_id = ?1"
    )?;
    let count: i32 = stmt.query_row([id], |row| row.get(0))?;
    Ok(count)
}

pub fn resolve_summary_model_config(conn: &Connection, settings: &crate::models::settings::AppSettings) -> Result<Option<ModelConfig>, String> {
    // 1. Try the configured summary model first
    if let Some(ref config_id) = settings.summary_model_config_id {
        if let Ok(Some(config)) = get_by_id(conn, config_id) {
            if config.api_key_encrypted.is_some() {
                return Ok(Some(config));
            }
        }
    }

    // 2. Fallback: first model_config with api_key_encrypted present
    let mut stmt = conn.prepare(
        &format!("SELECT {} FROM model_configs WHERE api_key_encrypted IS NOT NULL ORDER BY created_at DESC LIMIT 1", SELECT_COLUMNS)
    ).map_err(|e| e.to_string())?;
    let mut rows = stmt.query_map([], row_to_model_config).map_err(|e| e.to_string())?;
    if let Some(Ok(config)) = rows.next() {
        return Ok(Some(config));
    }

    Ok(None)
}
