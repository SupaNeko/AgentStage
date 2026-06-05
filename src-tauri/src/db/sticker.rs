use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::models::sticker::{ResolvedStickerResponse, StickerPackResponse, StickerResponse};

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

pub fn validate_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("名称不能为空".to_string());
    }
    if trimmed.contains('_') {
        return Err("名称不能包含下划线".to_string());
    }
    Ok(trimmed.to_string())
}

pub fn next_available_name<F>(base: &str, exists: F) -> String
where
    F: Fn(&str) -> bool,
{
    if !exists(base) {
        return base.to_string();
    }
    let mut idx = 1;
    loop {
        let candidate = format!("{}{}", base, idx);
        if !exists(&candidate) {
            return candidate;
        }
        idx += 1;
    }
}

pub fn create_pack(conn: &Connection, name: &str) -> Result<StickerPackResponse, String> {
    let name = validate_name(name)?;
    let id = Uuid::new_v4().to_string();
    let now = now_ms();
    conn.execute(
        "INSERT INTO sticker_packs (id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        params![id, name, now],
    ).map_err(|e| e.to_string())?;
    get_pack(conn, &id)?.ok_or_else(|| "创建表情包后读取失败".to_string())
}

pub fn get_pack(conn: &Connection, id: &str) -> Result<Option<StickerPackResponse>, String> {
    let row = conn.query_row(
        "SELECT id, name, created_at, updated_at FROM sticker_packs WHERE id = ?1 AND is_deleted = 0",
        [id],
        |row| Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
        )),
    ).optional().map_err(|e| e.to_string())?;

    let Some((id, name, created_at, updated_at)) = row else {
        return Ok(None);
    };
    let stickers = list_stickers_by_pack(conn, &id)?;
    Ok(Some(StickerPackResponse { id, name, stickers, created_at, updated_at }))
}

pub fn list_packs(conn: &Connection) -> Result<Vec<StickerPackResponse>, String> {
    let mut stmt = conn.prepare(
        "SELECT id, name, created_at, updated_at FROM sticker_packs WHERE is_deleted = 0 ORDER BY updated_at DESC"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| Ok((
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, i64>(2)?,
        row.get::<_, i64>(3)?,
    ))).map_err(|e| e.to_string())?;

    let mut packs = Vec::new();
    for row in rows {
        let (id, name, created_at, updated_at) = row.map_err(|e| e.to_string())?;
        let stickers = list_stickers_by_pack(conn, &id)?;
        packs.push(StickerPackResponse { id, name, stickers, created_at, updated_at });
    }
    Ok(packs)
}

pub fn insert_sticker_metadata(
    conn: &Connection,
    pack_id: &str,
    name: &str,
    file_path: &str,
    mime_type: &str,
    width: i32,
    height: i32,
    file_size: i64,
) -> Result<StickerResponse, String> {
    let name = validate_name(name)?;
    let pack_exists: bool = conn.query_row(
        "SELECT 1 FROM sticker_packs WHERE id = ?1 AND is_deleted = 0",
        [pack_id],
        |_| Ok(true),
    ).unwrap_or(false);
    if !pack_exists {
        return Err("表情包不存在或已删除".to_string());
    }
    let id = Uuid::new_v4().to_string();
    let now = now_ms();
    conn.execute(
        "INSERT INTO stickers (id, pack_id, name, file_path, mime_type, width, height, file_size, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
        params![id, pack_id, name, file_path, mime_type, width, height, file_size, now],
    ).map_err(|e| e.to_string())?;
    get_sticker(conn, &id)?.ok_or_else(|| "创建表情后读取失败".to_string())
}

pub fn get_sticker(conn: &Connection, id: &str) -> Result<Option<StickerResponse>, String> {
    let row = conn.query_row(
        "SELECT id, pack_id, name, file_path, mime_type, width, height, file_size, created_at, updated_at
         FROM stickers WHERE id = ?1 AND is_deleted = 0",
        [id],
        |row| Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, i32>(5)?,
            row.get::<_, i32>(6)?,
            row.get::<_, i64>(7)?,
            row.get::<_, i64>(8)?,
            row.get::<_, i64>(9)?,
        )),
    ).optional().map_err(|e| e.to_string())?;

    let Some((id, pack_id, name, file_path, mime_type, width, height, file_size, created_at, updated_at)) = row else {
        return Ok(None);
    };
    Ok(Some(StickerResponse { id, pack_id, name, file_path, mime_type, width, height, file_size, created_at, updated_at }))
}

