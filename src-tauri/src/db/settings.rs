use rusqlite::{Connection, Result};
use crate::models::settings::AppSettings;

pub fn get_or_create_settings(conn: &Connection) -> Result<AppSettings> {
    let result = conn.query_row(
        "SELECT id, global_min_trigger_interval, private_message_limit_default, \
                group_message_limit_default, private_limit_enabled_default, \
                group_limit_enabled_default, theme, font_size, language, \
                enter_to_send, launch_on_startup, minimize_to_tray, \
                active_persona_id, default_avatar_path, quiet_hours_start, quiet_hours_end, summary_model_config_id, updated_at \
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
                active_persona_id: row.get(12).ok(),
                default_avatar_path: row.get(13).ok(),
                quiet_hours_start: row.get(14)?,
                quiet_hours_end: row.get(15)?,
                summary_model_config_id: row.get(16).ok(),
                updated_at: row.get(17)?,
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

pub fn update_settings(conn: &Connection, req: &crate::models::settings::UpdateAppSettingsRequest) -> Result<()> {
    let current = get_or_create_settings(conn)?;
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE app_settings SET 
            global_min_trigger_interval = ?1, private_message_limit_default = ?2,
            group_message_limit_default = ?3, private_limit_enabled_default = ?4,
            group_limit_enabled_default = ?5, theme = ?6, font_size = ?7,
            language = ?8, enter_to_send = ?9, launch_on_startup = ?10,
            minimize_to_tray = ?11, active_persona_id = ?12,
            default_avatar_path = ?13, quiet_hours_start = ?14, quiet_hours_end = ?15, summary_model_config_id = ?16, updated_at = ?17 WHERE id = 1",
        rusqlite::params![
            req.global_min_trigger_interval.unwrap_or(current.global_min_trigger_interval),
            req.private_message_limit_default.unwrap_or(current.private_message_limit_default),
            req.group_message_limit_default.unwrap_or(current.group_message_limit_default),
            req.private_limit_enabled_default.unwrap_or(current.private_limit_enabled_default) as i32,
            req.group_limit_enabled_default.unwrap_or(current.group_limit_enabled_default) as i32,
            req.theme.as_deref().unwrap_or(&current.theme),
            req.font_size.as_deref().unwrap_or(&current.font_size),
            req.language.as_deref().unwrap_or(&current.language),
            req.enter_to_send.unwrap_or(current.enter_to_send) as i32,
            req.launch_on_startup.unwrap_or(current.launch_on_startup) as i32,
            req.minimize_to_tray.unwrap_or(current.minimize_to_tray) as i32,
            req.active_persona_id.as_deref().or(current.active_persona_id.as_deref()),
            req.default_avatar_path.as_deref().or(current.default_avatar_path.as_deref()),
            current.quiet_hours_start,
            current.quiet_hours_end,
            req.summary_model_config_id.as_deref().or(current.summary_model_config_id.as_deref()),
            now,
        ],
    )?;
    Ok(())
}

pub fn update_quiet_hours(
    conn: &Connection,
    start: i32,
    end: i32,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE app_settings SET quiet_hours_start = ?1, quiet_hours_end = ?2 WHERE id = 1",
        rusqlite::params![start, end],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::settings::UpdateAppSettingsRequest;

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
        conn.execute_batch(crate::db::schema::MIGRATION_V20).unwrap();
        conn
    }

    #[test]
    fn test_update_settings_preserve_untouched_fields() {
        let conn = init_test_db();
        let before = get_or_create_settings(&conn).unwrap();
        assert_eq!(before.theme, "default");
        assert_eq!(before.font_size, "medium");

        let req = UpdateAppSettingsRequest {
            global_min_trigger_interval: Some(60),
            private_message_limit_default: None,
            group_message_limit_default: None,
            private_limit_enabled_default: None,
            group_limit_enabled_default: None,
            theme: None,
            font_size: None,
            language: None,
            enter_to_send: None,
            launch_on_startup: None,
            minimize_to_tray: None,
            active_persona_id: None,
            default_avatar_path: None,
            summary_model_config_id: None,
        };
        update_settings(&conn, &req).unwrap();

        let after = get_or_create_settings(&conn).unwrap();
        assert_eq!(after.global_min_trigger_interval, 60);
        assert_eq!(after.theme, "default");
        assert_eq!(after.font_size, "medium");
    }

    #[test]
    fn test_update_quiet_hours() {
        let conn = init_test_db();
        let before = get_or_create_settings(&conn).unwrap();
        assert_eq!(before.quiet_hours_start, 0);
        assert_eq!(before.quiet_hours_end, 480);

        update_quiet_hours(&conn, 120, 360).unwrap();

        let after = get_or_create_settings(&conn).unwrap();
        assert_eq!(after.quiet_hours_start, 120);
        assert_eq!(after.quiet_hours_end, 360);
    }
}
