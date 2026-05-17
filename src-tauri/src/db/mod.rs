pub mod agent;
pub mod agent_relationship;
pub mod agent_unread;
pub mod chat_page;
pub mod connection;
pub mod frozen_state;
pub mod migration;
pub mod schema;
pub mod session;
pub mod message;
pub mod settings;
pub mod trigger_state;
pub mod user_persona;

/// Resolve a relative avatar path to an absolute path using the data directory
pub fn resolve_avatar_path(relative_path: Option<String>) -> Option<String> {
    let relative = relative_path?;
    let data_dir = crate::get_data_dir().ok()?;
    let absolute = data_dir.join(&relative);
    let result = absolute.to_string_lossy().to_string();
    println!(
        "[Avatar] resolve_avatar_path: relative='{}' data_dir='{}' absolute='{}' exists={}",
        relative,
        data_dir.display(),
        result,
        absolute.exists()
    );
    Some(result)
}
