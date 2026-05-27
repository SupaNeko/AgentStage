use tauri::State;
use crate::db::connection::{get_db, DbState};
use crate::db::model_config as model_config_repo;
use crate::models::model_config::{ModelConfigResponse, CreateModelConfigRequest, UpdateModelConfigRequest, DeleteModelConfigRequest, TestModelConfigConnectionRequest, TestApiConnectionResponse};

#[tauri::command]
pub async fn list_model_configs(state: State<'_, DbState>) -> Result<Vec<ModelConfigResponse>, String> {
    let conn = get_db(&state).await?;
    let configs = model_config_repo::list_all(&conn).map_err(|e| e.to_string())?;
    Ok(configs.into_iter().map(ModelConfigResponse::from).collect())
}

#[tauri::command]
pub async fn create_model_config(state: State<'_, DbState>, req: CreateModelConfigRequest) -> Result<ModelConfigResponse, String> {
    let conn = get_db(&state).await?;
    let config = model_config_repo::create(&conn, &req).map_err(|e| e.to_string())?;
    Ok(ModelConfigResponse::from(config))
}

#[tauri::command]
pub async fn update_model_config(state: State<'_, DbState>, req: UpdateModelConfigRequest) -> Result<ModelConfigResponse, String> {
    let conn = get_db(&state).await?;
    let config = model_config_repo::update(&conn, &req).map_err(|e| e.to_string())?;
    Ok(ModelConfigResponse::from(config))
}

#[tauri::command]
pub async fn delete_model_config(state: State<'_, DbState>, req: DeleteModelConfigRequest) -> Result<(), String> {
    let conn = get_db(&state).await?;
    model_config_repo::delete(&conn, &req.id).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn test_model_config_connection(state: State<'_, DbState>, req: TestModelConfigConnectionRequest) -> Result<TestApiConnectionResponse, String> {
    crate::logger::debug(&format!("[DEBUG test_model_config_connection] id={}", req.id));

    let conn = get_db(&state).await?;
    let config = model_config_repo::get_by_id(&conn, &req.id)
        .map_err(|e| e.to_string())?
        .ok_or("Model config not found")?;

    let start = std::time::Instant::now();

    let base_url = config.base_url.unwrap_or_else(|| match config.provider.as_str() {
        "openai" => "https://api.openai.com/v1".to_string(),
        "anthropic" => "https://api.anthropic.com/v1".to_string(),
        "google" => "https://generativelanguage.googleapis.com/v1beta/openai".to_string(),
        "kimi" => "https://api.moonshot.cn/v1".to_string(),
        "minimax" => "https://api.minimax.chat/v1".to_string(),
        _ => "https://api.openai.com/v1".to_string(),
    });

    let api_key = config.api_key_encrypted
        .as_ref()
        .and_then(|enc| crate::crypto::decrypt(enc).ok())
        .unwrap_or_default();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("构建 HTTP 客户端失败: {}", e))?;

    let body = serde_json::json!({
        "model": config.model_name,
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 1
    });

    let url = format!("{}/chat/completions", base_url);

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    let latency_ms = start.elapsed().as_millis() as u64;
    let status = response.status();

    if status.is_success() {
        crate::logger::debug(&format!("[DEBUG test_model_config_connection] success latency={}ms", latency_ms));
        Ok(TestApiConnectionResponse {
            success: true,
            latency_ms,
            message: "连接成功".to_string(),
        })
    } else {
        let text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        let err_msg = if status.as_u16() == 401 {
            "API Key 无效或已过期".to_string()
        } else if status.as_u16() == 404 {
            "模型不存在，请检查模型名称".to_string()
        } else if status.as_u16() == 429 {
            "请求过于频繁，请稍后再试".to_string()
        } else {
            format!("HTTP {}: {}", status, text)
        };
        crate::logger::debug(&format!("[DEBUG test_model_config_connection] failed status={} msg={}", status, err_msg));
        Ok(TestApiConnectionResponse {
            success: false,
            latency_ms,
            message: err_msg,
        })
    }
}
