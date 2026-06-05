use crate::get_data_dir;

#[tauri::command]
pub async fn get_data_dir_cmd() -> Result<String, String> {
    let path = get_data_dir().map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}
