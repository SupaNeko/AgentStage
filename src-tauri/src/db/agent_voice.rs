use rusqlite::{params, Connection, Result};

use crate::models::agent_voice::{AgentVoice, SaveAgentVoiceRequest, VoiceCacheItem};

const VOICE_COLUMNS: &str = "id, agent_id, model_name, model_path, speaker_id, target_language, emotion_params, speed, translate_enabled, translate_model_config_id, generation_mode, created_at, updated_at";

fn row_to_agent_voice(row: &rusqlite::Row) -> Result<AgentVoice> {
    Ok(AgentVoice {
        id: row.get(0)?,
        agent_id: row.get(1)?,
        model_name: row.get(2)?,
        model_path: row.get(3)?,
        speaker_id: row.get(4)?,
        target_language: row.get(5)?,
        emotion_params: row.get(6)?,
        speed: row.get(7)?,
        translate_enabled: row.get::<_, i64>(8)? != 0,
        translate_model_config_id: row.get(9)?,
        generation_mode: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

/// 保存角色语音配置（每个角色仅一条，存在则更新）
pub fn save_agent_voice(conn: &Connection, req: &SaveAgentVoiceRequest) -> Result<AgentVoice> {
    let now = chrono::Utc::now().timestamp_millis();
    let existing = get_agent_voice_by_agent_id(conn, &req.agent_id)?;

    let id = match &existing {
        Some(v) => {
            conn.execute(
                "UPDATE agent_voices SET model_name = ?1, model_path = ?2, speaker_id = ?3, target_language = ?4, emotion_params = ?5, speed = ?6, translate_enabled = ?7, translate_model_config_id = ?8, generation_mode = ?9, updated_at = ?10 WHERE id = ?11",
                params![
                    req.model_name,
                    req.model_path,
                    req.speaker_id,
                    req.target_language,
                    req.emotion_params,
                    req.speed,
                    if req.translate_enabled { 1 } else { 0 },
                    req.translate_model_config_id,
                    req.generation_mode,
                    now,
                    v.id,
                ],
            )?;
            v.id.clone()
        }
        None => {
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO agent_voices (id, agent_id, model_name, model_path, speaker_id, target_language, emotion_params, speed, translate_enabled, translate_model_config_id, generation_mode, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    id,
                    req.agent_id,
                    req.model_name,
                    req.model_path,
                    req.speaker_id,
                    req.target_language,
                    req.emotion_params,
                    req.speed,
                    if req.translate_enabled { 1 } else { 0 },
                    req.translate_model_config_id,
                    req.generation_mode,
                    now,
                    now,
                ],
            )?;
            id
        }
    };

    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM agent_voices WHERE id = ?1",
        VOICE_COLUMNS
    ))?;
    stmt.query_row([id], row_to_agent_voice)
}

pub fn get_agent_voice_by_agent_id(conn: &Connection, agent_id: &str) -> Result<Option<AgentVoice>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM agent_voices WHERE agent_id = ?1",
        VOICE_COLUMNS
    ))?;
    let mut rows = stmt.query_map([agent_id], row_to_agent_voice)?;
    rows.next().transpose()
}

pub fn delete_agent_voice(conn: &Connection, agent_id: &str) -> Result<()> {
    conn.execute("DELETE FROM agent_voices WHERE agent_id = ?1", [agent_id])?;
    Ok(())
}

const CACHE_COLUMNS: &str = "id, message_id, session_id, agent_id, file_path, file_size, created_at";

fn row_to_cache_item(row: &rusqlite::Row) -> Result<VoiceCacheItem> {
    Ok(VoiceCacheItem {
        id: row.get(0)?,
        message_id: row.get(1)?,
        session_id: row.get(2)?,
        agent_id: row.get(3)?,
        file_path: row.get(4)?,
        file_size: row.get::<_, Option<i64>>(5)?.unwrap_or(0),
        created_at: row.get(6)?,
    })
}

pub fn insert_vits_cache(
    conn: &Connection,
    message_id: &str,
    session_id: &str,
    agent_id: &str,
    file_path: &str,
    file_size: i64,
) -> Result<VoiceCacheItem> {
    let now = chrono::Utc::now().timestamp_millis();
    // 同一 message 重复生成时替换旧记录
    conn.execute("DELETE FROM vits_cache WHERE message_id = ?1", [message_id])?;
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO vits_cache (id, message_id, session_id, agent_id, file_path, file_size, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, message_id, session_id, agent_id, file_path, file_size, now],
    )?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM vits_cache WHERE id = ?1",
        CACHE_COLUMNS
    ))?;
    stmt.query_row([id], row_to_cache_item)
}

pub fn get_vits_cache_by_message_id(conn: &Connection, message_id: &str) -> Result<Option<VoiceCacheItem>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM vits_cache WHERE message_id = ?1",
        CACHE_COLUMNS
    ))?;
    let mut rows = stmt.query_map([message_id], row_to_cache_item)?;
    rows.next().transpose()
}

pub fn get_vits_cache_by_id(conn: &Connection, id: &str) -> Result<Option<VoiceCacheItem>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM vits_cache WHERE id = ?1",
        CACHE_COLUMNS
    ))?;
    let mut rows = stmt.query_map([id], row_to_cache_item)?;
    rows.next().transpose()
}

