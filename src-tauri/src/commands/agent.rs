use tauri::State;
use crate::db::connection::{get_db, DbState};
use crate::db::agent as agent_repo;
use crate::db::user_persona as user_persona_repo;
use crate::models::agent::{AgentResponse, CreateAgentRequest, UpdateAgentRequest, DeleteAgentRequest, TestApiConnectionRequest, TestApiConnectionResponse};

#[tauri::command]
pub async fn create_agent(state: State<'_, DbState>, req: CreateAgentRequest) -> Result<AgentResponse, String> {
    crate::logger::backend("DEBUG", &format!("[DEBUG create_agent] name={}", req.name));

    let conn = get_db(&state).await?;
    if let Ok(Some(_)) = agent_repo::get_agent_by_name(&conn, &req.name) {
        return Err(format!("已存在同名角色 '{}'，请使用其他名称", req.name));
    }
    if let Ok(Some(_)) = user_persona_repo::get_user_persona_by_name(&conn, &req.name) {
        return Err(format!("该名称已被用户人设 '{}' 使用，请使用其他名称", req.name));
    }
    let agent = agent_repo::create(&conn, &req).map_err(|e| e.to_string())?;
    Ok(AgentResponse::from(agent))
}

#[tauri::command]
pub async fn get_agent(state: State<'_, DbState>, id: String) -> Result<Option<AgentResponse>, String> {
    crate::logger::backend("DEBUG", &format!("[DEBUG get_agent] id={}", id));

    let conn = get_db(&state).await?;
    let agent = agent_repo::get_by_id(&conn, &id).map_err(|e| e.to_string())?;
    Ok(agent.map(AgentResponse::from))
}

#[tauri::command]
pub async fn list_agents(state: State<'_, DbState>) -> Result<Vec<AgentResponse>, String> {
    let conn = get_db(&state).await?;
    let agents = agent_repo::list_all(&conn).map_err(|e| e.to_string())?;

    crate::logger::backend("DEBUG", &format!("[DEBUG list_agents] returned {} agents", agents.len()));
    Ok(agents.into_iter().map(AgentResponse::from).collect())
}

#[tauri::command]
pub async fn update_agent(state: State<'_, DbState>, req: UpdateAgentRequest) -> Result<AgentResponse, String> {
    crate::logger::backend("DEBUG", &format!("[DEBUG update_agent] id={}", req.id));

    let conn = get_db(&state).await?;
    if let Some(ref name) = req.name {
        if let Ok(Some(existing)) = agent_repo::get_agent_by_name(&conn, name) {
            if existing.id != req.id {
                return Err(format!("已存在同名角色 '{}'，请使用其他名称", name));
            }
        }
        if let Ok(Some(_)) = user_persona_repo::get_user_persona_by_name(&conn, name) {
            return Err(format!("该名称已被用户人设 '{}' 使用，请使用其他名称", name));
        }
    }
    let agent = agent_repo::update(&conn, &req).map_err(|e| e.to_string())?;
    Ok(AgentResponse::from(agent))
}

#[tauri::command]
pub async fn delete_agent(state: State<'_, DbState>, req: DeleteAgentRequest) -> Result<bool, String> {
    crate::logger::backend("DEBUG", &format!("[DEBUG delete_agent] id={}", req.id));

    let conn = get_db(&state).await?;
    agent_repo::soft_delete(&conn, &req.id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn test_api_connection(req: TestApiConnectionRequest) -> Result<TestApiConnectionResponse, String> {
    crate::logger::backend("DEBUG", &format!("[DEBUG test_api_connection] provider={} model={}", req.model_provider, req.model_name));

    let start = std::time::Instant::now();

    let base_url = req.base_url.unwrap_or_else(|| match req.model_provider.as_str() {
        "openai" => "https://api.openai.com/v1".to_string(),
        "anthropic" => "https://api.anthropic.com/v1".to_string(),
        "google" => "https://generativelanguage.googleapis.com/v1beta/openai".to_string(),
        "kimi" => "https://api.moonshot.cn/v1".to_string(),
        "minimax" => "https://api.minimax.chat/v1".to_string(),
        _ => "https://api.openai.com/v1".to_string(),
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("构建 HTTP 客户端失败: {}", e))?;

    let body = serde_json::json!({
        "model": req.model_name,
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 1
    });

    let url = format!("{}/chat/completions", base_url);

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", req.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    let latency_ms = start.elapsed().as_millis() as u64;
    let status = response.status();

    if status.is_success() {
        crate::logger::backend("DEBUG", &format!("[DEBUG test_api_connection] success latency={}ms", latency_ms));
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
        crate::logger::backend("DEBUG", &format!("[DEBUG test_api_connection] failed status={} msg={}", status, err_msg));
        Ok(TestApiConnectionResponse {
            success: false,
            latency_ms,
            message: err_msg,
        })
    }
}
