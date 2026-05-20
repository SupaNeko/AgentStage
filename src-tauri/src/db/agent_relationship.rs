use rusqlite::{Connection, Result};
use crate::models::agent_relationship::RelationshipItem;

pub fn get_relationship(
    conn: &Connection,
    observer_id: &str,
    target_id: &str,
    target_type: &str,
) -> Result<String> {
    crate::logger::backend("DEBUG", &format!(
        "[DEBUG agent_relationship::get_relationship] observer_id={}, target_id={}, target_type={}",
        observer_id, target_id, target_type
    ));
    let text: Result<String> = conn.query_row(
        "SELECT relationship_text FROM agent_relationships WHERE observer_id = ?1 AND target_id = ?2 AND target_type = ?3",
        (observer_id, target_id, target_type),
        |row| row.get(0),
    );
    match text {
        Ok(t) => {
            crate::logger::backend("DEBUG", &format!(
                "[DEBUG agent_relationship::get_relationship] found text='{}'", t
            ));
            Ok(t)
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            crate::logger::backend("DEBUG", "[DEBUG agent_relationship::get_relationship] no rows, returning empty");
            Ok(String::new())
        }
        Err(e) => {
            crate::logger::backend("ERROR", &format!(
                "[DEBUG agent_relationship::get_relationship] error: {}", e
            ));
            Err(e)
        }
    }
}

pub fn upsert_relationship(
    conn: &Connection,
    observer_id: &str,
    target_id: &str,
    target_type: &str,
    relationship_text: &str,
) -> Result<()> {
    crate::logger::backend("DEBUG", &format!(
        "[DEBUG agent_relationship::upsert_relationship] observer_id={}, target_id={}, target_type={}, text='{}'",
        observer_id, target_id, target_type, relationship_text
    ));
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO agent_relationships (observer_id, target_id, target_type, relationship_text, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(observer_id, target_id, target_type) DO UPDATE SET
             relationship_text = excluded.relationship_text,
             updated_at = excluded.updated_at",
        (observer_id, target_id, target_type, relationship_text, now),
    )?;
    crate::logger::backend("DEBUG", "[DEBUG agent_relationship::upsert_relationship] success");
    Ok(())
}

pub fn list_relationships_by_observer(
    conn: &Connection,
    observer_id: &str,
) -> Result<Vec<RelationshipItem>> {
    crate::logger::backend("DEBUG", &format!(
        "[DEBUG agent_relationship::list_relationships_by_observer] observer_id={}", observer_id
    ));
    let mut stmt = conn.prepare(
        r#"
        SELECT target_id, target_type, target_name, target_avatar, target_label, relationship_text, updated_at
        FROM (
            -- 1. 当前激活的用户人设
            SELECT 
                up.id as target_id,
                'user_persona' as target_type,
                up.name as target_name,
                up.avatar_path as target_avatar,
                '用户' as target_label,
                COALESCE(ar.relationship_text, '') as relationship_text,
                COALESCE(ar.memory_text, '') as memory_text,
                COALESCE(ar.updated_at, 0) as updated_at,
                0 as sort_order
            FROM app_settings s
            JOIN user_personas up ON up.id = s.active_persona_id
            LEFT JOIN agent_relationships ar 
                ON ar.observer_id = ?1 AND ar.target_id = up.id AND ar.target_type = 'user_persona'
            WHERE s.id = 1

            UNION ALL

            -- 2. 好友（agent）
            SELECT 
                a.id as target_id,
                'agent' as target_type,
                a.name as target_name,
                a.avatar_path as target_avatar,
                '好友' as target_label,
                COALESCE(ar.relationship_text, '') as relationship_text,
                COALESCE(ar.memory_text, '') as memory_text,
                COALESCE(ar.updated_at, 0) as updated_at,
                1 as sort_order
            FROM friendships f
            JOIN agents a ON a.id = f.agent_id_2
            LEFT JOIN agent_relationships ar 
                ON ar.observer_id = ?1 AND ar.target_id = a.id AND ar.target_type = 'agent'
            WHERE f.agent_id_1 = ?1 AND f.participant_type_2 = 'agent' AND a.is_deleted = 0

            UNION ALL

            -- 3. 群友（agent，排除好友）
            SELECT 
                a.id as target_id,
                'agent' as target_type,
                a.name as target_name,
                a.avatar_path as target_avatar,
                '群友' as target_label,
                COALESCE(ar.relationship_text, '') as relationship_text,
                COALESCE(ar.memory_text, '') as memory_text,
                COALESCE(ar.updated_at, 0) as updated_at,
                2 as sort_order
            FROM group_members gm_observer
            JOIN group_members gm_target ON gm_observer.session_id = gm_target.session_id
            JOIN group_sessions gs ON gs.session_id = gm_observer.session_id AND gs.is_dissolved = 0
            JOIN agents a ON a.id = gm_target.participant_id AND gm_target.participant_type = 'agent'
            LEFT JOIN agent_relationships ar 
                ON ar.observer_id = ?1 AND ar.target_id = a.id AND ar.target_type = 'agent'
            LEFT JOIN friendships f 
                ON f.agent_id_1 = ?1 AND f.agent_id_2 = a.id AND f.participant_type_2 = 'agent'
            WHERE gm_observer.participant_id = ?1 AND gm_observer.participant_type = 'agent'
              AND gm_target.participant_id != ?1
              AND f.agent_id_2 IS NULL
              AND a.is_deleted = 0
            GROUP BY a.id
        )
        ORDER BY sort_order, target_name
        "#
    )?;

    let rows = stmt.query_map([observer_id], |row| {
        Ok(RelationshipItem {
            target_id: row.get("target_id")?,
            target_type: row.get("target_type")?,
            target_name: row.get("target_name")?,
            target_avatar: crate::db::resolve_avatar_path(row.get("target_avatar")?),
            target_label: row.get("target_label")?,
            relationship_text: row.get("relationship_text")?,
            memory_text: row.get("memory_text")?,
            updated_at: row.get("updated_at")?,
        })
    })?;

    let result: Result<Vec<RelationshipItem>> = rows.collect();
    match &result {
        Ok(items) => crate::logger::backend("DEBUG", &format!(
            "[DEBUG agent_relationship::list_relationships_by_observer] returned {} items", items.len()
        )),
        Err(e) => crate::logger::backend("ERROR", &format!(
            "[DEBUG agent_relationship::list_relationships_by_observer] error: {}", e
        )),
    }
    result
}

