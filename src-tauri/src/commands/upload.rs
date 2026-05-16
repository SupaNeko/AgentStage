use tauri::State;
use crate::db::connection::{get_db, DbState};
use base64::{Engine as _, engine::general_purpose};
use std::fs;

#[derive(Debug, serde::Deserialize)]
pub struct UploadAvatarRequest {
    pub target_type: String,
    pub target_id: String,
    pub image_data_base64: String,
}

#[tauri::command]
pub async fn upload_avatar(
    state: State<'_, DbState>,
    req: UploadAvatarRequest,
) -> Result<String, String> {
    let conn = get_db(&state).await?;

    let app_dir = crate::get_data_dir()
        .map_err(|e| e.to_string())?
        .join("avatars")
        .join(&req.target_type);

    fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;

    let base64_data = if let Some(idx) = req.image_data_base64.find(',') {
        &req.image_data_base64[idx + 1..]
    } else {
        &req.image_data_base64
    };

    let image_bytes = general_purpose::STANDARD.decode(base64_data).map_err(|e| e.to_string())?;

    let ext = if image_bytes.starts_with(b"\x89PNG") {
        "png"
    } else if image_bytes.starts_with(b"\xff\xd8") {
        "jpg"
    } else {
        "png"
    };

    let filename = format!("{}.{}", req.target_id, ext);
    let filepath = app_dir.join(&filename);
    fs::write(&filepath, image_bytes).map_err(|e| e.to_string())?;

    let relative_path = format!("avatars/{}/{}", req.target_type, filename);
    match req.target_type.as_str() {
        "agent" => {
            conn.execute(
                "UPDATE agents SET avatar_path = ?1 WHERE id = ?2",
                (&relative_path, &req.target_id),
            ).map_err(|e| e.to_string())?;
        }
        "group" => {
            conn.execute(
                "UPDATE group_sessions SET avatar_path = ?1 WHERE session_id = ?2",
                (&relative_path, &req.target_id),
            ).map_err(|e| e.to_string())?;
        }
        "user" => {
            conn.execute(
                "UPDATE user_personas SET avatar_path = ?1 WHERE is_default = 1",
                [&relative_path],
            ).map_err(|e| e.to_string())?;
        }
        _ => return Err("Invalid target_type".to_string()),
    }

    // Return absolute path for frontend immediate display
    let absolute_path = filepath.to_string_lossy().to_string();
    Ok(absolute_path)
}
