use tauri::State;
use crate::db::connection::{get_db, DbState};
use crate::db::settings as settings_repo;
use crate::models::settings::{SettingsResponse, UpdateAppSettingsRequest};

#[tauri::command]
pub async fn get_settings(state: State<'_, DbState>) -> Result<SettingsResponse, String> {
    let conn = get_db(&state).await?;
    let settings = settings_repo::get_or_create_settings(&conn)
        .map_err(|e| e.to_string())?;
    Ok(settings.into())
}

#[tauri::command]
pub async fn update_settings(
    state: State<'_, DbState>,
    req: UpdateAppSettingsRequest,
) -> Result<SettingsResponse, String> {
    let conn = get_db(&state).await?;
    settings_repo::update_settings(&conn, &req)
        .map_err(|e| e.to_string())?;

    // 搜索 API 配置：厂商变更且未提供新 Key 时，清空已存 Key
    if req.search_provider.is_some() || req.search_api_key.is_some() {
        let current = settings_repo::get_or_create_settings(&conn)
            .map_err(|e| e.to_string())?;
        let new_provider = req.search_provider.clone().or(current.search_provider.clone());
        let new_key: Option<Vec<u8>> = if let Some(ref raw) = req.search_api_key {
            if raw.is_empty() {
                None // 显式清空
            } else {
                Some(crate::crypto::encrypt(raw).map_err(|e| format!("加密 API Key 失败: {}", e))?)
            }
        } else if req.search_provider.is_some() && req.search_provider != current.search_provider {
            None // 切换厂商，旧 Key 不再适用
        } else {
            current.search_api_key_encrypted.clone()
        };
        settings_repo::update_search_config(&conn, new_provider.as_deref(), new_key.as_deref())
            .map_err(|e| e.to_string())?;
    }

    // 虚拟时间配置
    if req.virtual_time_enabled.is_some() || req.virtual_time_base.is_some() || req.virtual_time_rate.is_some() {
        let current = settings_repo::get_or_create_settings(&conn)
            .map_err(|e| e.to_string())?;
        settings_repo::update_virtual_time(
            &conn,
            req.virtual_time_enabled.unwrap_or(current.virtual_time_enabled),
            req.virtual_time_base,
            req.virtual_time_rate,
        )
        .map_err(|e| e.to_string())?;
    }

    let settings = settings_repo::get_or_create_settings(&conn)
        .map_err(|e| e.to_string())?;
    Ok(settings.into())
}

/// 测试搜索 API 连通性。api_key 为空时使用已保存的 Key。
#[tauri::command]
pub async fn test_search_api(
    state: State<'_, DbState>,
    provider: String,
    api_key: Option<String>,
) -> Result<String, String> {
    let key = match api_key.filter(|k| !k.is_empty()) {
        Some(k) => k,
        None => {
            let conn = get_db(&state).await?;
            let settings = settings_repo::get_or_create_settings(&conn)
                .map_err(|e| e.to_string())?;
            settings
                .search_api_key_encrypted
                .as_ref()
                .and_then(|enc| crate::crypto::decrypt(enc).ok())
                .filter(|k| !k.is_empty())
                .ok_or_else(|| crate::search::SearchError::NotConfigured.user_message(""))?
        }
    };
    let searcher = crate::search::create_provider(&provider, &key)
        .map_err(|e| e.user_message(&provider))?;
    searcher
        .search("今日新闻")
        .await
        .map_err(|e| e.user_message(searcher.display_name()))?;
    Ok(format!("{}搜索 API 连接成功", searcher.display_name()))
}
