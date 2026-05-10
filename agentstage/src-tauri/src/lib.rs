pub mod commands;
pub mod crypto;
pub mod db;
pub mod models;

use commands::agent::{create_agent, delete_agent, get_agent, list_agents, update_agent};
use commands::message::{get_session_messages, send_user_message};
use commands::session::{create_private_session, delete_session, get_session, list_sessions};
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
            create_private_session,
            list_sessions,
            get_session,
            delete_session,
            send_user_message,
            get_session_messages,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
