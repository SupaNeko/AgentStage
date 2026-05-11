use rusqlite::{Connection, Result};
use crate::models::settings::AppSettings;

pub fn get_or_create_settings(conn: &Connection) -> Result<AppSettings> {
    let result = conn.query_row(
        "SELECT id, global_min_trigger_interval, private_message_limit_default, \
                group_message_limit_default, private_limit_enabled_default, \
                group_limit_enabled_default, theme, font_size, language, \
                enter_to_send, launch_on_startup, minimize_to_tray, updated_at \
         FROM app_settings WHERE id = 1",
        [],
        |row| {
            Ok(AppSettings {
                id: row.get(0)?,
                global_min_trigger_interval: row.get(1)?,
                private_message_limit_default: row.get(2)?,
                group_message_limit_default: row.get(3)?,
                private_limit_enabled_default: row.get::<_, i32>(4)? != 0,
                group_limit_enabled_default: row.get::<_, i32>(5)? != 0,
                theme: row.get(6)?,
                font_size: row.get(7)?,
                language: row.get(8)?,
                enter_to_send: row.get::<_, i32>(9)? != 0,
                launch_on_startup: row.get::<_, i32>(10)? != 0,
                minimize_to_tray: row.get::<_, i32>(11)? != 0,
                updated_at: row.get(12)?,
            })
        },
    );

    match result {
        Ok(settings) => Ok(settings),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            let now = chrono::Utc::now().timestamp_millis();
            conn.execute(
                "INSERT INTO app_settings (id, updated_at) VALUES (1, ?1)",
                [now],
            )?;
            get_or_create_settings(conn)
        }
        Err(e) => Err(e),
    }
}
