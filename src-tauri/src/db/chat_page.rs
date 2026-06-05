use rusqlite::{Connection, Result};
use crate::models::chat_page::ChatPage;

pub fn list_chat_pages(conn: &Connection, session_id: &str) -> Result<Vec<ChatPage>> {
    let mut stmt = conn.prepare(
        "SELECT 
            cp.id, cp.session_id, cp.page_index, cp.name, cp.is_active, cp.created_at,
            COALESCE(msg_stats.msg_count, 0) as message_count,
            COALESCE(msg_stats.last_msg_at, cp.created_at) as updated_at
        FROM chat_pages cp
        LEFT JOIN (
            SELECT session_id, page_index, COUNT(*) as msg_count, MAX(created_at) as last_msg_at
            FROM messages
            WHERE is_deleted = 0
            GROUP BY session_id, page_index
        ) msg_stats ON cp.session_id = msg_stats.session_id AND cp.page_index = msg_stats.page_index
        WHERE cp.session_id = ?1
          AND cp.page_index <= (
              SELECT COALESCE(current_chat_page, 0) FROM private_sessions WHERE session_id = ?1
              UNION ALL
              SELECT COALESCE(current_chat_page, 0) FROM group_sessions WHERE session_id = ?1
              LIMIT 1
          )
        ORDER BY cp.page_index DESC"
    )?;
    
    let rows = stmt.query_map([session_id], |row| {
        Ok(ChatPage {
            id: row.get(0)?,
            session_id: row.get(1)?,
            page_index: row.get(2)?,
            name: row.get(3)?,
            is_active: row.get::<_, i32>(4)? != 0,
            created_at: row.get(5)?,
            message_count: row.get(6)?,
            updated_at: row.get(7)?,
        })
    })?;
    
    rows.collect()
}

pub fn update_name(conn: &Connection, session_id: &str, page_index: i32, name: &str) -> Result<()> {
    conn.execute(
        "UPDATE chat_pages SET name = ?1 WHERE session_id = ?2 AND page_index = ?3",
        rusqlite::params![name, session_id, page_index],
    )?;
    Ok(())
}