pub fn list_vits_cache(conn: &Connection, agent_id: Option<&str>) -> Result<Vec<VoiceCacheItem>> {
    match agent_id {
        Some(aid) => {
            let mut stmt = conn.prepare(&format!(
                "SELECT {} FROM vits_cache WHERE agent_id = ?1 ORDER BY created_at DESC",
                CACHE_COLUMNS
            ))?;
            let rows = stmt.query_map([aid], row_to_cache_item)?;
            rows.collect()
        }
        None => {
            let mut stmt = conn.prepare(&format!(
                "SELECT {} FROM vits_cache ORDER BY created_at DESC",
                CACHE_COLUMNS
            ))?;
            let rows = stmt.query_map([], row_to_cache_item)?;
            rows.collect()
        }
    }
}

pub fn delete_vits_cache(conn: &Connection, id: &str) -> Result<()> {
    conn.execute("DELETE FROM vits_cache WHERE id = ?1", [id])?;
    Ok(())
}

pub fn clear_vits_cache(conn: &Connection, session_id: Option<&str>) -> Result<()> {
    match session_id {
        Some(sid) => {
            conn.execute("DELETE FROM vits_cache WHERE session_id = ?1", [sid])?;
        }
        None => {
            conn.execute("DELETE FROM vits_cache", [])?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::BASE_SCHEMA;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        // 单测聚焦仓库逻辑本身，不构造 agents/messages 父表数据链
        conn.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
        conn.execute_batch(BASE_SCHEMA).unwrap();
        conn
    }

    fn sample_request(agent_id: &str) -> SaveAgentVoiceRequest {
        SaveAgentVoiceRequest {
            agent_id: agent_id.to_string(),
            model_name: "test-model".to_string(),
            model_path: "D:\\models\\test".to_string(),
            speaker_id: None,
            target_language: "ja".to_string(),
            emotion_params: None,
            speed: 1.0,
            translate_enabled: true,
            translate_model_config_id: None,
            generation_mode: "auto_silent".to_string(),
        }
    }

    #[test]
    fn test_save_and_get_agent_voice() {
        let conn = setup();
        let saved = save_agent_voice(&conn, &sample_request("agent1")).unwrap();
        assert_eq!(saved.agent_id, "agent1");
        assert_eq!(saved.model_name, "test-model");
        assert!(saved.translate_enabled);
        assert_eq!(saved.speed, 1.0);

        let loaded = get_agent_voice_by_agent_id(&conn, "agent1").unwrap();
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.id, saved.id);
        assert_eq!(loaded.target_language, "ja");

        let missing = get_agent_voice_by_agent_id(&conn, "nobody").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_save_agent_voice_upsert_keeps_single_row() {
        let conn = setup();
        let first = save_agent_voice(&conn, &sample_request("agent1")).unwrap();

        let mut req = sample_request("agent1");
        req.model_name = "other-model".to_string();
        req.speed = 1.5;
        req.translate_enabled = false;
        req.generation_mode = "manual".to_string();
        let second = save_agent_voice(&conn, &req).unwrap();

        // 更新应复用原记录 id，不产生新行
        assert_eq!(first.id, second.id);
        assert_eq!(second.model_name, "other-model");
        assert!(!second.translate_enabled);
        assert_eq!(second.generation_mode, "manual");

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM agent_voices WHERE agent_id = 'agent1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_delete_agent_voice() {
        let conn = setup();
        save_agent_voice(&conn, &sample_request("agent1")).unwrap();
        delete_agent_voice(&conn, "agent1").unwrap();
        assert!(get_agent_voice_by_agent_id(&conn, "agent1").unwrap().is_none());
        // 重复删除应幂等
        delete_agent_voice(&conn, "agent1").unwrap();
    }

    #[test]
    fn test_vits_cache_crud() {
        let conn = setup();
        let item = insert_vits_cache(&conn, "msg1", "sess1", "agent1", "C:\\tmp\\a.wav", 1024).unwrap();
        assert_eq!(item.message_id, "msg1");
        assert_eq!(item.file_size, 1024);

        let by_msg = get_vits_cache_by_message_id(&conn, "msg1").unwrap().unwrap();
        assert_eq!(by_msg.id, item.id);

        let list = list_vits_cache(&conn, None).unwrap();
        assert_eq!(list.len(), 1);
        let list_by_agent = list_vits_cache(&conn, Some("agent1")).unwrap();
        assert_eq!(list_by_agent.len(), 1);
        let list_other = list_vits_cache(&conn, Some("agent2")).unwrap();
        assert!(list_other.is_empty());

        delete_vits_cache(&conn, &item.id).unwrap();
        assert!(get_vits_cache_by_message_id(&conn, "msg1").unwrap().is_none());
    }

    #[test]
    fn test_insert_vits_cache_replaces_same_message() {
        let conn = setup();
        insert_vits_cache(&conn, "msg1", "sess1", "agent1", "C:\\tmp\\a.wav", 100).unwrap();
        let second = insert_vits_cache(&conn, "msg1", "sess1", "agent1", "C:\\tmp\\b.wav", 200).unwrap();
        assert_eq!(second.file_path, "C:\\tmp\\b.wav");

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM vits_cache WHERE message_id = 'msg1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_clear_vits_cache_scoped_by_session() {
        let conn = setup();
        insert_vits_cache(&conn, "msg1", "sess1", "agent1", "C:\\tmp\\a.wav", 100).unwrap();
        insert_vits_cache(&conn, "msg2", "sess2", "agent1", "C:\\tmp\\b.wav", 100).unwrap();

        clear_vits_cache(&conn, Some("sess1")).unwrap();
        assert!(get_vits_cache_by_message_id(&conn, "msg1").unwrap().is_none());
        assert!(get_vits_cache_by_message_id(&conn, "msg2").unwrap().is_some());

        clear_vits_cache(&conn, None).unwrap();
        assert!(list_vits_cache(&conn, None).unwrap().is_empty());
    }
}
