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
