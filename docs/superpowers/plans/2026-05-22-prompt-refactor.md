# Prompt 结构优化 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将工具描述迁移到 Schema，将 Prompt 拆分为 system/user content，实现标准 OpenAI 多轮 Tool Calling（最多 5 轮），应用于所有 LLM 调用场景。

**Architecture:** 新增 `LlmConversation<P: LlmProvider>` 统一多轮对话管理器；`PromptAssembler` 返回 `PromptParts { system, user }`；`OpenAiCompatibleProvider` 新增 `chat_raw` 底层接口；所有调用点统一接入 `LlmConversation::run`。

**Tech Stack:** Rust, Tauri v2, async-trait, OpenAI-compatible API, rusqlite

---

## 文件结构映射

| 文件 | 职责 |
|------|------|
| `src-tauri/src/llm/provider.rs` | `LlmProvider` trait，新增 `chat_raw` 方法 |
| `src-tauri/src/llm/openai.rs` | `OpenAiCompatibleProvider` 实现 `chat_raw`，旧 `chat` 改为包装器 |
| `src-tauri/src/llm/conversation.rs` | **新建**：`PromptParts`、`ConversationResult`、`LlmConversation` 多轮管理器 |
| `src-tauri/src/llm/mod.rs` | 导出 `conversation` 模块 |
| `src-tauri/src/llm/prompt_templates.rs` | `SYSTEM_PROMPT` 精简工具列表；`TOOL_INSTRUCTION_TEMPLATE` 移除详细说明 |
| `src-tauri/src/llm/prompt.rs` | `assemble()` 返回 `PromptParts`；`build_instruction()` 精简；测试更新 |
| `src-tauri/src/llm/tool.rs` | 各 schema `description` 扩充；新增 `execute_single()`；测试 |
| `src-tauri/src/scheduler/mod.rs` | 3 个调用点改造为 `LlmConversation::run` |

---

## Task 1: 扩展 LlmProvider trait + 改造 OpenAiCompatibleProvider

**Files:**
- Modify: `src-tauri/src/llm/provider.rs`
- Modify: `src-tauri/src/llm/openai.rs`

- [ ] **Step 1: 给 `LlmProvider` trait 添加 `chat_raw` 方法**

在 `provider.rs` 的 trait 中追加方法签名（已有 `async_trait` 和 `LlmResponse` import，保持不变）：

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(
        &self,
        system_prompt: &str,
        messages: Vec<serde_json::Value>,
        tools: Vec<serde_json::Value>,
    ) -> Result<LlmResponse, String>;

    async fn chat_raw(
        &self,
        messages: Vec<serde_json::Value>,
        tools: Vec<serde_json::Value>,
    ) -> Result<LlmResponse, String>;
}
```

- [ ] **Step 2: 在 `openai.rs` 提取 `chat_raw`，让 `chat` 调用它**

将 `impl LlmProvider for OpenAiCompatibleProvider` 中原来的 `chat` 方法体提取为 `chat_raw`。`chat` 仅负责拼接 system message 到 messages 头部，然后调用 `chat_raw`。

`chat_raw` 保留原有的 HTTP 请求、响应解析、日志记录逻辑，但不再自动插入 system prompt（因为调用方已传入完整 messages 数组）。保留 Minimax 兼容性 fallback（检查 messages 中是否至少有一个 user role，若无则追加一个）。

关键变更点：
- `chat` 方法：先 `let mut full_messages = vec![system_msg]`，再 `full_messages.extend(messages)`，最后 `self.chat_raw(full_messages, tools).await`
- `chat_raw` 方法：request_body 的 `messages` 直接使用传入参数；日志标签从 `[DEBUG openai::chat]` 改为 `[DEBUG openai::chat_raw]`

- [ ] **Step 3: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: 0 errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/llm/provider.rs src-tauri/src/llm/openai.rs
git commit -m "feat(llm): add chat_raw to LlmProvider trait and OpenAiCompatibleProvider"
```

---

## Task 2: 创建 conversation.rs 核心数据结构

**Files:**
- Create: `src-tauri/src/llm/conversation.rs`
- Modify: `src-tauri/src/llm/mod.rs`

- [ ] **Step 1: 创建 `conversation.rs`**

新建文件，定义以下结构：

```rust
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
```

- [ ] **Step 2: 导出模块**

在 `src-tauri/src/llm/mod.rs` 第一行追加：`pub mod conversation;`

