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
            let round_start = chrono::Utc::now().timestamp_millis();
            crate::logger::debug(&format!(
                "[LlmConversation] round={}/{} START messages_count={}",
                round + 1, max_rounds, messages.len()
            ));

            let mut response: Option<LlmResponse> = None;
            let llm_start = chrono::Utc::now().timestamp_millis();
            for attempt in 0..3 {
                match self.provider.chat_raw(messages.clone(), tools.clone()).await {
                    Ok(resp) => { response = Some(resp); break; }
                    Err(e) => {
                        crate::logger::error(&format!(
                            "[LlmConversation] round={} attempt={}/3 failed: {}", round + 1, attempt + 1, e
                        ));
                        if attempt == 2 { return Err(format!("LLM call failed after 3 retries: {}", e)); }
                    }
                }
            }
            let llm_elapsed = chrono::Utc::now().timestamp_millis() - llm_start;
            let response = response.unwrap();
            crate::logger::debug(&format!(
                "[LlmConversation] round={} LLM responded tool_calls={} content_len={} llm_elapsed_ms={}",
                round + 1, response.tool_calls.len(),
                response.content.as_ref().map(|c| c.len()).unwrap_or(0),
                llm_elapsed
            ));

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
                let round_elapsed = chrono::Utc::now().timestamp_millis() - round_start;
                crate::logger::debug(&format!(
                    "[LlmConversation] round={}/{} END (no tools) total_elapsed_ms={}",
                    round + 1, max_rounds, round_elapsed
                ));
                break;
            }

            let tool_start = chrono::Utc::now().timestamp_millis();
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

            // 在 tool results 后添加提示，引导 AI 继续完成剩余任务
            messages.push(json!({
                "role": "user",
                "content": "工具调用已执行完毕。请根据执行结果检查是否还有需要继续完成的操作（如发送消息、创建定时任务、修改记忆或人设等）。如果所有任务已完成，请直接回复用户或返回空内容，不要调用任何工具。"
            }));

            let tool_elapsed = chrono::Utc::now().timestamp_millis() - tool_start;
            let round_elapsed = chrono::Utc::now().timestamp_millis() - round_start;
            crate::logger::debug(&format!(
                "[LlmConversation] round={}/{} END tools={} tool_elapsed_ms={} total_elapsed_ms={}",
                round + 1, max_rounds, response.tool_calls.len(), tool_elapsed, round_elapsed
            ));

            if round == max_rounds - 1 {
                crate::logger::warn(&format!("[LlmConversation] 达到最大轮次上限 {}，强制结束", max_rounds));
                break;
            }
        }

        let total_rounds = messages.iter().filter(|m| m["role"] == "assistant").count();
        Ok(ConversationResult { final_content, executed_tool_calls, messages: all_messages, total_rounds })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Mutex;
    use std::sync::Arc;
    use tokio::sync::Mutex as TokioMutex;
    use rusqlite::Connection;
    use crate::db::connection::DbState;
    use crate::db::schema::*;
    use std::collections::HashMap;

    struct MockProvider { responses: Mutex<Vec<LlmResponse>> }
    #[async_trait]
    impl LlmProvider for MockProvider {
        async fn chat(&self, _s: &str, _m: Vec<serde_json::Value>, _t: Vec<serde_json::Value>) -> Result<LlmResponse, String> { unimplemented!() }
        async fn chat_raw(&self, _m: Vec<serde_json::Value>, _t: Vec<serde_json::Value>) -> Result<LlmResponse, String> {
            Ok(self.responses.lock().unwrap().remove(0))
        }
    }
    fn mock_provider(responses: Vec<LlmResponse>) -> MockProvider { MockProvider { responses: Mutex::new(responses) } }
    fn make_response(content: Option<&str>, tool_calls: Vec<ToolCall>) -> LlmResponse {
        LlmResponse { content: content.map(|s| s.to_string()), tool_calls, usage: None }
    }
    fn make_tool_call(id: &str, name: &str, args: &str) -> ToolCall {
        ToolCall { id: id.to_string(), name: name.to_string(), arguments: args.to_string() }
    }
    fn init_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = OFF;", []).unwrap();
        conn.execute_batch(MIGRATION_V1).unwrap();
        conn.execute_batch(MIGRATION_V2).unwrap();
        conn.execute_batch(MIGRATION_V3).unwrap();
        conn.execute_batch(MIGRATION_V4).unwrap();
        conn.execute_batch(MIGRATION_V5).unwrap();
        conn.execute_batch(MIGRATION_V7).unwrap();
        conn.execute_batch(MIGRATION_V11).unwrap();
        conn.execute_batch(MIGRATION_V12).unwrap();
        conn.execute_batch(MIGRATION_V13).unwrap();
        conn.execute_batch(MIGRATION_V15).unwrap();
        conn
    }
    fn make_db_state(conn: Connection) -> DbState { DbState(Arc::new(TokioMutex::new(conn))) }

    #[tokio::test]
    async fn test_zero_round_no_tools() {
        let db = make_db_state(init_test_db());
        let scheduler = crate::scheduler::Scheduler::new(db.clone());
        let provider = mock_provider(vec![make_response(Some("Done"), vec![])]);
        let conv = LlmConversation::new(provider, db, scheduler);
        let result = conv.run("sys", "usr", vec![], 5, "agent1", &HashMap::new()).await.unwrap();
        assert_eq!(result.total_rounds, 1);
        assert!(result.final_content.is_some());
        assert_eq!(result.executed_tool_calls.len(), 0);
        assert_eq!(result.messages.len(), 0);
    }

    #[tokio::test]
    async fn test_reaches_max_rounds() {
        let db = make_db_state(init_test_db());
        let scheduler = crate::scheduler::Scheduler::new(db.clone());
        let provider = mock_provider(vec![
            make_response(None, vec![make_tool_call("tc1","delete_timer",r#"{"task_id":"x"}"#)]),
            make_response(None, vec![make_tool_call("tc2","delete_timer",r#"{"task_id":"x"}"#)]),
            make_response(None, vec![make_tool_call("tc3","delete_timer",r#"{"task_id":"x"}"#)]),
            make_response(None, vec![make_tool_call("tc4","delete_timer",r#"{"task_id":"x"}"#)]),
            make_response(None, vec![make_tool_call("tc5","delete_timer",r#"{"task_id":"x"}"#)]),
        ]);
        let conv = LlmConversation::new(provider, db, scheduler);
        let result = conv.run("sys", "usr", vec![], 5, "agent1", &HashMap::new()).await.unwrap();
        assert_eq!(result.total_rounds, 5);
        assert!(result.final_content.is_none());
        assert_eq!(result.executed_tool_calls.len(), 5);
    }
}
