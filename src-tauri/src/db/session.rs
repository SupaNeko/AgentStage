use rusqlite::{Connection, Result};
use crate::models::session::{SessionResponse, SessionParticipant};
use crate::db::chat_page_participant;
use uuid::Uuid;

fn resolve_participant(
    conn: &Connection,
    participant_type: &str,
    participant_id: &str,
) -> Result<SessionParticipant> {
    if participant_type == "user" {
        let (name, avatar_path): (String, Option<String>) = conn.query_row(
            "SELECT COALESCE(up.name, '用户'), up.avatar_path
             FROM app_settings s
             LEFT JOIN user_personas up ON s.active_persona_id = up.id
             LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap_or(("用户".to_string(), None));
        Ok(SessionParticipant {
            participant_type: participant_type.to_string(),
            participant_id: participant_id.to_string(),
            name,
            avatar_path: crate::db::resolve_avatar_path(avatar_path),
            is_deleted: false,
        })
    } else {
        let result = conn.query_row(
            "SELECT name, avatar_path, is_deleted FROM agents WHERE id = ?1",
            [participant_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, i32>(2)? != 0)),
        );
        let (name, avatar_path, is_deleted) = match result {
            Ok((n, a, d)) => (n, a, d),
            Err(rusqlite::Error::QueryReturnedNoRows) => ("未知角色".to_string(), None, false),
            Err(e) => return Err(e),
        };
        Ok(SessionParticipant {
            participant_type: participant_type.to_string(),
            participant_id: participant_id.to_string(),
            name,
            avatar_path: crate::db::resolve_avatar_path(avatar_path),
            is_deleted,
        })
    }
}

fn get_participants_for_session(
    conn: &Connection,
    session_id: &str,
    session_type: &str,
) -> Result<Vec<SessionParticipant>> {
    if session_type == "private" {
        let mut stmt = conn.prepare(
            "SELECT participant_1_type, participant_1_id, participant_2_type, participant_2_id
             FROM private_sessions
             WHERE session_id = ?1"
        )?;
        let row = stmt.query_row([session_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        let mut participants = Vec::with_capacity(2);
        participants.push(resolve_participant(conn, &row.0, &row.1)?);
        participants.push(resolve_participant(conn, &row.2, &row.3)?);
        Ok(participants)
    } else {
        let mut stmt = conn.prepare(
            "SELECT participant_type, participant_id
             FROM group_members
             WHERE session_id = ?1 AND is_active = 1
             ORDER BY participant_type DESC, participant_id ASC"
        )?;
        let rows = stmt.query_map([session_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
            ))
        })?;
        let mut participants = Vec::new();
        for row in rows {
            let (ptype, pid) = row?;
            participants.push(resolve_participant(conn, &ptype, &pid)?);
        }
        Ok(participants)
    }
}

fn build_session_response_from_row(row: &rusqlite::Row) -> Result<SessionResponse> {
    Ok(SessionResponse {
        id: row.get(0)?,
        session_type: row.get(1)?,
        last_message_at: row.get(2)?,
        unread_count: row.get(3)?,
        participants: Vec::new(),
        group_name: row.get(6)?,
        group_avatar: crate::db::resolve_avatar_path(row.get(7)?),
        mute_enabled: row.get::<_, Option<i32>>(5)?.map(|v| v != 0),
        current_chat_page: row.get(4)?,
        is_dissolved: row.get::<_, Option<i32>>(8)?.map(|v| v != 0).unwrap_or(false),
        last_message_preview: row.get(9)?,
    })
}

pub fn get_session_by_id(conn: &Connection, session_id: &str) -> Result<Option<SessionResponse>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.session_type, s.last_message_at, s.unread_count,
                COALESCE(ps.current_chat_page, gs.current_chat_page, 0),
                ss.mute_enabled,
                gs.name,
                gs.avatar_path,
                gs.is_dissolved,
                (SELECT m.content FROM messages m WHERE m.session_id = s.id AND m.is_deleted = 0 AND m.page_index = COALESCE(ps.current_chat_page, gs.current_chat_page, 0) ORDER BY m.created_at DESC LIMIT 1) as last_message_preview
         FROM sessions s
         LEFT JOIN private_sessions ps ON s.id = ps.session_id
         LEFT JOIN group_sessions gs ON s.id = gs.session_id
         LEFT JOIN session_settings ss ON s.id = ss.session_id
         WHERE s.id = ?1 AND s.is_deleted = 0"
    )?;
    let mut rows = stmt.query_map([session_id], build_session_response_from_row)?;
    if let Some(row) = rows.next() {
        let mut session = row?;
        session.participants = get_participants_for_session(conn, &session.id, &session.session_type)?;
        Ok(Some(session))
    } else {
        Ok(None)
    }
}

pub fn list_sessions(conn: &Connection) -> Result<Vec<SessionResponse>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.session_type, s.last_message_at, s.unread_count,
                COALESCE(ps.current_chat_page, gs.current_chat_page, 0),
                ss.mute_enabled,
                gs.name,
                gs.avatar_path,
                gs.is_dissolved,
                (SELECT m.content FROM messages m WHERE m.session_id = s.id AND m.is_deleted = 0 AND m.page_index = COALESCE(ps.current_chat_page, gs.current_chat_page, 0) ORDER BY m.created_at DESC LIMIT 1) as last_message_preview
         FROM sessions s
         LEFT JOIN private_sessions ps ON s.id = ps.session_id
         LEFT JOIN group_sessions gs ON s.id = gs.session_id
         LEFT JOIN session_settings ss ON s.id = ss.session_id
         WHERE s.is_deleted = 0
         ORDER BY s.last_message_at DESC"
    )?;
    let rows = stmt.query_map([], build_session_response_from_row)?;
    let mut sessions = Vec::new();
    for row in rows {
        let mut session = row?;
        session.participants = get_participants_for_session(conn, &session.id, &session.session_type)?;
        sessions.push(session);
    }
    Ok(sessions)
}