pub fn delete_relationships_by_target(
    conn: &Connection,
    target_id: &str,
    target_type: &str,
) -> Result<()> {
    conn.execute(
        "DELETE FROM agent_relationships WHERE target_id = ?1 AND target_type = ?2",
        (target_id, target_type),
    )?;
    Ok(())
}

pub fn add_friendship(conn: &Connection, agent_id_1: &str, agent_id_2: &str) -> Result<()> {
    crate::logger::backend("DEBUG", &format!(
        "[DEBUG agent_relationship::add_friendship] agent_id_1={}, agent_id_2={}",
        agent_id_1, agent_id_2
    ));
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT OR IGNORE INTO friendships (id, agent_id_1, agent_id_2, participant_type_2, created_at, source_session_id)
         VALUES (?1, ?2, ?3, 'agent', ?4, NULL)",
        rusqlite::params![uuid::Uuid::new_v4().to_string(), agent_id_1, agent_id_2, now],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO friendships (id, agent_id_1, agent_id_2, participant_type_2, created_at, source_session_id)
         VALUES (?1, ?2, ?3, 'agent', ?4, NULL)",
        rusqlite::params![uuid::Uuid::new_v4().to_string(), agent_id_2, agent_id_1, now],
    )?;
    crate::logger::backend("DEBUG", "[DEBUG agent_relationship::add_friendship] success");
    Ok(())
}

pub fn remove_friendship(conn: &Connection, agent_id_1: &str, agent_id_2: &str) -> Result<()> {
    crate::logger::backend("DEBUG", &format!(
        "[DEBUG agent_relationship::remove_friendship] agent_id_1={}, agent_id_2={}",
        agent_id_1, agent_id_2
    ));
    conn.execute(
        "DELETE FROM friendships WHERE agent_id_1 = ?1 AND agent_id_2 = ?2 AND participant_type_2 = 'agent'",
        (agent_id_1, agent_id_2),
    )?;
    conn.execute(
        "DELETE FROM friendships WHERE agent_id_1 = ?1 AND agent_id_2 = ?2 AND participant_type_2 = 'agent'",
        (agent_id_2, agent_id_1),
    )?;
    crate::logger::backend("DEBUG", "[DEBUG agent_relationship::remove_friendship] success");
    Ok(())
}

