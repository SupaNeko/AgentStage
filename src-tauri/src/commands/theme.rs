use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

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

fn read_theme_from_dir(path: &Path, source: &str) -> Option<ThemeInfo> {
    let theme_id = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty() && s != "user")?;

    if !is_valid_theme_id(&theme_id) {
        return None;
    }

    let theme_json_path = path.join("theme.json");
    if !theme_json_path.exists() {
        return None;
    }

    let content = fs::read_to_string(&theme_json_path).ok()?;
    let theme_json: ThemeJson = serde_json::from_str(&content).ok()?;

    let preview_path = if path.join("preview.png").exists() {
        if source == "user" {
            format!("themes/user/{}/preview.png", theme_id)
        } else {
            format!("themes/{}/preview.png", theme_id)
        }
    } else {
        String::new()
    };

    Some(ThemeInfo {
        id: theme_id,
        name: theme_json.name,
        version: theme_json.version,
        author: theme_json.author,
        description: theme_json.description,
        tags: theme_json.tags,
        preview_path,
        source: source.to_string(),
    })
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
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        if let Some(theme) = read_theme_from_dir(&path, "builtin") {
            themes.push(theme);
        }
    }

    let user_dir = themes_dir.join("user");
    if user_dir.exists() {
        let entries = fs::read_dir(&user_dir).map_err(|e| e.to_string())?;
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            if let Some(theme) = read_theme_from_dir(&path, "user") {
                themes.push(theme);
            }
        }
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
    if css_path.exists() {
        return fs::read_to_string(&css_path).map_err(|e| e.to_string());
    }

    let user_css_path = themes_dir.join("user").join(&theme_id).join("style.css");
    if user_css_path.exists() {
        return fs::read_to_string(&user_css_path).map_err(|e| e.to_string());
    }

    Err("Theme CSS not found".to_string())
}

pub fn ensure_themes_initialized() -> Result<(), String> {
    let themes_dir = get_themes_dir()?;
    fs::create_dir_all(&themes_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(themes_dir.join("user")).map_err(|e| e.to_string())?;
    Ok(())
}
