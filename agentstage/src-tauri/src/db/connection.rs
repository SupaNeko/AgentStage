use rusqlite::Connection;
use tokio::sync::Mutex;
use tauri::Manager;

pub struct DbState(pub Mutex<Connection>);

pub fn init_db(app: &tauri::App) -> Result<DbState, Box<dyn std::error::Error>> {
    let app_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_dir)?;

    let db_path = app_dir.join("agentstage.db");
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

    Ok(DbState(Mutex::new(conn)))
}

pub async fn get_db<'a>(state: &'a tauri::State<'a, DbState>) -> Result<tokio::sync::MutexGuard<'a, Connection>, String> {
    Ok(state.0.lock().await)
}
