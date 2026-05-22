use std::collections::HashMap;
use serde_json::json;

use crate::db::connection::DbState;
use crate::llm::provider::LlmProvider;
use crate::llm::tool::{LlmResponse, ToolCall, ToolExecutor};
use crate::models::message::Message;

pub struct PromptParts {
    pub system: String,
    pub user: String,
}

pub struct ExecutedToolCall {
    pub tool_call: ToolCall,
    pub result: ToolExecutionResult,
}

pub enum ToolExecutionResult {
    Success(String),
    Error(String),
}

pub struct ConversationResult {
    pub final_content: Option<String>,
    pub executed_tool_calls: Vec<ExecutedToolCall>,
    pub messages: Vec<Message>,
    pub total_rounds: usize,
}

pub struct LlmConversation<P: LlmProvider> {
    provider: P,
    db_state: DbState,
    scheduler: crate::scheduler::Scheduler,
}

impl<P: LlmProvider> LlmConversation<P> {
    pub fn new(
        provider: P,
        db_state: DbState,
        scheduler: crate::scheduler::Scheduler,
    ) -> Self {
        Self { provider, db_state, scheduler }
    }

    pub async fn run(
        &self,
        system: &str,
        initial_user_content: &str,
        tools: Vec<serde_json::Value>,
        max_rounds: usize,
        agent_id: &str,
        session_pages: &HashMap<String, i32>,
    ) -> Result<ConversationResult, String> {
        let mut messages: Vec<serde_json::Value> = vec![
            json!({"role": "system", "content": system}),
            json!({"role": "user", "content": initial_user_content}),
        ];

        let mut executed_tool_calls: Vec<ExecutedToolCall> = Vec::new();
        let mut all_messages: Vec<Message> = Vec::new();
        let mut final_content: Option<String> = None;

        for round in 0..max_rounds {
            let mut response: Option<LlmResponse> = None;
            for attempt in 0..3 {
                match self.provider.chat_raw(messages.clone(), tools.clone()).await {
                    Ok(resp) => { response = Some(resp); break; }
                    Err(e) => {
                        crate::logger::backend("ERROR", &format!(
                            "[LlmConversation] round={} attempt={}/3 failed: {}", round + 1, attempt + 1, e
                        ));
                        if attempt == 2 { return Err(format!("LLM call failed after 3 retries: {}", e)); }
                    }
                }
            }
            let response = response.unwrap();

            let assistant_message = json!({
                "role": "assistant",
                "content": response.content,
                "tool_calls": response.tool_calls.iter().map(|tc| json!({
                    "id": tc.id,
                    "type": "function",
                    "function": { "name": tc.name, "arguments": tc.arguments }
                })).collect::<Vec<_>>()
            });
            messages.push(assistant_message);

            if response.tool_calls.is_empty() {
                final_content = response.content;
                break;
            }

            let executor = ToolExecutor::new(self.db_state.clone(), self.scheduler.clone());
            for tc in &response.tool_calls {
                let result = match executor.execute_single(agent_id, tc, session_pages).await {
                    Ok(msgs) => {
                        let text = if msgs.is_empty() { "执行成功".to_string() }
                                   else { format!("执行成功，产生 {} 条消息", msgs.len()) };
                        all_messages.extend(msgs);
                        ToolExecutionResult::Success(text)
                    }
                    Err(e) => ToolExecutionResult::Error(format!("执行失败: {}", e)),
                };

                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": tc.id,
                    "content": match &result {
                        ToolExecutionResult::Success(s) => s.clone(),
                        ToolExecutionResult::Error(e) => e.clone(),
                    }
                }));

                executed_tool_calls.push(ExecutedToolCall { tool_call: tc.clone(), result });
            }

            if round == max_rounds - 1 {
                crate::logger::backend("WARN", &format!("[LlmConversation] 达到最大轮次上限 {}，强制结束", max_rounds));
                break;
            }
        }

        let total_rounds = messages.iter().filter(|m| m["role"] == "assistant").count();
        Ok(ConversationResult { final_content, executed_tool_calls, messages: all_messages, total_rounds })
    }
}