pub fn list_history_sessions(conn: &Connection) -> Result<Vec<SessionResponse>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.session_type, s.last_message_at, s.unread_count,
                COALESCE(ps.current_chat_page, gs.current_chat_page, 0),
                ss.mute_enabled,
                gs.name,
                gs.avatar_path,
                gs.is_dissolved,
                (SELECT m.content FROM messages m WHERE m.session_id = s.id AND m.is_deleted = 0 AND m.page_index = COALESCE(ps.current_chat_page, gs.current_chat_page, 0) ORDER BY m.created_at DESC LIMIT 1) as last_message_preview
         FROM sessions s
         LEFT JOIN private_sessions ps ON s.id = ps.session_id
         LEFT JOIN group_sessions gs ON s.id = gs.session_id
         LEFT JOIN session_settings ss ON s.id = ss.session_id
         WHERE s.is_deleted = 0
           AND (SELECT COUNT(*) FROM chat_pages cp WHERE cp.session_id = s.id) > 1
         ORDER BY s.last_message_at DESC"
    )?;
    let rows = stmt.query_map([], build_session_response_from_row)?;
    let mut sessions = Vec::new();
    for row in rows {
        let mut session = row?;
        session.participants = get_participants_for_session(conn, &session.id, &session.session_type)?;
        sessions.push(session);
    }
    Ok(sessions)
}

pub fn get_private_session_by_agent_id(conn: &Connection, agent_id: &str) -> Result<Option<SessionResponse>> {
    let mut stmt = conn.prepare(
        "SELECT s.id FROM sessions s
         JOIN private_sessions ps ON s.id = ps.session_id
         WHERE s.is_deleted = 0 AND s.session_type = 'private'
           AND (
               (ps.participant_1_type = 'user' AND ps.participant_1_id = 'user' AND ps.participant_2_type = 'agent' AND ps.participant_2_id = ?1)
               OR
               (ps.participant_1_type = 'agent' AND ps.participant_1_id = ?1 AND ps.participant_2_type = 'user' AND ps.participant_2_id = 'user')
           )"
    )?;
    let mut rows = stmt.query_map([agent_id], |row| row.get::<_, String>(0))?;
    if let Some(Ok(id)) = rows.next() {
        get_session_by_id(conn, &id)
    } else {
        Ok(None)
    }
}

pub fn get_private_session_between_agents(conn: &Connection, a_id: &str, b_id: &str) -> Result<Option<SessionResponse>> {
    let mut stmt = conn.prepare(
        "SELECT s.id FROM sessions s
         JOIN private_sessions ps ON s.id = ps.session_id
         WHERE s.is_deleted = 0 AND s.session_type = 'private'
           AND (
               (ps.participant_1_type = 'agent' AND ps.participant_1_id = ?1 AND ps.participant_2_type = 'agent' AND ps.participant_2_id = ?2)
               OR
               (ps.participant_1_type = 'agent' AND ps.participant_1_id = ?2 AND ps.participant_2_type = 'agent' AND ps.participant_2_id = ?1)
           )"
    )?;
    let mut rows = stmt.query_map([a_id, b_id], |row| row.get::<_, String>(0))?;
    if let Some(Ok(id)) = rows.next() {
        get_session_by_id(conn, &id)
    } else {
        Ok(None)
    }
}

fn create_private_session_raw(
    conn: &Connection,
    p1_type: &str,
    p1_id: &str,
    p2_type: &str,
    p2_id: &str,
) -> Result<String> {
    let session_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    conn.execute(
        "INSERT INTO sessions (id, session_type, created_at, updated_at) VALUES (?1, 'private', ?2, ?3)",
        (&session_id, now, now),
    )?;

    conn.execute(
        "INSERT INTO private_sessions (session_id, participant_1_type, participant_1_id, participant_2_type, participant_2_id, message_limit_enabled, created_at) VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)",
        (&session_id, p1_type, p1_id, p2_type, p2_id, now),
    )?;

    conn.execute(
        "INSERT INTO chat_pages (id, session_id, page_index, name, is_active, message_count, created_at, updated_at)
         VALUES (?1, ?2, 0, '默认', 1, 0, ?3, ?3)",
        rusqlite::params![&Uuid::new_v4().to_string(), &session_id, now],
    )?;

    init_session_settings(conn, &session_id, "private")?;

    Ok(session_id)
}