- [ ] **Step 3: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: 0 errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/llm/conversation.rs src-tauri/src/llm/mod.rs
git commit -m "feat(llm): add LlmConversation multi-turn manager with PromptParts and ConversationResult"
```

---

## Task 3: LlmConversation 单元测试

**Files:**
- Modify: `src-tauri/src/llm/conversation.rs`

- [ ] **Step 1: 添加 mock provider 和测试**

在 `conversation.rs` 末尾追加 `#[cfg(test)] mod tests`：

```rust
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
```

- [ ] **Step 2: 运行测试**

Run: `cd src-tauri && cargo test --lib llm::conversation::tests`
Expected: 2 passed

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/llm/conversation.rs
git commit -m "test(llm): add LlmConversation unit tests for round counting and max rounds"
```

---

## Task 4: Prompt 模板精简

**Files:**
- Modify: `src-tauri/src/llm/prompt_templates.rs`

- [ ] **Step 1: 精简 `SYSTEM_PROMPT` 中的工具列表**

将 `SYSTEM_PROMPT` 中 `## 6. 可用工具` 之后的详细说明替换为一句话列表（保留原有 `SYSTEM_PROMPT` 的其他部分不变）：

```
## 6. 可用工具
- send_message：向指定会话发送消息
- start_private_chat：向某个角色发起私聊
- update_relationship：更新你对某个参与者的关系描述
- update_memory：更新你的记忆
- create_timer：创建一个定时任务
- delete_timer：删除一个定时任务
```

- [ ] **Step 2: 精简 `TOOL_INSTRUCTION_TEMPLATE`**

将 `TOOL_INSTRUCTION_TEMPLATE` 替换为仅保留 `context_list` 和极简提示的版本（约 5 行）。移除所有工具的参数说明、示例、注意事项。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/llm/prompt_templates.rs
git commit -m "feat(prompt): simplify SYSTEM_PROMPT and TOOL_INSTRUCTION_TEMPLATE"
```

---

## Task 5: PromptAssembler 拆分 + 测试更新

**Files:**
- Modify: `src-tauri/src/llm/prompt.rs`

- [ ] **Step 1: 修改 `assemble` 返回 `PromptParts`**

将 `assemble` 的返回类型从 `Result<String, String>` 改为 `Result<PromptParts, String>`。

在方法末尾，将原 `layers.join("\n\n")` 拆分为两段：
- `system_layers` = Layer 1 (SYSTEM_PROMPT + TIMER_CAPABILITY) + Layer 2 (【你的角色设定】+ persona)
- `user_layers` = Layer 2.8 (pending timers) + Layer 2.5 (memory) + Layer 3 (participants) + Layer 4 (history) + Layer 6 (instruction)

返回：
```rust
Ok(PromptParts {
    system: system_layers.join("\n\n"),
    user: user_layers.into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n\n"),
})
```

- [ ] **Step 2: 更新所有测试**

现有测试调用 `PromptAssembler::assemble(...).unwrap()` 得到 String，现在需改为：

```rust
let parts = PromptAssembler::assemble(...).unwrap();
```

然后更新断言：
- `assert!(parts.system.contains("【你的角色设定】"))`
- `assert!(parts.user.contains("【历史聊天记录】"))`
- `assert!(parts.system.contains("你是一个正在参与即时通讯聊天的 AI 角色"))`
- `assert!(!parts.system.contains("send_message —"))` (详细说明不应在 system 中)
- 其他原有断言针对具体内容的，检查 `parts.user` 中是否包含

- [ ] **Step 3: 运行测试**

Run: `cd src-tauri && cargo test --lib llm::prompt`
Expected: 全部通过

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/llm/prompt.rs
git commit -m "feat(prompt): split assemble into system/user PromptParts"
```

---

## Task 6: 工具 Schema 描述扩充

**Files:**
- Modify: `src-tauri/src/llm/tool.rs`

- [ ] **Step 1: 扩充 `send_message` description**

将 `description` 替换为包含规则的长文本（从 `TOOL_INSTRUCTION_TEMPLATE` 迁移过来）：

```json
"description": "向指定会话发送一条消息。你可以在 content 中使用 <br/> 标签进行分割，被分割的消息将被显示为多条消息。\n\n规则：\n1. target_id 必须是系统提供的完整 session_id，不能使用会话名称或其他 ID。\n2. 只能回复上方列出的会话（见 context_list）。\n3. target_type 为 'private' 或 'group'。\n4. 如果填入无效的 target_id，调用会失败。"
```

- [ ] **Step 2: 扩充 `start_private_chat` description**

迁移原模板中关于发起私聊的规则和注意事项。

- [ ] **Step 3: 扩充 `update_relationship` description**

迁移原模板中关于关系描述的全部规则、old_text 精确匹配要求、200字限制、示例。

- [ ] **Step 4: 扩充 `update_memory` description**

