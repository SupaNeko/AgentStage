use serde::Serialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
pub struct ThemeInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub tags: Vec<String>,
    pub preview_path: String,
    pub source: String,
}

#[derive(Debug, serde::Deserialize)]
struct ThemeJson {
    name: String,
    version: String,
    author: String,
    description: String,
    tags: Vec<String>,
    preview: String,
}

fn get_themes_dir() -> Result<PathBuf, String> {
    crate::get_data_dir()
        .map(|d| d.join("themes"))
        .map_err(|e| e.to_string())
}

fn is_valid_theme_id(theme_id: &str) -> bool {
    !theme_id.is_empty()
        && !theme_id.contains("..")
        && !theme_id.contains('/')
        && !theme_id.contains('\\')
}

#[tauri::command]
pub async fn list_themes() -> Result<Vec<ThemeInfo>, String> {
    let themes_dir = get_themes_dir()?;
    let mut themes = Vec::new();

    if !themes_dir.exists() {
        return Ok(themes);
    }

    let entries = fs::read_dir(&themes_dir).map_err(|e| e.to_string())?;

    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let theme_id = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_default();

        if theme_id == "user" || theme_id.is_empty() {
            continue;
        }

        let theme_json_path = path.join("theme.json");
        if !theme_json_path.exists() {
            continue;
        }

        let content = fs::read_to_string(&theme_json_path).map_err(|e| e.to_string())?;
        let theme_json: ThemeJson = serde_json::from_str(&content).map_err(|e| e.to_string())?;

        themes.push(ThemeInfo {
            id: theme_id.clone(),
            name: theme_json.name,
            version: theme_json.version,
            author: theme_json.author,
            description: theme_json.description,
            tags: theme_json.tags,
            preview_path: format!("themes/{}/{}", theme_id, theme_json.preview),
            source: "builtin".to_string(),
        });
    }

    Ok(themes)
}

#[tauri::command]
pub async fn read_theme_css(theme_id: String) -> Result<String, String> {
    if !is_valid_theme_id(&theme_id) {
        return Err("Invalid theme ID".to_string());
    }

    let themes_dir = get_themes_dir()?;
    let css_path = themes_dir.join(&theme_id).join("style.css");

    if !css_path.exists() {
        return Err("Theme CSS not found".to_string());
    }

    let css = fs::read_to_string(&css_path).map_err(|e| e.to_string())?;
    Ok(css)
}

pub fn ensure_themes_initialized() -> Result<(), String> {
    let themes_dir = get_themes_dir()?;
    fs::create_dir_all(&themes_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(themes_dir.join("user")).map_err(|e| e.to_string())?;
    Ok(())
}
