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
                COALESCE(ar.updated_at, 0) as updated_at,
                2 as sort_order
            FROM group_members gm_observer
            JOIN group_members gm_target ON gm_observer.session_id = gm_target.session_id
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