迁移原模板中关于记忆的全部规则、self/other 区别、old_text 精确匹配、字数限制、示例。

- [ ] **Step 5: 扩充 `create_timer` 和 `delete_timer` description**

迁移原模板中关于定时任务的说明。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/llm/tool.rs
git commit -m "feat(tools): migrate detailed tool descriptions from prompt template to schema definitions"
```

---

## Task 7: ToolExecutor.execute_single + 测试

**Files:**
- Modify: `src-tauri/src/llm/tool.rs`

- [ ] **Step 1: 提取 `execute_single` 方法**

在 `ToolExecutor` 中新增方法：

```rust
pub async fn execute_single(
    &self,
    agent_id: &str,
    tool_call: &ToolCall,
    session_pages: &HashMap<String, i32>,
) -> Result<Vec<Message>, ToolError> {
    match tool_call.name.as_str() {
        "send_message" => self.execute_send_message(agent_id, &tool_call.arguments, session_pages).await,
        "start_private_chat" => self.execute_start_private_chat(agent_id, &tool_call.arguments, session_pages).await,
        "update_relationship" => { self.execute_update_relationship(agent_id, &tool_call.arguments).await?; Ok(vec![]) }
        "update_memory" => { self.execute_update_memory(agent_id, &tool_call.arguments).await?; Ok(vec![]) }
        "create_timer" => { self.execute_create_timer(agent_id, &tool_call.arguments).await?; Ok(vec![]) }
        "delete_timer" => { self.execute_delete_timer(agent_id, &tool_call.arguments).await?; Ok(vec![]) }
        _ => Err(ToolError::InvalidArguments(format!("未知工具: {}", tool_call.name))),
    }
}
```

然后重构现有的 `execute` 方法，使其循环调用 `execute_single`：

```rust
pub async fn execute(
    &self,
    agent_id: &str,
    tool_calls: Vec<ToolCall>,
    session_pages: &HashMap<String, i32>,
) -> Result<Vec<Message>, ToolError> {
    let mut results = Vec::new();
    for tc in tool_calls {
        let msgs = self.execute_single(agent_id, &tc, session_pages).await?;
        results.extend(msgs);
    }
    Ok(results)
}
```

- [ ] **Step 2: 运行现有测试**

Run: `cd src-tauri && cargo test --lib llm::tool`
Expected: 全部通过（重构后的 `execute` 行为应与之前一致）

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/llm/tool.rs
git commit -m "refactor(tools): extract execute_single from ToolExecutor and reuse in execute loop"
```

---

## Task 8: trigger_agent_inner 改造（聊天触发）

**Files:**
- Modify: `src-tauri/src/scheduler/mod.rs`

- [ ] **Step 1: 改造 LLM 调用部分**

找到 `trigger_agent_inner` 中的以下代码块：

```rust
let (agent, prompt) = { ... PromptAssembler::assemble(...) ... };
```

改为：

```rust
let (agent, parts) = {
    let conn = self.db_state.0.lock().await;
    // ... 原有逻辑 ...
    let parts = PromptAssembler::assemble(&conn, agent_id, ...).map_err(|e| e.to_string())?;
    (agent, parts)
};
```

找到阶段 4 的 LLM 调用（`call_llm_with_retry`）和阶段 5 的 `ToolExecutor::execute`，替换为：

```rust
use crate::llm::conversation::LlmConversation;
use crate::llm::tool::get_all_tool_schemas;

let conversation = LlmConversation::new(provider, self.db_state.clone(), self.clone());
let result = match conversation.run(
    &parts.system,
    &parts.user,
    get_all_tool_schemas(),
    5,
    agent_id,
    &session_pages,
).await {
    Ok(r) => r,
    Err(e) => {
        crate::logger::backend("ERROR", &format!("[trigger_agent_inner] LLM conversation failed: {}", e));
        self.restore_pending(agent_id, pending).await;
        self.emit("agent_error", serde_json::json!({"agent_id": agent_id, "error": e}));
        return Ok(());
    }
};

let agent_messages = result.messages;
```