pub fn list_stickers_by_pack(conn: &Connection, pack_id: &str) -> Result<Vec<StickerResponse>, String> {
    let mut stmt = conn.prepare(
        "SELECT id, pack_id, name, file_path, mime_type, width, height, file_size, created_at, updated_at
         FROM stickers WHERE pack_id = ?1 AND is_deleted = 0 ORDER BY created_at ASC"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([pack_id], |row| Ok((
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, String>(2)?,
        row.get::<_, String>(3)?,
        row.get::<_, String>(4)?,
        row.get::<_, i32>(5)?,
        row.get::<_, i32>(6)?,
        row.get::<_, i64>(7)?,
        row.get::<_, i64>(8)?,
        row.get::<_, i64>(9)?,
    ))).map_err(|e| e.to_string())?;

    let mut stickers = Vec::new();
    for row in rows {
        let (id, pack_id, name, file_path, mime_type, width, height, file_size, created_at, updated_at) = row.map_err(|e| e.to_string())?;
        stickers.push(StickerResponse { id, pack_id, name, file_path, mime_type, width, height, file_size, created_at, updated_at });
    }
    Ok(stickers)
}

pub fn update_pack_name(conn: &Connection, id: &str, name: &str) -> Result<StickerPackResponse, String> {
    let name = validate_name(name)?;
    let now = now_ms();
    conn.execute(
        "UPDATE sticker_packs SET name = ?1, updated_at = ?2 WHERE id = ?3 AND is_deleted = 0",
        params![name, now, id],
    ).map_err(|e| e.to_string())?;
    get_pack(conn, id)?.ok_or_else(|| "表情包不存在或已删除".to_string())
}