pub fn upsert_memory(
    conn: &Connection,
    observer_id: &str,
    target_id: &str,
    target_type: &str,
    memory_text: &str,
) -> Result<()> {
    crate::logger::backend("DEBUG", &format!(
        "[DEBUG agent_relationship::upsert_memory] observer_id={}, target_id={}, target_type={}, text_len={}",
        observer_id, target_id, target_type, memory_text.len()
    ));
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO agent_relationships (observer_id, target_id, target_type, memory_text, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(observer_id, target_id, target_type) DO UPDATE SET
             memory_text = excluded.memory_text,
             updated_at = excluded.updated_at",
        (observer_id, target_id, target_type, memory_text, now),
    )?;
    crate::logger::backend("DEBUG", "[DEBUG agent_relationship::upsert_memory] success");
    Ok(())
}

pub fn clear_memories_by_observer(conn: &Connection, observer_id: &str) -> Result<()> {
    crate::logger::backend("DEBUG", &format!(
        "[DEBUG agent_relationship::clear_memories_by_observer] observer_id={}", observer_id
    ));
    conn.execute(
        "UPDATE agent_relationships SET memory_text = '' WHERE observer_id = ?1",
        [observer_id],
    )?;
    crate::logger::backend("DEBUG", "[DEBUG agent_relationship::clear_memories_by_observer] success");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn init_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = OFF;", []).unwrap();
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
        conn
    }

    #[test]
    fn test_add_friendship_creates_bidirectional_records() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Agent One", 0i64),
        ).unwrap();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent2", "Agent Two", 0i64),
        ).unwrap();

        add_friendship(&conn, "agent1", "agent2").unwrap();

        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM friendships WHERE participant_type_2 = 'agent'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 2, "add_friendship should create bidirectional records");

        // Verify both directions exist
        let forward: i32 = conn.query_row(
            "SELECT COUNT(*) FROM friendships WHERE agent_id_1 = 'agent1' AND agent_id_2 = 'agent2'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(forward, 1);

        let reverse: i32 = conn.query_row(
            "SELECT COUNT(*) FROM friendships WHERE agent_id_1 = 'agent2' AND agent_id_2 = 'agent1'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(reverse, 1);
    }

    #[test]
    fn test_add_friendship_is_idempotent() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Agent One", 0i64),
        ).unwrap();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent2", "Agent Two", 0i64),
        ).unwrap();

        add_friendship(&conn, "agent1", "agent2").unwrap();
        add_friendship(&conn, "agent1", "agent2").unwrap(); // duplicate

        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM friendships WHERE participant_type_2 = 'agent'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 2, "Duplicate add_friendship should not create extra records");
    }

    #[test]
    fn test_remove_friendship_deletes_bidirectional_records() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Agent One", 0i64),
        ).unwrap();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent2", "Agent Two", 0i64),
        ).unwrap();

        add_friendship(&conn, "agent1", "agent2").unwrap();
        remove_friendship(&conn, "agent1", "agent2").unwrap();

        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM friendships WHERE participant_type_2 = 'agent'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 0, "remove_friendship should delete both directions");
    }

    #[test]
    fn test_list_relationships_excludes_dissolved_groups() {
        let conn = init_test_db();

        // Insert app settings and user persona (required by list_relationships_by_observer)
        conn.execute(
            "INSERT INTO app_settings (id, updated_at) VALUES (1, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO user_personas (id, name, description, created_at, updated_at) VALUES ('up1', 'User', '', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "UPDATE app_settings SET active_persona_id = 'up1' WHERE id = 1",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Agent One", 0i64),
        ).unwrap();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent2", "Agent Two", 0i64),
        ).unwrap();

        // Create group session with agent1 and agent2
        let session = crate::db::session::create_group_session(&conn, "Test Group", &["agent1".into(), "agent2".into()]).unwrap();

        // Before disband: agent1 should see agent2 as groupmate
        let relationships = list_relationships_by_observer(&conn, "agent1").unwrap();
        let groupmate = relationships.iter().find(|r| r.target_id == "agent2" && r.target_label == "群友");
        assert!(groupmate.is_some(), "agent2 should appear as groupmate before disband");

        // Disband group
        crate::db::session::disband_group(&conn, &session.id).unwrap();

        // After disband: agent1 should no longer see agent2 as groupmate
        let relationships_after = list_relationships_by_observer(&conn, "agent1").unwrap();
        let groupmate_after = relationships_after.iter().find(|r| r.target_id == "agent2");
        assert!(groupmate_after.is_none(), "agent2 should NOT appear as groupmate after group is dissolved");
    }
}
