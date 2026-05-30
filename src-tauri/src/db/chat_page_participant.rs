use rusqlite::{Connection, OptionalExtension, params};

#[derive(Debug, Clone)]
pub struct ChatPageParticipant {
    pub chat_page_id: String,
    pub participant_id: String,
    pub participant_type: String,
    pub participant_name: String,
    pub participant_avatar: Option<String>,
    pub participant_simplified_persona: Option<String>,
}

pub fn insert_snapshot(
    conn: &Connection,
    chat_page_id: &str,
    participant_id: &str,
    participant_type: &str,
    participant_name: &str,
    participant_avatar: Option<&str>,
    participant_simplified_persona: Option<&str>,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO chat_page_participants (chat_page_id, participant_id, participant_type, participant_name, participant_avatar, participant_simplified_persona)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![chat_page_id, participant_id, participant_type, participant_name, participant_avatar, participant_simplified_persona],
    )?;
    Ok(())
}

pub fn list_by_chat_page(
    conn: &Connection,
    chat_page_id: &str,
) -> Result<Vec<ChatPageParticipant>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT chat_page_id, participant_id, participant_type, participant_name, participant_avatar, participant_simplified_persona
         FROM chat_page_participants
         WHERE chat_page_id = ?1"
    )?;
    let rows = stmt.query_map([chat_page_id], |row| {
        Ok(ChatPageParticipant {
            chat_page_id: row.get(0)?,
            participant_id: row.get(1)?,
            participant_type: row.get(2)?,
            participant_name: row.get(3)?,
            participant_avatar: row.get(4)?,
            participant_simplified_persona: row.get(5)?,
        })
    })?;
    rows.collect()
}

pub fn get_chat_page_id(
    conn: &Connection,
    session_id: &str,
    page_index: i32,
) -> Result<Option<String>, rusqlite::Error> {
    conn.query_row(
        "SELECT id FROM chat_pages WHERE session_id = ?1 AND page_index = ?2",
        params![session_id, page_index],
        |row| row.get(0),
    ).optional()
}
