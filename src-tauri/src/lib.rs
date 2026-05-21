pub mod constants;
pub mod commands;
pub mod crypto;
pub mod db;
pub mod logger;
pub mod llm;
pub mod models;
pub mod scheduler;

use commands::agent::{create_agent, delete_agent, get_agent, list_agents, update_agent, test_api_connection, reset_agent_memory};
use commands::log::log_frontend;
use commands::message::{get_session_messages, send_user_message, send_history_message};
use commands::session::{
    create_group_session, create_private_session, delete_session, clear_session_history, get_group_members,
    get_session, list_sessions, list_history_sessions, get_session_config, update_session_config,
    reset_session, reset_message_count, disband_group, add_group_member, remove_group_member,
    list_chat_pages,
};
use commands::settings::{get_settings, update_settings};
use commands::upload::upload_avatar;
use commands::user_persona::{list_user_personas, create_user_persona, update_user_persona, delete_user_persona, get_current_user_persona, activate_user_persona};
use commands::agent_relationship::{list_agent_relationships, update_agent_relationship, add_friendships, remove_friendship, update_agent_memory};
use commands::generate_persona::generate_persona;
use db::connection::init_db;
use scheduler::Scheduler;
use tauri::Manager;

/// 获取应用数据目录
/// 所有数据（数据库、日志、WebView2）统一放在程序目录下，不在任何用户目录放东西
pub fn get_data_dir() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let exe_path = std::env::current_exe()?;
    let exe_dir = exe_path.parent().ok_or("Failed to get exe directory")?;

    // Debug 构建强制使用项目根目录的 data/，避免 target/debug/data/ 干扰
    #[cfg(debug_assertions)]
    {
        let mut dir = exe_dir.to_path_buf();
        for _ in 0..5 {
            if dir.join("src-tauri").join("Cargo.toml").exists() {
                return Ok(dir.join("data"));
            }
            if let Some(parent) = dir.parent() {
                dir = parent.to_path_buf();
            } else {
                break;
            }
        }
    }

    // Release 构建：exe 同级目录（兼容便携模式）
    let portable = exe_dir.join("data");
    if portable.exists() {
        return Ok(portable);
    }
    Ok(exe_dir.join("data"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = get_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            logger::init(&app_data_dir);

            let db_state = init_db(&app_data_dir)?;
            app.manage(db_state.clone());

            // 手动创建主窗口，强制 WebView2 数据目录到程序目录，避免在 %LOCALAPPDATA% 创建 EBWebView
            let webview_data_dir = app_data_dir.join("webview");
            std::fs::create_dir_all(&webview_data_dir)?;

            let url = if cfg!(debug_assertions) {
                tauri::WebviewUrl::External("http://127.0.0.1:1420".parse().map_err(|_| "Invalid dev URL")?)
            } else {
                tauri::WebviewUrl::App("index.html".into())
            };

            tauri::WebviewWindowBuilder::new(app, "main", url)
                .title("agentstage")
                .inner_size(1200.0, 800.0)
                .min_inner_size(900.0, 600.0)
                .center()
                .additional_browser_args("--remote-debugging-port=9222")
                .data_directory(webview_data_dir)
                .build()?;

            let scheduler = Scheduler::new(db_state);
            scheduler.set_app_handle(app.handle().clone());
            let scheduler_for_bg = scheduler.clone();
            let scheduler_for_recover = scheduler.clone();
            app.manage(scheduler);

            // 恢复 scheduler 状态
            let scheduler_for_timers = scheduler_for_recover.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = scheduler_for_recover.recover_from_db().await {
                    crate::logger::backend("ERROR", &format!("Failed to recover scheduler from db: {}", e));
                }
                scheduler_for_timers.init_proactive_timers().await;
                let scheduler_clone = scheduler_for_timers.clone();
                tauri::async_runtime::spawn(async move {
                    scheduler_clone.start_timer_scan().await;
                });
            });

            // 启动后台扫描任务（在独立线程中运行 Tokio runtime，避免依赖 Tauri 的 runtime 上下文）
            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
                rt.block_on(scheduler_for_bg.start_background_scan());
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            create_agent,
            generate_persona,
            get_agent,
            list_agents,
            update_agent,
            delete_agent,
            reset_agent_memory,
            test_api_connection,
            create_private_session,
            list_sessions,
            list_history_sessions,
            get_session,
            delete_session,
            clear_session_history,
            create_group_session,
            get_group_members,
            get_session_config,
            update_session_config,
            reset_session,
            reset_message_count,
            disband_group,
            add_group_member,
            remove_group_member,
            list_chat_pages,
            send_user_message,
            send_history_message,
            get_session_messages,
            log_frontend,
            get_settings,
            update_settings,
            upload_avatar,
            list_user_personas,
            create_user_persona,
            update_user_persona,
            delete_user_persona,
            get_current_user_persona,
            activate_user_persona,
            list_agent_relationships,
            update_agent_relationship,
            add_friendships,
            remove_friendship,
            update_agent_memory,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
