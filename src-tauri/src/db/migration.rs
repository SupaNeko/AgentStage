use rusqlite::Connection;
use std::collections::HashSet;

pub struct Migration {
    pub version: i32,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial_schema",
        sql: super::schema::MIGRATION_V1,
    },
    Migration {
        version: 2,
        name: "friendships_support_user",
        sql: super::schema::MIGRATION_V2,
    },
    Migration {
        version: 3,
        name: "message_system_v2",
        sql: super::schema::MIGRATION_V3,
    },
    Migration {
        version: 4,
        name: "session_config_panel",
        sql: super::schema::MIGRATION_V4,
    },
    Migration {
        version: 5,
        name: "group_sessions_current_chat_page",
        sql: super::schema::MIGRATION_V5,
    },
    Migration {
        version: 6,
        name: "session_inbox_frozen_and_unread",
        sql: super::schema::MIGRATION_V6,
    },
    Migration {
        version: 7,
        name: "private_session_symmetric",
        sql: super::schema::MIGRATION_V7,
    },
    Migration {
        version: 8,
        name: "agent_thinking_mode",
        sql: super::schema::MIGRATION_V8,
    },
    Migration {
        version: 9,
        name: "group_session_dissolve",
        sql: super::schema::MIGRATION_V9,
    },
    Migration {
        version: 11,
        name: "user_persona_config",
        sql: super::schema::MIGRATION_V11,
    },
    Migration {
        version: 12,
        name: "agent_relationships",
        sql: super::schema::MIGRATION_V12,
    },
    Migration {
        version: 13,
        name: "memory_system_base",
        sql: super::schema::MIGRATION_V13,
    },
    Migration {
        version: 14,
        name: "overflow_summary_fields",
        sql: super::schema::MIGRATION_V14,
    },
    Migration {
        version: 15,
        name: "timer_and_proactive_session",
        sql: super::schema::MIGRATION_V15,
    },
    Migration {
        version: 16,
        name: "drop_last_message_preview",
        sql: super::schema::MIGRATION_V16,
    },
    Migration {
        version: 17,
        name: "remove_theme_check_constraint",
        sql: super::schema::MIGRATION_V17,
    },
];

pub fn run_migrations(conn: &mut Connection) -> Result<(), Box<dyn std::error::Error>> {
    conn.execute(super::schema::CREATE_MIGRATIONS_TABLE, [])?;

    let applied_versions: HashSet<i32> = {
        let mut stmt = conn.prepare("SELECT version FROM migrations")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.filter_map(|r| r.ok()).collect()
    };

    for migration in MIGRATIONS {
        if !applied_versions.contains(&migration.version) {
            let tx = conn.transaction()?;
            tx.execute_batch(migration.sql)?;
            tx.execute(
                "INSERT INTO migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                (migration.version, migration.name, chrono::Utc::now().timestamp_millis()),
            )?;
            tx.commit()?;
        }
    }

    Ok(())
}
