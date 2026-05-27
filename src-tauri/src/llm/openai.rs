use async_trait::async_trait;
use reqwest;
use serde_json;

use crate::llm::provider::LlmProvider;
use crate::llm::tool::{LlmResponse, ToolCall};

pub struct OpenAiCompatibleProvider {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
    temperature: Option<f64>,
    max_tokens: i32,
}

impl OpenAiCompatibleProvider {
    pub fn new(
        api_key: String,
        base_url: Option<String>,
        model: String,
        temperature: Option<f64>,
        max_tokens: i32,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("Failed to build reqwest client");

        Self {
            client,
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            model,
            temperature,
            max_tokens,
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatibleProvider {
    async fn chat(
        &self,
        system_prompt: &str,
        messages: Vec<serde_json::Value>,
        tools: Vec<serde_json::Value>,
    ) -> Result<LlmResponse, String> {
        let mut full_messages = vec![serde_json::json!({
            "role": "system",
            "content": system_prompt,
        })];
        full_messages.extend(messages);
        self.chat_raw(full_messages, tools).await
    }

    async fn chat_raw(
        &self,
        messages: Vec<serde_json::Value>,
        tools: Vec<serde_json::Value>,
    ) -> Result<LlmResponse, String> {
        let mut request_body = serde_json::json!({
            "model": self.model,
            "messages": messages,
            "max_tokens": self.max_tokens,
        });

        if let Some(temp) = self.temperature {
            request_body["temperature"] = serde_json::json!(temp);
        }

        // Some providers (e.g. Minimax) require at least one user message.
        // If there are no user messages, append a dummy user message.
        if let Some(arr) = request_body["messages"].as_array() {
            let has_user = arr.iter().any(|m| m.get("role") == Some(&serde_json::json!("user")));
            if !has_user {
                if let Some(arr_mut) = request_body["messages"].as_array_mut() {
                    arr_mut.push(serde_json::json!({
                        "role": "user",
                        "content": ".",
                    }));
                }
            }
        }

        // Add tools if provided
        if !tools.is_empty() {
            request_body["tools"] = serde_json::Value::Array(tools);
            request_body["tool_choice"] = serde_json::json!("auto");
        }

        let url = format!("{}/chat/completions", self.base_url);
        let messages_count = request_body["messages"].as_array().map(|a| a.len()).unwrap_or(0);
        crate::logger::debug(&format!("[DEBUG openai::chat_raw] url={}, model={}, messages_count={}, tools_empty={}", url, self.model, messages_count, request_body.get("tools").is_none()));

        // Log each message content for multi-turn debugging (full content, no truncation)
        if let Some(arr) = request_body["messages"].as_array() {
            for (i, msg) in arr.iter().enumerate() {
                let role = msg["role"].as_str().unwrap_or("?");
                let content = msg["content"].as_str().unwrap_or("(null)");
                let tool_calls = msg["tool_calls"].as_array();
                let tool_calls_info = if let Some(tcs) = tool_calls {
                    let names: Vec<String> = tcs.iter().map(|tc| tc["function"]["name"].as_str().unwrap_or("?").to_string()).collect();
                    format!(" [tool_calls: {}]", names.join(", "))
                } else { String::new() };
                let tool_call_id = msg["tool_call_id"].as_str().unwrap_or("");
                let tool_id_info = if !tool_call_id.is_empty() { format!(" [tool_call_id: {}]", tool_call_id) } else { String::new() };
                crate::logger::debug(&format!(
                    "[DEBUG openai::chat_raw] msg[{}] role={} content={}{}{}",
                    i, role, content, tool_calls_info, tool_id_info
                ));
            }
        }

        let send_start = chrono::Utc::now().timestamp_millis();
        crate::logger::debug(&format!("[DEBUG openai::chat_raw] sending request..."));

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        let send_elapsed = chrono::Utc::now().timestamp_millis() - send_start;
        let status = response.status();
        crate::logger::debug(&format!(
            "[DEBUG openai::chat_raw] http_status={}, send_elapsed_ms={}",
            status, send_elapsed
        ));
        if !status.is_success() {
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(format!("HTTP {}: {}", status, text));
        }

        let parse_start = chrono::Utc::now().timestamp_millis();
        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse JSON response: {}", e))?;
        let parse_elapsed = chrono::Utc::now().timestamp_millis() - parse_start;
        crate::logger::debug(&format!(
            "[DEBUG openai::chat_raw] json_parse_elapsed_ms={}", parse_elapsed
        ));

        let choice = json["choices"]
            .get(0)
            .ok_or_else(|| "No choices in response".to_string())?;
        let message = choice["message"].clone();

        let content = message["content"].as_str().map(|s| s.to_string());

        let mut tool_calls: Vec<ToolCall> = Vec::new();
        if let Some(arr) = message["tool_calls"].as_array() {
            for tc in arr {
                let id = tc["id"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let name = tc["function"]["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                let arguments = tc["function"]["arguments"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string();
                tool_calls.push(ToolCall {
                    id,
                    name,
                    arguments,
                });
            }
        }

        let tool_calls_json = serde_json::to_string(&tool_calls).unwrap_or_default();
        crate::logger::debug(&format!(
            "[DEBUG openai::chat_raw] response content={:?} tool_calls={}",
            content, tool_calls_json
        ));

        let usage = message.get("usage").cloned().or_else(|| json.get("usage").cloned());

        Ok(LlmResponse {
            content,
            tool_calls,
            usage,
        })
    }
}