- [ ] **Step 2: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: 0 errors

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/scheduler/mod.rs
git commit -m "feat(scheduler): migrate trigger_agent_inner to LlmConversation multi-turn"
```

---

## Task 9: trigger_special 改造

**Files:**
- Modify: `src-tauri/src/scheduler/mod.rs`

- [ ] **Step 1: 改造 prompt 组装和 LLM 调用**

在 `trigger_special` 中找到 `PromptAssembler::assemble` 调用，改为获取 `PromptParts`：

```rust
let parts = {
    let conn = self.db_state.0.lock().await;
    PromptAssembler::assemble(&conn, agent_id, ...).map_err(|e| e.to_string())?
};
```

然后拼接 special_layer 到 user content：

```rust
let full_user = format!("{}\n\n{}", parts.user, special_layer);
```

将原有的 `Self::call_llm` + `ToolExecutor::execute` 替换为：

```rust
let conversation = LlmConversation::new(provider, self.db_state.clone(), self.clone());
let result = conversation.run(&parts.system, &full_user, get_all_tool_schemas(), 5, agent_id, &HashMap::new()).await;
```

- [ ] **Step 2: 编译检查 + Commit**

Run: `cd src-tauri && cargo check`

```bash
git add src-tauri/src/scheduler/mod.rs
git commit -m "feat(scheduler): migrate trigger_special to LlmConversation multi-turn"
```

---

## Task 10: overflow_summary 改造

**Files:**
- Modify: `src-tauri/src/scheduler/mod.rs`

- [ ] **Step 1: 拆分 system/user 并接入 LlmConversation**

在 `do_run_overflow_summary` 中找到以下代码：

```rust
let system_prompt = prompt_templates::SUMMARY_SYSTEM_PROMPT
    .replace("{current_time}", &now)
    .replace("{detailed_persona}", &agent.detailed_persona)
    .replace("{long_term_memory}", long_term_memory)
    .replace("{participants}", &participants_text)
    .replace("{session_messages}", &session_messages_text);
```

替换为：

```rust
let system = format!(
    "你是一个记忆整理助手。你的任务是在一次聊天会话结束后，回顾对话内容，判断是否有值得长期保存的信息。\n\n当前时间：{}\n\n## 你的角色设定\n{}\n\n## 可用工具\n- update_memory：更新你的记忆\n- update_relationship：更新关系描述\n\n## 任务\n请仔细阅读本次对话记录，判断是否有值得保存的信息。如果有，请使用工具更新。如果没有，可以不调用任何工具。",
    now, agent.detailed_persona
);

let user = format!(
    "## 关于你的记忆\n{}\n\n## 你认识的参与者\n{}\n\n## 本次对话记录\n{}\n\n请回顾本次对话，判断是否有值得保存到记忆中的信息。",
    long_term_memory, participants_text, session_messages_text
);
```

然后替换 LLM 调用：

```rust
let tools = vec![update_memory_tool_schema(), update_relationship_tool_schema()];
let conversation = LlmConversation::new(provider, self.db_state.clone(), self.clone());
let _ = conversation.run(&system, &user, tools, 5, &agent_id, &session_pages).await;
```

- [ ] **Step 2: 编译检查 + Commit**

Run: `cd src-tauri && cargo check`

```bash
git add src-tauri/src/scheduler/mod.rs
git commit -m "feat(scheduler): migrate overflow_summary to LlmConversation with system/user split"
```

---

## Task 11: 最终验证

**Files:** 全部

- [ ] **Step 1: 全量编译检查**

Run: `cd src-tauri && cargo check`
Expected: 0 errors

- [ ] **Step 2: 前端类型检查**

Run: `npx svelte-check --tsconfig ./tsconfig.json`
Expected: 0 errors（83 个预存 a11y 警告可忽略）

- [ ] **Step 3: 最终提交（如无修改则跳过）**

若前两步均通过且工作区无未提交改动，此步骤跳过。若有临时修复，提交：

```bash
git commit -m "fix: resolve compilation issues after prompt refactor"
```

---

## 自检清单

1. **Spec 覆盖**：
   - [x] 工具描述迁移到 Schema → Task 6
   - [x] Prompt 拆分为 system/user → Task 5
   - [x] 多轮 Tool Calling（最多 5 轮）→ Task 2 + Task 3
   - [x] 标准 OpenAI role="tool" 回传 → Task 2 (run 方法)
   - [x] tool_calls 为空自动结束 → Task 2 (run 方法中的 break)
   - [x] 所有 LLM 调用场景改造 → Task 8, 9, 10
   - [x] 溢出摘要拆分 → Task 10

2. **Placeholder 扫描**：计划中无 TBD/TODO/"implement later"/"similar to"。

3. **类型一致性**：
   - `PromptParts` 在 Task 2 定义，Task 5 使用 → 一致
   - `LlmConversation<P: LlmProvider>` 在 Task 2 定义，Task 8/9/10 使用 → 一致
   - `chat_raw` 在 Task 1 定义，Task 2 调用 → 一致
   - `execute_single` 在 Task 7 定义，Task 2 调用 → 一致

4. **无过度设计**：Persona Generation 明确不改造，符合 scope。