pub fn create_private_session(conn: &Connection, agent_id: &str) -> Result<SessionResponse> {
    if let Some(existing) = get_private_session_by_agent_id(conn, agent_id)? {
        return Ok(existing);
    }

    let tx = conn.unchecked_transaction()?;
    let session_id = create_private_session_raw(conn, "user", "user", "agent", agent_id)?;
    let now = chrono::Utc::now().timestamp_millis();

    conn.execute(
        "INSERT INTO friendships (id, agent_id_1, participant_type_2, created_at, source_session_id) VALUES (?1, ?2, 'user', ?3, ?4)",
        (&Uuid::new_v4().to_string(), agent_id, now, &session_id),
    )?;

    tx.commit()?;
    get_session_by_id(conn, &session_id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn create_agent_agent_session(conn: &Connection, a_id: &str, b_id: &str) -> Result<SessionResponse> {
    if let Some(existing) = get_private_session_between_agents(conn, a_id, b_id)? {
        return Ok(existing);
    }

    let (p1_id, p2_id) = if a_id < b_id { (a_id, b_id) } else { (b_id, a_id) };
    let tx = conn.unchecked_transaction()?;
    let session_id = create_private_session_raw(conn, "agent", p1_id, "agent", p2_id)?;
    tx.commit()?;
    get_session_by_id(conn, &session_id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn soft_delete_session(conn: &Connection, session_id: &str) -> Result<bool> {
    // 先归档当前会话，确保当前 page 的消息进入历史记录
    let _ = reset_session(conn, session_id)?;

    let now = chrono::Utc::now().timestamp_millis();
    let rows = conn.execute(
        "UPDATE sessions SET is_deleted = 1, deleted_at = ?2 WHERE id = ?1 AND is_deleted = 0",
        (session_id, now),
    )?;
    Ok(rows > 0)
}

pub fn clear_session_history(conn: &Connection, session_id: &str) -> Result<bool> {
    let tx = conn.unchecked_transaction()?;

    // 获取当前 page_index，保留当前页面消息，只清除历史归档页面
    let current_page: i32 = conn.query_row(
        "SELECT COALESCE(ps.current_chat_page, gs.current_chat_page, 0)
         FROM sessions s
         LEFT JOIN private_sessions ps ON s.id = ps.session_id
         LEFT JOIN group_sessions gs ON s.id = gs.session_id
         WHERE s.id = ?1",
        [session_id],
        |row| row.get(0),
    ).unwrap_or(0);

    // 1. 只删除非当前页面的历史消息
    conn.execute(
        "DELETE FROM messages WHERE session_id = ?1 AND page_index != ?2",
        (session_id, current_page),
    )?;

    // 2. 只删除非当前页面的历史归档 page
    conn.execute(
        "DELETE FROM chat_pages WHERE session_id = ?1 AND page_index != ?2",
        (session_id, current_page),
    )?;

    // 保留当前 page 的 last_message_at/preview 不变（供 SessionList 显示）

    tx.commit()?;
    Ok(true)
}

pub fn create_group_session(
    conn: &Connection,
    name: &str,
    agent_ids: &[String],
) -> Result<SessionResponse> {
    if agent_ids.len() < 2 {
        return Err(rusqlite::Error::InvalidParameterName(
            "群聊至少需要选择 2 个角色".into()
        ));
    }

    let session_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let tx = conn.unchecked_transaction()?;

    conn.execute(
        "INSERT INTO sessions (id, session_type, created_at, updated_at) VALUES (?1, 'group', ?2, ?3)",
        (&session_id, now, now),
    )?;

    conn.execute(
        "INSERT INTO group_sessions (session_id, name, mute_enabled, created_at) VALUES (?1, ?2, 0, ?3)",
        (&session_id, name, now),
    )?;

    conn.execute(
        "INSERT INTO chat_pages (id, session_id, page_index, name, is_active, message_count, created_at, updated_at)
         VALUES (?1, ?2, 0, '默认', 1, 0, ?3, ?3)",
        rusqlite::params![&Uuid::new_v4().to_string(), &session_id, now],
    )?;

    init_session_settings(conn, &session_id, "group")?;

    conn.execute(
        "INSERT INTO group_members (session_id, participant_type, participant_id, joined_at) VALUES (?1, 'user', 'user', ?2)",
        (&session_id, now),
    )?;

    for agent_id in agent_ids {
        conn.execute(
            "INSERT INTO group_members (session_id, participant_type, participant_id, joined_at) VALUES (?1, 'agent', ?2, ?3)",
            (&session_id, agent_id, now),
        )?;
    }

    tx.commit()?;
    get_session_by_id(conn, &session_id)?
        .ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn get_group_members(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<crate::models::session::GroupMemberResponse>> {
    let mut stmt = conn.prepare(
        "SELECT gm.participant_type, gm.participant_id,
                CASE WHEN gm.participant_type = 'user' THEN COALESCE(up.name, '用户') ELSE COALESCE(a.name, '未知角色') END as name,
                CASE WHEN gm.participant_type = 'user' THEN up.avatar_path ELSE a.avatar_path END as avatar_path
         FROM group_members gm
         LEFT JOIN agents a ON gm.participant_type = 'agent' AND gm.participant_id = a.id
         LEFT JOIN app_settings s ON 1=1
         LEFT JOIN user_personas up ON gm.participant_type = 'user' AND s.active_persona_id = up.id
         WHERE gm.session_id = ?1 AND gm.is_active = 1
         ORDER BY gm.participant_type DESC, name ASC"
    )?;
    let rows = stmt.query_map([session_id], |row| {
        Ok(crate::models::session::GroupMemberResponse {
            participant_type: row.get(0)?,
            participant_id: row.get(1)?,
            name: row.get(2)?,
            avatar_path: crate::db::resolve_avatar_path(row.get(3)?),
        })
    })?;
    rows.collect()
}

pub fn get_session_config(conn: &Connection, session_id: &str, session_type: &str) -> Result<crate::models::session::SessionConfig> {
    let defaults = if session_type == "private" {
        (30, 10)
    } else {
        (80, 30)
    };

    conn.query_row(
        "SELECT ss.session_id, COALESCE(ss.history_limit, ?2), COALESCE(ss.message_limit, ?3),
                ss.message_limit_enabled, ss.mute_enabled,
                COALESCE(ps.agent_message_count, gs.agent_message_count, 0),
                ss.overflow_summary_threshold, ss.last_overflow_summary_index
         FROM session_settings ss
         LEFT JOIN private_sessions ps ON ss.session_id = ps.session_id
         LEFT JOIN group_sessions gs ON ss.session_id = gs.session_id
         WHERE ss.session_id = ?1",
        rusqlite::params![session_id, defaults.0, defaults.1],
        |row| {
            Ok(crate::models::session::SessionConfig {
                session_id: row.get(0)?,
                history_limit: row.get(1)?,
                message_limit: row.get(2)?,
                message_limit_enabled: row.get::<_, i32>(3)? != 0,
                mute_enabled: row.get::<_, i32>(4)? != 0,
                agent_message_count: row.get(5)?,
                overflow_summary_threshold: row.get(6)?,
                last_overflow_summary_index: row.get(7)?,
            })
        },
    )
}

pub fn reset_message_count(conn: &Connection, session_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE private_sessions SET agent_message_count = 0 WHERE session_id = ?1",
        [session_id],
    )?;
    conn.execute(
        "UPDATE group_sessions SET agent_message_count = 0 WHERE session_id = ?1",
        [session_id],
    )?;
    Ok(())
}

pub fn init_session_settings(conn: &Connection, session_id: &str, session_type: &str) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let (history_limit, message_limit) = if session_type == "private" {
        (30, 10)
    } else {
        (80, 30)
    };
    conn.execute(
        "INSERT OR IGNORE INTO session_settings (session_id, history_limit, message_limit, message_limit_enabled, mute_enabled, created_at, updated_at)
         VALUES (?1, ?2, ?3, 1, 0, ?4, ?4)",
        rusqlite::params![session_id, history_limit, message_limit, now],
    )?;
    Ok(())
}

pub fn update_session_config(conn: &Connection, req: &crate::models::session::UpdateSessionConfigRequest) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();

    let history_limit = req.history_limit;
    let message_limit = req.message_limit;
    let message_limit_enabled = req.message_limit_enabled.map(|v| v as i32);
    let mute_enabled = req.mute_enabled.map(|v| v as i32);
    let overflow_summary_threshold = req.overflow_summary_threshold;
    let last_overflow_summary_index = req.last_overflow_summary_index;

    let mut sets = Vec::new();
    let mut params: Vec<&dyn rusqlite::ToSql> = Vec::new();

    if let Some(ref v) = history_limit {
        sets.push("history_limit = ?");
        params.push(v as &dyn rusqlite::ToSql);
    }
    if let Some(ref v) = message_limit {
        sets.push("message_limit = ?");
        params.push(v as &dyn rusqlite::ToSql);
    }
    if let Some(ref v) = message_limit_enabled {
        sets.push("message_limit_enabled = ?");
        params.push(v as &dyn rusqlite::ToSql);
    }
    if let Some(ref v) = mute_enabled {
        sets.push("mute_enabled = ?");
        params.push(v as &dyn rusqlite::ToSql);
    }
    if let Some(ref v) = overflow_summary_threshold {
        sets.push("overflow_summary_threshold = ?");
        params.push(v as &dyn rusqlite::ToSql);
    }
    if let Some(ref v) = last_overflow_summary_index {
        sets.push("last_overflow_summary_index = ?");
        params.push(v as &dyn rusqlite::ToSql);
    }

    if sets.is_empty() {
        return Ok(());
    }

    sets.push("updated_at = ?");
    params.push(&now as &dyn rusqlite::ToSql);
    params.push(&req.session_id as &dyn rusqlite::ToSql);

    let sql = format!("UPDATE session_settings SET {} WHERE session_id = ?", sets.join(", "));
    conn.execute(&sql, rusqlite::params_from_iter(params))?;
    Ok(())
}

pub fn reset_session(conn: &Connection, session_id: &str) -> Result<(String, i32)> {
    let now = chrono::Utc::now().timestamp_millis();
    let tx = conn.unchecked_transaction()?;

    let max_page: i32 = conn.query_row(
        "SELECT COALESCE(MAX(page_index), 0) FROM chat_pages WHERE session_id = ?1",
        [session_id],
        |row| row.get(0),
    ).unwrap_or(0);

    let new_page_index = max_page + 1;
    let page_id = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO chat_pages (id, session_id, page_index, name, is_active, message_count, created_at, updated_at)
         VALUES (?1, ?2, ?3, '续开', 1, 0, ?4, ?4)",
        rusqlite::params![&page_id, session_id, new_page_index, now],
    )?;

    // Insert participant snapshot for the old page (max_page)
    if let Ok(Some(old_page_id)) = chat_page_participant::get_chat_page_id(conn, session_id, max_page) {
        // Private session
        if let Ok((p1_type, p1_id, p2_type, p2_id)) = conn.query_row(
            "SELECT participant_1_type, participant_1_id, participant_2_type, participant_2_id FROM private_sessions WHERE session_id = ?1",
            [session_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?)),
        ) {
            for (ptype, pid) in [(p1_type, p1_id), (p2_type, p2_id)] {
                let (name, avatar, persona): (String, Option<String>, Option<String>) = if ptype == "agent" {
                    conn.query_row(
                        "SELECT name, avatar_path, simplified_persona FROM agents WHERE id = ?1",
                        [&pid],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    ).unwrap_or_else(|_| ("未知角色".to_string(), None, None))
                } else {
                    let (n, a): (String, Option<String>) = conn.query_row(
                        "SELECT COALESCE(up.name, '用户'), up.avatar_path FROM app_settings LEFT JOIN user_personas up ON up.id = app_settings.active_persona_id WHERE app_settings.id = 1",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    ).unwrap_or_else(|_| ("用户".to_string(), None));
                    (n, a, None)
                };
                let _ = chat_page_participant::insert_snapshot(conn, &old_page_id, &pid, &ptype, &name, avatar.as_deref(), persona.as_deref());
            }
        }

        // Group session
        let mut stmt = conn.prepare(
            "SELECT participant_type, participant_id FROM group_members WHERE session_id = ?1"
        )?;
        let rows = stmt.query_map([session_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            if let Ok((ptype, pid)) = row {
                let (name, avatar, persona): (String, Option<String>, Option<String>) = if ptype == "agent" {
                    conn.query_row(
                        "SELECT name, avatar_path, simplified_persona FROM agents WHERE id = ?1",
                        [&pid],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                    ).unwrap_or_else(|_| ("未知角色".to_string(), None, None))
                } else {
                    let (n, a): (String, Option<String>) = conn.query_row(
                        "SELECT COALESCE(up.name, '用户'), up.avatar_path FROM app_settings LEFT JOIN user_personas up ON up.id = app_settings.active_persona_id WHERE app_settings.id = 1",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    ).unwrap_or_else(|_| ("用户".to_string(), None));
                    (n, a, None)
                };
                let _ = chat_page_participant::insert_snapshot(conn, &old_page_id, &pid, &ptype, &name, avatar.as_deref(), persona.as_deref());
            }
        }
    }

    let session_type: String = conn.query_row(
        "SELECT session_type FROM sessions WHERE id = ?1",
        [session_id],
        |row| row.get(0),
    )?;

    if session_type == "private" {
        conn.execute(
            "UPDATE private_sessions SET current_chat_page = ?1, agent_message_count = 0 WHERE session_id = ?2",
            rusqlite::params![new_page_index, session_id],
        )?;
    } else {
        conn.execute(
            "UPDATE group_sessions SET current_chat_page = ?1, agent_message_count = 0 WHERE session_id = ?2",
            rusqlite::params![new_page_index, session_id],
        )?;
    }

    // 清空未读消息
    conn.execute("DELETE FROM agent_unread_queue WHERE session_id = ?1", [session_id])?;

    // 解除冻结
    conn.execute("DELETE FROM session_frozen_states WHERE session_id = ?1", [session_id])?;

    // 清空 overflow summary index
    conn.execute(
        "UPDATE session_settings SET last_overflow_summary_index = 0 WHERE session_id = ?1",
        [session_id],
    )?;

    tx.commit()?;
    Ok((page_id, new_page_index))
}

pub fn disband_group(conn: &Connection, session_id: &str) -> Result<bool> {
    // 先归档当前会话，确保当前 page 的消息进入历史记录
    let _ = reset_session(conn, session_id)?;

    let rows = conn.execute(
        "UPDATE group_sessions SET is_dissolved = 1 WHERE session_id = ?1",
        [session_id],
    )?;
    Ok(rows > 0)
}

pub fn add_group_member(conn: &Connection, session_id: &str, agent_id: &str) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let tx = conn.unchecked_transaction()?;

    conn.execute(
        "INSERT OR IGNORE INTO group_members (session_id, participant_type, participant_id, joined_at) VALUES (?1, 'agent', ?2, ?3)",
        (session_id, agent_id, now),
    )?;

    tx.commit()?;
    Ok(())
}

pub fn remove_group_member(conn: &Connection, session_id: &str, agent_id: &str) -> Result<bool> {
    let rows = conn.execute(
        "DELETE FROM group_members WHERE session_id = ?1 AND participant_type = 'agent' AND participant_id = ?2",
        (session_id, agent_id),
    )?;
    Ok(rows > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = OFF;", []).unwrap();
        conn.execute_batch(crate::db::schema::BASE_SCHEMA).unwrap();
        conn
    }

    fn insert_session(conn: &Connection, session_id: &str, session_type: &str) {
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO sessions (id, session_type, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
            rusqlite::params![session_id, session_type, now],
        ).unwrap();
    }

    fn insert_private_session(conn: &Connection, session_id: &str, agent_id: &str, page: i32) {
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO private_sessions (session_id, participant_1_type, participant_1_id, participant_2_type, participant_2_id, current_chat_page, created_at) VALUES (?1, 'user', 'user', 'agent', ?2, ?3, ?4)",
            rusqlite::params![session_id, agent_id, page, now],
        ).unwrap();
    }

    fn insert_session_settings(conn: &Connection, session_id: &str, history_limit: i32) {
        let now = chrono::Utc::now().timestamp_millis();
        conn.execute(
            "INSERT INTO session_settings (session_id, history_limit, message_limit, message_limit_enabled, mute_enabled, created_at, updated_at) VALUES (?1, ?2, 10, 1, 0, ?3, ?3)",
            rusqlite::params![session_id, history_limit, now],
        ).unwrap();
    }

    #[test]
    fn test_v6_migration_creates_frozen_states_and_unread_queue() {
        let conn = init_test_db();
        
        // Verify session_frozen_states exists
        conn.execute(
            "INSERT INTO session_frozen_states (session_id, is_frozen, frozen_at, updated_at) VALUES ('test', 1, 0, 0)",
            [],
        ).unwrap();
        
        // Verify agent_unread_queue exists
        conn.execute(
            "INSERT INTO agent_unread_queue (session_id, agent_id, message_id, created_at) VALUES ('test', 'agent1', 'msg1', 0)",
            [],
        ).unwrap();
        
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM agent_unread_queue WHERE session_id = 'test'",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_create_group_session_min_2_agents() {
        let conn = init_test_db();
        let result = create_group_session(&conn, "Test Group", &["agent1".into()]);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_group_session_and_get_members() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Agent One", 0i64),
        ).unwrap();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent2", "Agent Two", 0i64),
        ).unwrap();

        let session = create_group_session(&conn, "Test Group", &["agent1".into(), "agent2".into()]).unwrap();
        assert_eq!(session.session_type, "group");
        assert_eq!(session.participants.len(), 3);
        assert_eq!(session.participants[0].participant_type, "user");
        assert_eq!(session.participants[0].name, "用户");
        assert_eq!(session.participants[1].name, "Agent One");
        assert_eq!(session.participants[2].name, "Agent Two");

        let members = get_group_members(&conn, &session.id).unwrap();
        assert_eq!(members.len(), 3);
        assert_eq!(members[0].participant_type, "user");
        assert_eq!(members[0].name, "用户");
        assert_eq!(members[1].name, "Agent One");
        assert_eq!(members[2].name, "Agent Two");
    }

    #[test]
    fn test_session_config_defaults() {
        let conn = init_test_db();
        
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        let config = get_session_config(&conn, &session.id, "private").unwrap();
        assert_eq!(config.history_limit, 30);
        assert_eq!(config.message_limit, 10);
        assert!(config.message_limit_enabled);
        assert!(!config.mute_enabled);
    }

    #[test]
    fn test_update_session_config() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        update_session_config(&conn, &crate::models::session::UpdateSessionConfigRequest {
            session_id: session.id.clone(),
            history_limit: Some(50),
            message_limit: Some(20),
            message_limit_enabled: Some(false),
            mute_enabled: Some(true),
            overflow_summary_threshold: None,
            last_overflow_summary_index: None,
        }).unwrap();
        
        let config = get_session_config(&conn, &session.id, "private").unwrap();
        assert_eq!(config.history_limit, 50);
        assert_eq!(config.message_limit, 20);
        assert!(!config.message_limit_enabled);
        assert!(config.mute_enabled);
    }

    #[test]
    fn test_reset_session_creates_new_page() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        let (page_id, new_page_index) = reset_session(&conn, &session.id).unwrap();
        assert!(!page_id.is_empty());
        assert_eq!(new_page_index, 1);
        
        let page_index: i32 = conn.query_row(
            "SELECT page_index FROM chat_pages WHERE id = ?1",
            [&page_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(page_index, 1);
    }

    #[test]
    fn test_reset_session_returns_tuple_and_increments_page_index() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        let (page_id_1, new_page_index_1) = reset_session(&conn, &session.id).unwrap();
        assert!(!page_id_1.is_empty());
        assert_eq!(new_page_index_1, 1);
        
        let (page_id_2, new_page_index_2) = reset_session(&conn, &session.id).unwrap();
        assert!(!page_id_2.is_empty());
        assert_eq!(new_page_index_2, 2);
        assert_ne!(page_id_1, page_id_2);
    }

    #[test]
    fn test_prompt_assemble_with_new_session() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, ' detailed', 'simple', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        // Insert a user message
        crate::db::message::insert_message(&conn, &session.id, "user", "user", "Hello!", "text", None).unwrap();
        
        // Assemble prompt
        let pending = vec![crate::models::message::Message {
            id: "test-msg".to_string(),
            session_id: session.id.clone(),
            sender_type: "user".to_string(),
            sender_id: "user".to_string(),
            sender_name: "用户".to_string(),
            sender_avatar: None,
            content: "Hello!".to_string(),
            created_at: chrono::Utc::now().timestamp_millis(),
            message_type: "text".to_string(),
            tool_call_data: None,
            generation_info: None,
            is_deleted: false,
            page_index: 0,
        }];
        
        let prompt = crate::llm::prompt::PromptAssembler::assemble(&conn, "agent1", None, None, &pending, &std::collections::HashSet::new());
        assert!(prompt.is_ok(), "PromptAssembler failed: {:?}", prompt.err());
        let parts = prompt.unwrap();
        assert!(parts.user.contains("Hello!"));
        assert!(parts.system.contains("Test Agent") || parts.user.contains("Test Agent"));
    }

    #[test]
    fn test_get_messages_by_session_returns_agent_messages() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        // Insert user message
        let user_msg = crate::db::message::insert_message(&conn, &session.id, "user", "user", "Hello!", "text", None).unwrap();
        assert_eq!(user_msg.sender_type, "user");
        
        // Insert agent message
        let agent_msg = crate::db::message::insert_message(&conn, &session.id, "agent", "agent1", "Hi there!", "text", None).unwrap();
        assert_eq!(agent_msg.sender_type, "agent");
        
        // Query messages
        let messages = crate::db::message::get_messages_by_session(&conn, &session.id, 0, 100, 0).unwrap();
        assert_eq!(messages.len(), 2, "Expected 2 messages, got {}", messages.len());
        
        let agent_messages: Vec<_> = messages.iter().filter(|m| m.sender_type == "agent").collect();
        assert_eq!(agent_messages.len(), 1, "Agent message missing from query results");
        assert_eq!(agent_messages[0].content, "Hi there!");
    }

    #[test]
    fn test_session_config_includes_agent_message_count() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        conn.execute(
            "UPDATE private_sessions SET agent_message_count = 5 WHERE session_id = ?1",
            [&session.id],
        ).unwrap();
        
        let config = get_session_config(&conn, &session.id, "private").unwrap();
        assert_eq!(config.agent_message_count, 5);
    }

    #[test]
    fn test_reset_message_count() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        conn.execute(
            "UPDATE private_sessions SET agent_message_count = 5 WHERE session_id = ?1",
            [&session.id],
        ).unwrap();
        
        reset_message_count(&conn, &session.id).unwrap();
        
        let count: i32 = conn.query_row(
            "SELECT agent_message_count FROM private_sessions WHERE session_id = ?1",
            [&session.id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_list_chat_pages_returns_pages_in_desc_order() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        // Reset once to create page 1
        let _ = reset_session(&conn, &session.id).unwrap();
        
        let pages = crate::db::chat_page::list_chat_pages(&conn, &session.id).unwrap();
        assert_eq!(pages.len(), 2, "Expected 2 pages (default + reset), got {}", pages.len());
        assert_eq!(pages[0].page_index, 1); // DESC order: newest first
        assert_eq!(pages[1].page_index, 0);
    }

    #[test]
    fn test_list_chat_pages_aggregates_message_stats() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        // Insert message to page 0
        crate::db::message::insert_message(&conn, &session.id, "user", "user", "Hello", "text", Some(0)).unwrap();
        
        let pages = crate::db::chat_page::list_chat_pages(&conn, &session.id).unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].message_count, 1);
        assert!(pages[0].updated_at > pages[0].created_at, "updated_at should reflect last message time");
    }

    #[test]
    #[ignore]
    fn diagnose_real_db() {
        let conn = rusqlite::Connection::open(r"D:\code_project\AgentStage\data\agentstage.db").unwrap();
        let mut stmt = conn.prepare("SELECT id, session_id, sender_type, sender_id, content, page_index FROM messages ORDER BY created_at").unwrap();
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i32>(5)?,
            ))
        }).unwrap();
        for row in rows {
            let (id, sid, st, sid2, content, page): (String, String, String, String, String, i32) = row.unwrap();
            eprintln!("msg: id={} session={} sender_type={} sender_id={} page={} content={}", id, sid, st, sid2, page, content.chars().take(30).collect::<String>());
        }
        
        let mut stmt = conn.prepare("SELECT session_id, current_chat_page FROM private_sessions").unwrap();
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
        }).unwrap();
        for row in rows {
            let (sid, page): (String, i32) = row.unwrap();
            eprintln!("private_session: session={} current_chat_page={}", sid, page);
        }
        
        let mut stmt = conn.prepare("SELECT session_id, current_chat_page FROM group_sessions").unwrap();
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)?))
        }).unwrap();
        for row in rows {
            let (sid, page): (String, i32) = row.unwrap();
            eprintln!("group_session: session={} current_chat_page={}", sid, page);
        }
    }

    #[test]
    fn test_get_session_messages_with_page_index() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        // Insert messages to page 0 and page 1
        crate::db::message::insert_message(&conn, &session.id, "user", "user", "Page0", "text", Some(0)).unwrap();
        let _ = reset_session(&conn, &session.id).unwrap();
        crate::db::message::insert_message(&conn, &session.id, "user", "user", "Page1", "text", Some(1)).unwrap();
        
        let page0_msgs = crate::db::message::get_messages_by_session(&conn, &session.id, 0, 100, 0).unwrap();
        assert_eq!(page0_msgs.len(), 1);
        assert_eq!(page0_msgs[0].content, "Page0");
        
        let page1_msgs = crate::db::message::get_messages_by_session(&conn, &session.id, 1, 100, 0).unwrap();
        assert_eq!(page1_msgs.len(), 1);
        assert_eq!(page1_msgs[0].content, "Page1");
    }

    #[test]
    fn test_send_user_message_with_page_index_writes_to_old_page() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        let _ = reset_session(&conn, &session.id).unwrap(); // current_chat_page = 1
        
        // Simulate sending to old page 0
        let msg = crate::db::message::insert_message(&conn, &session.id, "user", "user", "Old page msg", "text", Some(0)).unwrap();
        assert_eq!(msg.page_index, 0);
        
        // current_chat_page should still be 1
        let current_page: i32 = conn.query_row(
            "SELECT current_chat_page FROM private_sessions WHERE session_id = ?1",
            [&session.id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(current_page, 1);
    }

    #[test]
    fn test_create_agent_agent_session() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Agent One", 0i64),
        ).unwrap();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent2", "Agent Two", 0i64),
        ).unwrap();

        let session = create_agent_agent_session(&conn, "agent1", "agent2").unwrap();
        assert_eq!(session.session_type, "private");
        assert_eq!(session.participants.len(), 2);
        let ids: Vec<String> = session.participants.iter().map(|p| p.participant_id.clone()).collect();
        assert!(ids.contains(&"agent1".to_string()));
        assert!(ids.contains(&"agent2".to_string()));

        // idempotent
        let session2 = create_agent_agent_session(&conn, "agent2", "agent1").unwrap();
        assert_eq!(session.id, session2.id);
    }

    #[test]
    fn test_get_private_session_between_agents() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Agent One", 0i64),
        ).unwrap();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent2", "Agent Two", 0i64),
        ).unwrap();

        let session = create_agent_agent_session(&conn, "agent1", "agent2").unwrap();
        let found = get_private_session_between_agents(&conn, "agent1", "agent2").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, session.id);

        let found_rev = get_private_session_between_agents(&conn, "agent2", "agent1").unwrap();
        assert!(found_rev.is_some());
    }

    #[test]
    fn test_get_agent_by_name() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Agent One", 0i64),
        ).unwrap();

        let agent = crate::db::agent::get_agent_by_name(&conn, "Agent One").unwrap();
        assert!(agent.is_some());
        assert_eq!(agent.unwrap().id, "agent1");

        let none = crate::db::agent::get_agent_by_name(&conn, "Unknown").unwrap();
        assert!(none.is_none());
    }

    #[test]
    fn test_add_group_member_does_not_create_friendships() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Agent One", 0i64),
        ).unwrap();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent2", "Agent Two", 0i64),
        ).unwrap();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent3", "Agent Three", 0i64),
        ).unwrap();

        let session = create_group_session(&conn, "Test Group", &["agent1".into(), "agent2".into()]).unwrap();

        // Verify no friendships exist before adding member
        let count_before: i32 = conn.query_row(
            "SELECT COUNT(*) FROM friendships",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count_before, 0, "No friendships should exist before adding member");

        // Add new member
        add_group_member(&conn, &session.id, "agent3").unwrap();

        // Verify still no friendships created
        let count_after: i32 = conn.query_row(
            "SELECT COUNT(*) FROM friendships",
            [],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count_after, 0, "Adding group member should NOT create friendships");
    }

    #[test]
    fn test_disband_group_archives_current_page() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Agent One", 0i64),
        ).unwrap();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent2", "Agent Two", 0i64),
        ).unwrap();

        let session = create_group_session(&conn, "Test Group", &["agent1".into(), "agent2".into()]).unwrap();

        // Verify only 1 page exists before disband
        let page_count_before: i32 = conn.query_row(
            "SELECT COUNT(*) FROM chat_pages WHERE session_id = ?1",
            [&session.id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(page_count_before, 1);

        // Disband group
        disband_group(&conn, &session.id).unwrap();

        // Verify group is dissolved
        let is_dissolved: bool = conn.query_row(
            "SELECT is_dissolved FROM group_sessions WHERE session_id = ?1",
            [&session.id],
            |row| row.get::<_, i32>(0).map(|v| v != 0),
        ).unwrap();
        assert!(is_dissolved, "Group should be dissolved");

        // Verify current page was archived (new page created)
        let page_count_after: i32 = conn.query_row(
            "SELECT COUNT(*) FROM chat_pages WHERE session_id = ?1",
            [&session.id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(page_count_after, 2, "Disband should archive current page by creating a new one");
    }

    #[test]
    fn test_soft_delete_session_archives_current_page() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Agent One", 0i64),
        ).unwrap();

        let session = create_private_session(&conn, "agent1").unwrap();

        // Verify only 1 page exists before delete
        let page_count_before: i32 = conn.query_row(
            "SELECT COUNT(*) FROM chat_pages WHERE session_id = ?1",
            [&session.id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(page_count_before, 1);

        // Soft delete session
        soft_delete_session(&conn, &session.id).unwrap();

        // Verify session is deleted
        let is_deleted: bool = conn.query_row(
            "SELECT is_deleted FROM sessions WHERE id = ?1",
            [&session.id],
            |row| row.get::<_, i32>(0).map(|v| v != 0),
        ).unwrap();
        assert!(is_deleted, "Session should be soft deleted");

        // Verify current page was archived (new page created)
        let page_count_after: i32 = conn.query_row(
            "SELECT COUNT(*) FROM chat_pages WHERE session_id = ?1",
            [&session.id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(page_count_after, 2, "Soft delete should archive current page by creating a new one");
    }

    #[test]
    fn test_reset_session_clears_overflow_index() {
        let conn = init_test_db();
        insert_session(&conn, "sess1", "private");
        insert_private_session(&conn, "sess1", "agent1", 0);
        insert_session_settings(&conn, "sess1", 50);
        
        // Set overflow index to non-zero
        conn.execute(
            "UPDATE session_settings SET last_overflow_summary_index = 100 WHERE session_id = 'sess1'",
            [],
        ).unwrap();
        
        reset_session(&conn, "sess1").unwrap();
        
        let index: i32 = conn.query_row(
            "SELECT last_overflow_summary_index FROM session_settings WHERE session_id = 'sess1'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(index, 0, "reset_session should clear last_overflow_summary_index");
    }
}
