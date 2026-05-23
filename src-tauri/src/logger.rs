use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static LOG_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);
static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn init(app_data_dir: &std::path::Path) {
    let log_dir = app_data_dir.join("logs");
    let _ = create_dir_all(&log_dir);
    let mut guard = LOG_DIR.lock().unwrap();
    *guard = Some(log_dir);

    // Priority: env var > debug_assertions
    let enabled = std::env::var("AGENTSTAGE_DEBUG_LOG").is_ok() || cfg!(debug_assertions);
    DEBUG_ENABLED.store(enabled, Ordering::Relaxed);

    info(&format!(
        "[Logger] initialized | debug_enabled={} (reason={})",
        enabled,
        if std::env::var("AGENTSTAGE_DEBUG_LOG").is_ok() {
            "env AGENTSTAGE_DEBUG_LOG"
        } else if cfg!(debug_assertions) {
            "debug_assertions"
        } else {
            "default"
        }
    ));
}

fn log_to_file(filename: &str, level: &str, message: &str) {
    let log_dir = LOG_DIR.lock().unwrap();
    if let Some(dir) = log_dir.as_ref() {
        let path = dir.join(filename);
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            let _ = writeln!(file, "[{}] [{}] {}", timestamp, level, message);
        }
    }
}

/// Low-level method, writes directly to backend.log with specified level.
/// New code should prefer using info/debug/error/warn methods.
pub fn backend(level: &str, message: &str) {
    log_to_file("backend.log", level, message);
}

pub fn frontend(level: &str, message: &str) {
    log_to_file("frontend.log", level, message);
}

/// DEBUG level: writes to backend-debug.log only when DEBUG_ENABLED is true.
/// Use for detailed content (full prompts, HTTP bodies, message content, etc.)
pub fn debug(message: &str) {
    if DEBUG_ENABLED.load(Ordering::Relaxed) {
        log_to_file("backend-debug.log", "DEBUG", message);
    }
}

/// INFO level: writes to backend.log.
/// Use for business process records (agent triggered, timer expired, message sent, etc.)
pub fn info(message: &str) {
    log_to_file("backend.log", "INFO", message);
}

/// WARN level: writes to backend.log.
pub fn warn(message: &str) {
    log_to_file("backend.log", "WARN", message);
}

/// ERROR level: writes to backend.log.
pub fn error(message: &str) {
    log_to_file("backend.log", "ERROR", message);
}
