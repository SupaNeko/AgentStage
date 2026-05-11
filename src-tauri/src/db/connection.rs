use rusqlite::Connection;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct DbState(pub Arc<Mutex<Connection>>);

pub fn init_db(app_data_dir: &std::path::Path) -> Result<DbState, Box<dyn std::error::Error>> {
    let db_path = app_data_dir.join("agentstage.db");
    let mut conn = Connection::open(&db_path)?;

    conn.execute_batch("PRAGMA journal_mode = WAL;")?;

    let journal_mode: String = conn.query_row(
        "PRAGMA journal_mode",
        [],
        |row| row.get(0),
    )?;
    if journal_mode.to_lowercase() != "wal" {
        return Err(format!("Failed to enable WAL mode, got: {}", journal_mode).into());
    }

    conn.execute("PRAGMA foreign_keys = ON;", [])?;
    conn.execute("PRAGMA synchronous = NORMAL;", [])?;

    super::migration::run_migrations(&mut conn)?;

    // 启动时强制重置所有 is_triggering 标志（防止上次 panic 导致的死锁残留）
    let _ = conn.execute("UPDATE trigger_states SET is_triggering = 0", []);

    Ok(DbState(Arc::new(Mutex::new(conn))))
}

pub async fn get_db<'a>(state: &'a tauri::State<'a, DbState>) -> Result<tokio::sync::MutexGuard<'a, Connection>, String> {
    Ok(state.0.lock().await)
}

// 注意：集成测试移至 scheduler/mod.rs，避免 Windows cdylib 测试二进制入口点问题
// #[cfg(test)]
// mod tests { ... }