pub fn delete_pack(conn: &mut Connection, id: &str) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let now = now_ms();
    tx.execute(
        "UPDATE sticker_packs SET is_deleted = 1, deleted_at = ?1 WHERE id = ?2 AND is_deleted = 0",
        params![now, id],
    ).map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE stickers SET is_deleted = 1, deleted_at = ?1 WHERE pack_id = ?2 AND is_deleted = 0",
        params![now, id],
    ).map_err(|e| e.to_string())?;
    tx.execute(
        "DELETE FROM agent_sticker_packs WHERE pack_id = ?1",
        [id],
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

pub fn update_sticker_name(conn: &Connection, id: &str, name: &str) -> Result<StickerResponse, String> {
    let name = validate_name(name)?;
    let now = now_ms();
    conn.execute(
        "UPDATE stickers SET name = ?1, updated_at = ?2 WHERE id = ?3 AND is_deleted = 0",
        params![name, now, id],
    ).map_err(|e| e.to_string())?;
    get_sticker(conn, id)?.ok_or_else(|| "表情不存在或已删除".to_string())
}

pub fn delete_stickers(conn: &mut Connection, ids: &[String]) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let now = now_ms();
    for id in ids {
        tx.execute(
            "UPDATE stickers SET is_deleted = 1, deleted_at = ?1 WHERE id = ?2 AND is_deleted = 0",
            params![now, id],
        ).map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}

pub fn list_agent_pack_ids(conn: &Connection, agent_id: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn.prepare(
        "SELECT pack_id FROM agent_sticker_packs WHERE agent_id = ?1 ORDER BY created_at ASC"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([agent_id], |row| {
        row.get::<_, String>(0)
    }).map_err(|e| e.to_string())?;

    let mut ids = Vec::new();
    for row in rows {
        ids.push(row.map_err(|e| e.to_string())?);
    }
    Ok(ids)
}

pub fn set_agent_pack_ids(conn: &mut Connection, agent_id: &str, pack_ids: &[String]) -> Result<(), String> {
    for pack_id in pack_ids {
        let valid: bool = conn.query_row(
            "SELECT 1 FROM sticker_packs WHERE id = ?1 AND is_deleted = 0",
            [pack_id],
            |_| Ok(true),
        ).unwrap_or(false);
        if !valid {
            return Err("表情包不存在或已删除".to_string());
        }
    }

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "DELETE FROM agent_sticker_packs WHERE agent_id = ?1",
        [agent_id],
    ).map_err(|e| e.to_string())?;
    let now = now_ms();
    for pack_id in pack_ids {
        tx.execute(
            "INSERT INTO agent_sticker_packs (agent_id, pack_id, created_at) VALUES (?1, ?2, ?3)",
            params![agent_id, pack_id, now],
        ).map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}

pub fn resolve_refs(conn: &Connection, refs: &[String]) -> Result<Vec<ResolvedStickerResponse>, String> {
    let packs = list_packs(conn)?;
    let mut result = Vec::new();

    for reference in refs {
        let pieces: Vec<&str> = reference.split('_').collect();
        if pieces.len() != 2 || pieces[0].is_empty() || pieces[1].is_empty() {
            result.push(ResolvedStickerResponse {
                reference: reference.clone(),
                status: "invalid".to_string(),
                pack_id: None,
                sticker_id: None,
                file_path: None,
                mime_type: None,
                width: None,
                height: None,
            });
            continue;
        }

        let pack_name = pieces[0];
        let sticker_name = pieces[1];

        let mut found = None;
        for pack in &packs {
            if pack.name == pack_name {
                for sticker in &pack.stickers {
                    if sticker.name == sticker_name {
                        found = Some((pack.id.clone(), sticker.clone()));
                        break;
                    }
                }
                break;
            }
        }

        if let Some((pack_id, sticker)) = found {
            result.push(ResolvedStickerResponse {
                reference: reference.clone(),
                status: "valid".to_string(),
                pack_id: Some(pack_id),
                sticker_id: Some(sticker.id),
                file_path: Some(sticker.file_path),
                mime_type: Some(sticker.mime_type),
                width: Some(sticker.width),
                height: Some(sticker.height),
            });
        } else {
            result.push(ResolvedStickerResponse {
                reference: reference.clone(),
                status: "invalid".to_string(),
                pack_id: None,
                sticker_id: None,
                file_path: None,
                mime_type: None,
                width: None,
                height: None,
            });
        }
    }

    Ok(result)
}

pub fn list_prompt_stickers(conn: &Connection, agent_id: &str) -> Result<Vec<StickerPackResponse>, String> {
    let pack_ids = list_agent_pack_ids(conn, agent_id)?;
    let mut packs = Vec::new();
    for pack_id in pack_ids {
        if let Some(pack) = get_pack(conn, &pack_id)? {
            if !pack.stickers.is_empty() {
                packs.push(pack);
            }
        }
    }
    Ok(packs)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    fn init() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::BASE_SCHEMA).unwrap();
        conn
    }

    #[test]
    fn create_pack_rejects_underscore() {
        let conn = init();
        let err = super::create_pack(&conn, "猫_pack").unwrap_err();
        assert!(err.contains("不能包含下划线"));
    }

    #[test]
    fn resolve_deleted_sticker_is_invalid() {
        let mut conn = init();
        let pack = super::create_pack(&conn, "猫").unwrap();
        let sticker = super::insert_sticker_metadata(
            &conn,
            &pack.id,
            "可爱",
            "stickers/p/s.png",
            "image/png",
            128,
            128,
            100,
        ).unwrap();
        let valid = super::resolve_refs(&conn, &[String::from("猫_可爱")]).unwrap();
        assert_eq!(valid[0].status, "valid");
        super::delete_stickers(&mut conn, &[sticker.id]).unwrap();
        let invalid = super::resolve_refs(&conn, &[String::from("猫_可爱")]).unwrap();
        assert_eq!(invalid[0].status, "invalid");
    }

    #[test]
    fn set_agent_packs_is_idempotent() {
        let mut conn = init();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES ('a1', 'A', '', '', 0, 0)",
            [],
        ).unwrap();
        let pack = super::create_pack(&conn, "猫").unwrap();
        super::set_agent_pack_ids(&mut conn, "a1", &[pack.id.clone()]).unwrap();
        super::set_agent_pack_ids(&mut conn, "a1", &[pack.id.clone()]).unwrap();
        let ids = super::list_agent_pack_ids(&conn, "a1").unwrap();
        assert_eq!(ids, vec![pack.id]);
    }
}
