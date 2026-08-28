use rusqlite::{Connection, Result};
use crate::models::settings::AppSettings;

pub fn get_or_create_settings(conn: &Connection) -> Result<AppSettings> {
    let result = conn.query_row(
        "SELECT id, global_min_trigger_interval, private_message_limit_default, \
                group_message_limit_default, private_limit_enabled_default, \
                group_limit_enabled_default, theme, font_size, language, \
                enter_to_send, launch_on_startup, minimize_to_tray, \
                active_persona_id, default_avatar_path, quiet_hours_start, quiet_hours_end, summary_model_config_id, \
                search_provider, search_api_key_encrypted, \
                virtual_time_enabled, virtual_time_base, virtual_time_set_at, virtual_time_rate, updated_at \
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
                search_provider: row.get(17).ok(),
                search_api_key_encrypted: row.get(18).ok(),
                virtual_time_enabled: row.get::<_, i32>(19).unwrap_or(0) != 0,
                virtual_time_base: row.get(20).ok(),
                virtual_time_set_at: row.get(21).ok(),
                virtual_time_rate: row.get(22).unwrap_or(1),
                updated_at: row.get(23)?,
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

/// 更新搜索 API 配置。encrypted_key 为 None 时清空已存 Key。
pub fn update_search_config(
    conn: &Connection,
    provider: Option<&str>,
    encrypted_key: Option<&[u8]>,
) -> Result<(), rusqlite::Error> {
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE app_settings SET search_provider = ?1, search_api_key_encrypted = ?2, updated_at = ?3 WHERE id = 1",
        rusqlite::params![provider, encrypted_key, now],
    )?;
    Ok(())
}

/// 更新虚拟时间配置。提供 base 时，set_at 重置为当前真实时间。
pub fn update_virtual_time(
    conn: &Connection,
    enabled: bool,
    base: Option<i64>,
    rate: Option<i32>,
) -> Result<(), rusqlite::Error> {
    let now = chrono::Utc::now().timestamp_millis();
    let current = get_or_create_settings(conn)?;
    let new_rate = rate.unwrap_or(current.virtual_time_rate).max(1);
    match base {
        Some(b) => {
            conn.execute(
                "UPDATE app_settings SET virtual_time_enabled = ?1, virtual_time_base = ?2, virtual_time_set_at = ?3, virtual_time_rate = ?4, updated_at = ?5 WHERE id = 1",
                rusqlite::params![enabled as i32, b, now, new_rate, now],
            )?;
        }
        None => {
            conn.execute(
                "UPDATE app_settings SET virtual_time_enabled = ?1, virtual_time_rate = ?2, updated_at = ?3 WHERE id = 1",
                rusqlite::params![enabled as i32, new_rate, now],
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::settings::UpdateAppSettingsRequest;

    fn init_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::BASE_SCHEMA).unwrap();
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
            ..Default::default()
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

    #[test]
    fn test_search_config_roundtrip_and_clear() {
        let conn = init_test_db();
        let before = get_or_create_settings(&conn).unwrap();
        assert!(before.search_provider.is_none());
        assert!(before.search_api_key_encrypted.is_none());

        update_search_config(&conn, Some("bocha"), Some(b"encrypted-key")).unwrap();
        let s = get_or_create_settings(&conn).unwrap();
        assert_eq!(s.search_provider.as_deref(), Some("bocha"));
        assert_eq!(s.search_api_key_encrypted.as_deref(), Some(b"encrypted-key".as_slice()));

        // 切换厂商时清空 Key
        update_search_config(&conn, Some("zhipu"), None).unwrap();
        let s = get_or_create_settings(&conn).unwrap();
        assert_eq!(s.search_provider.as_deref(), Some("zhipu"));
        assert!(s.search_api_key_encrypted.is_none());
    }

    #[test]
    fn test_settings_response_never_leaks_search_key() {
        let conn = init_test_db();
        get_or_create_settings(&conn).unwrap(); // 确保单例行存在
        update_search_config(&conn, Some("kimi"), Some(b"super-secret")).unwrap();
        let s = get_or_create_settings(&conn).unwrap();
        let resp: crate::models::settings::SettingsResponse = s.into();
        assert!(resp.search_api_key_set);
        // SettingsResponse 不含任何 Key 字段，序列化后也不应出现明文
        let json = serde_json::to_string(&resp).unwrap();
        assert!(!json.contains("super-secret"));
    }

    #[test]
    fn test_virtual_time_update_sets_set_at_and_clamps_rate() {
        let conn = init_test_db();
        let before = get_or_create_settings(&conn).unwrap();
        assert!(!before.virtual_time_enabled);
        assert_eq!(before.virtual_time_rate, 1);

        let base = 1_800_000_000_000i64;
        update_virtual_time(&conn, true, Some(base), Some(0)).unwrap();
        let s = get_or_create_settings(&conn).unwrap();
        assert!(s.virtual_time_enabled);
        assert_eq!(s.virtual_time_base, Some(base));
        assert!(s.virtual_time_set_at.is_some());
        assert_eq!(s.virtual_time_rate, 1); // 0 被钳制为 1

        // 不提供 base 时保留原 base/set_at，仅改开关与流速
        update_virtual_time(&conn, false, None, Some(5)).unwrap();
        let s = get_or_create_settings(&conn).unwrap();
        assert!(!s.virtual_time_enabled);
        assert_eq!(s.virtual_time_base, Some(base));
        assert_eq!(s.virtual_time_rate, 5);
    }
}
