use async_trait::async_trait;
use crate::llm::tool::LlmResponse;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(
        &self,
        system_prompt: &str,
        messages: Vec<serde_json::Value>,
        tools: Vec<serde_json::Value>,
    ) -> Result<LlmResponse, String>;
}
