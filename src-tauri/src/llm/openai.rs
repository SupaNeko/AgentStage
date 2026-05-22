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
    temperature: f64,
    max_tokens: i32,
}

impl OpenAiCompatibleProvider {
    pub fn new(
        api_key: String,
        base_url: Option<String>,
        model: String,
        temperature: f64,
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
            "temperature": self.temperature,
            "max_tokens": self.max_tokens,
        });

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
        crate::logger::backend("DEBUG", &format!("[DEBUG openai::chat_raw] url={}, model={}, messages_count={}, tools_empty={}", url, self.model, messages_count, request_body.get("tools").is_none()));

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        let status = response.status();
        crate::logger::backend("DEBUG", &format!("[DEBUG openai::chat_raw] http_status={}", status));
        if !status.is_success() {
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(format!("HTTP {}: {}", status, text));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse JSON response: {}", e))?;

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

        crate::logger::backend("DEBUG", &format!("[DEBUG openai::chat_raw] content_exists={}, tool_calls_count={}", content.is_some(), tool_calls.len()));

        let usage = message.get("usage").cloned().or_else(|| json.get("usage").cloned());

        Ok(LlmResponse {
            content,
            tool_calls,
            usage,
        })
    }
}
