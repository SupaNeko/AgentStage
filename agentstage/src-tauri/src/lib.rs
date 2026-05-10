pub mod commands;
pub mod db;
pub mod models;

use commands::agent::{create_agent, delete_agent, get_agent, list_agents, update_agent};
use db::connection::init_db;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let db_state = init_db(app)?;
            app.manage(db_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            create_agent,
            get_agent,
            list_agents,
            update_agent,
            delete_agent,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
