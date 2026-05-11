use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

static LOG_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn init(app_data_dir: &std::path::Path) {
    let log_dir = app_data_dir.join("logs");
    let _ = create_dir_all(&log_dir);
    let mut guard = LOG_DIR.lock().unwrap();
    *guard = Some(log_dir);
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

pub fn backend(level: &str, message: &str) {
    log_to_file("backend.log", level, message);
}

pub fn frontend(level: &str, message: &str) {
    log_to_file("frontend.log", level, message);
}
