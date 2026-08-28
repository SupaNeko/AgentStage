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
    Migration {
        version: 18,
        name: "fix_app_settings_corruption",
        sql: super::schema::MIGRATION_V18,
    },
    Migration {
        version: 19,
        name: "global_model_config_refactor",
        sql: super::schema::MIGRATION_V19,
    },
    Migration {
        version: 20,
        name: "session_page_title_summary",
        sql: super::schema::MIGRATION_V20,
    },
    Migration {
        version: 21,
        name: "llm_usage_tracking",
        sql: super::schema::MIGRATION_V21,
    },
    Migration {
        version: 22,
        name: "chat_page_participant_snapshots",
        sql: super::schema::MIGRATION_V22,
    },
    Migration {
        version: 23,
        name: "sticker packs",
        sql: super::schema::MIGRATION_V23,
    },
    Migration {
        version: 24,
        name: "friendships_unique_index",
        sql: super::schema::MIGRATION_V24,
    },
    Migration {
        version: 25,
        name: "vits_voice",
        sql: super::schema::MIGRATION_V25,
    },
    Migration {
        version: 26,
        name: "search_api_and_virtual_time",
        sql: super::schema::MIGRATION_V26,
    },
];

pub fn run_migrations(conn: &mut Connection) -> Result<(), Box<dyn std::error::Error>> {
    conn.execute(super::schema::CREATE_MIGRATIONS_TABLE, [])?;

    let applied_count: i32 = conn.query_row(
        "SELECT COUNT(*) FROM migrations",
        [],
        |row| row.get(0),
    )?;

    if applied_count == 0 {
        // ========== 全新数据库：快速路径 ==========
        // 直接执行完整最新 schema，无需逐个 ALTER TABLE
        conn.execute_batch(super::schema::BASE_SCHEMA)?;

        // 批量标记所有迁移已应用
        let now = chrono::Utc::now().timestamp_millis();
        let tx = conn.transaction()?;
        for migration in MIGRATIONS {
            tx.execute(
                "INSERT INTO migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                (migration.version, migration.name, now),
            )?;
        }
        tx.commit()?;
        return Ok(());
    }

    // ========== 旧数据库：标准增量路径 ==========
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

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    /// 模拟 V24 时期旧库中的 llm_usage_records（CHECK 不含 tts_translate）
    const OLD_USAGE_TABLE: &str = r#"
CREATE TABLE llm_usage_records (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    model_config_id TEXT NOT NULL,
    session_id TEXT,
    trigger_type TEXT NOT NULL
        CHECK(trigger_type IN (
            'user_message',
            'background_scan',
            'timer',
            'proactive',
            'persona_generation'
        )),
    call_round INTEGER NOT NULL DEFAULT 1,
    prompt_tokens INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    message_id TEXT,
    created_at INTEGER NOT NULL,

    FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE CASCADE,
    FOREIGN KEY (model_config_id) REFERENCES model_configs(id) ON DELETE CASCADE,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE SET NULL
);
"#;

    /// 验证 V25 增量迁移：重建 usage 表、保留历史数据、放宽 CHECK、建立新表
    #[test]
    fn test_migration_v25_upgrades_old_usage_table() {
        let mut conn = Connection::open_in_memory().unwrap();
        // 旧库不存在 agents 等父表，测试聚焦迁移 SQL 本身
        conn.execute_batch("PRAGMA foreign_keys = OFF;").unwrap();
        conn.execute_batch(OLD_USAGE_TABLE).unwrap();
        conn.execute(
            "INSERT INTO llm_usage_records (id, agent_id, model_config_id, trigger_type, created_at) VALUES ('r1', 'a1', 'm1', 'user_message', 1000)",
            [],
        )
        .unwrap();
        // 旧 CHECK 下 tts_translate 应被拒绝
        let rejected = conn.execute(
            "INSERT INTO llm_usage_records (id, agent_id, model_config_id, trigger_type, created_at) VALUES ('rx', 'a1', 'm1', 'tts_translate', 1001)",
            [],
        );
        assert!(rejected.is_err());

        conn.execute_batch(crate::db::schema::MIGRATION_V25).unwrap();

        // 历史数据保留
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM llm_usage_records", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);

        // 新 CHECK 接受 tts_translate
        conn.execute(
            "INSERT INTO llm_usage_records (id, agent_id, model_config_id, trigger_type, created_at) VALUES ('r2', 'a1', 'm1', 'tts_translate', 1002)",
            [],
        )
        .unwrap();

        // 新表已建立
        conn.execute(
            "INSERT INTO agent_voices (id, agent_id, model_name, model_path, target_language, generation_mode, created_at, updated_at) VALUES ('v1', 'a1', 'm', 'p', 'ja', 'manual', 1, 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO vits_cache (id, message_id, session_id, agent_id, file_path, created_at) VALUES ('c1', 'msg1', 's1', 'a1', 'p', 1)",
            [],
        )
        .unwrap();

    }
}
