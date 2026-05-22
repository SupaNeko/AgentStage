# Prompt 结构优化设计文档

> 日期：2026-05-22  
> 目标：优化 Prompt 组装方式，提升工具调用能力，支持多轮工具执行

---

## 1. 背景与目标

### 1.1 当前问题

1. **工具描述冗长**：系统 Prompt 中包含了大量工具使用说明（参数、示例、注意事项），挤占了有效上下文空间。
2. **单轮限制**：当前 LLM 调用为单轮模式。模型一次性输出所有 tool_calls，执行后不再将结果回传给模型。若模型需要同时处理"发消息"和"更新记忆"，但受限于单轮只能完成部分操作，剩余操作被丢弃。
3. **Prompt 结构单一**：所有内容拼接为一个大字符串作为 system prompt，未利用 OpenAI API 的 `system` / `user` 多 content 能力。

### 1.2 优化目标

1. **工具描述归位**：将详细工具说明迁移到各工具 Schema 的 `description` 中；系统 Prompt 仅保留一句话功能简介。
2. **Prompt 拆分**：利用模型 API 的多 content 能力，将静态规则 + 角色设定放入 `system` content，动态上下文（聊天记录、工具说明等）放入 `user` content。
3. **多轮工具调用**：支持最多 5 轮对话。每轮执行工具后将结果通过标准 OpenAI `role: "tool"` 消息回传，让模型决定是否需要继续调用。`tool_calls` 为空时自动结束。

---

## 2. 架构设计

### 2.1 核心模块：`LlmConversation`

新增 `src-tauri/src/llm/conversation.rs`，封装统一的多轮对话管理器。

```rust
pub struct PromptParts {
    pub system: String,
    pub user: String,
}

pub struct ExecutedToolCall {
    pub tool_call: crate::llm::tool::ToolCall,
    pub result: ToolExecutionResult,
}

pub enum ToolExecutionResult {
    Success(String),
    Error(String),
}

pub struct ConversationResult {
    pub final_content: Option<String>,
    pub executed_tool_calls: Vec<ExecutedToolCall>,
    pub total_rounds: usize,
}

pub struct LlmConversation {
    provider: crate::llm::openai::OpenAiCompatibleProvider,
    db_state: crate::db::connection::DbState,
    scheduler: crate::scheduler::Scheduler,
}

impl LlmConversation {
    pub fn new(
        provider: crate::llm::openai::OpenAiCompatibleProvider,
        db_state: crate::db::connection::DbState,
        scheduler: crate::scheduler::Scheduler,
    ) -> Self;

    /// 统一入口：运行多轮对话，最多 max_rounds 轮
    pub async fn run(
        &self,
        system: &str,
        initial_user_content: &str,
        tools: Vec<serde_json::Value>,
        max_rounds: usize,
        agent_id: &str,
        session_pages: &std::collections::HashMap<String, i32>,
    ) -> Result<ConversationResult, String>;
}
```

### 2.2 Provider 层改造

`OpenAiCompatibleProvider` 新增底层方法 `chat_raw`，旧 `chat` 作为兼容包装器：

```rust
impl OpenAiCompatibleProvider {
    /// 旧接口保留（供非多轮场景临时使用）
    pub async fn chat(
        &self,
        system_prompt: &str,
        messages: Vec<serde_json::Value>,
        tools: Vec<serde_json::Value>,
    ) -> Result<LlmResponse, String>;

    /// 新底层接口：直接发送完整的 messages 数组
    pub async fn chat_raw(
        &self,
        messages: Vec<serde_json::Value>,
        tools: Vec<serde_json::Value>,
    ) -> Result<LlmResponse, String>;
}
```

### 2.3 数据流

```
PromptAssembler::assemble() ──► PromptParts { system, user }
                                       │
                                       ▼
                    ┌──────────────────────────────────┐
                    │   LlmConversation::run()         │
                    │   - 构建 messages[system, user]  │
                    │   - Round 0: provider.chat_raw() │
                    │   - 有 tool_calls?               │
                    │     ├─ Yes → ToolExecutor 执行   │
                    │     │      追加 assistant + tool │
                    │     │      Round++ (max 5)       │
                    │     └─ No  → 返回 ConversationResult
                    └──────────────────────────────────┘
```

---

## 3. Prompt 拆分策略

### 3.1 聊天场景（`trigger_chat` / `trigger_special`）

`PromptAssembler::assemble()` 返回 `PromptParts`。

**system content**（静态规则 + 角色设定）：
- `SYSTEM_PROMPT`（精简后）
- `TIMER_CAPABILITY`
- `【你的角色设定】` + `detailed_persona`

**user content**（动态上下文 + 极简工具说明）：
- `【等待中的定时任务】`（如有）
- `【关于你的记忆】`（如有）
- `【你认识的参与者】`
- `【历史聊天记录】`
- `【工具使用说明】`（仅含 `context_list` + 一句话提示）

### 3.2 溢出摘要场景（`overflow_summary`）

`SUMMARY_SYSTEM_PROMPT` 按 `## 你的角色设定` 切分：

- **system content**：`你是一个记忆整理助手...` 的静态规则 + `## 你的角色设定\n{detailed_persona}`
- **user content**：`## 关于你的记忆\n{long_term_memory}\n## 你认识的参与者\n{participants}\n## 本次对话记录\n{session_messages}\n\n请回顾本次对话，判断是否有值得保存到记忆中的信息。`

---

## 4. 工具描述迁移

### 4.1 系统 Prompt 精简

`SYSTEM_PROMPT` 中的工具列表从详细说明改为一行一句话：

```
## 6. 可用工具
- send_message：向指定会话发送消息
- start_private_chat：向某个角色发起私聊
- update_relationship：更新你对某个参与者的关系描述
- update_memory：更新你的记忆
- create_timer：创建一个定时任务
- delete_timer：删除一个定时任务
```

### 4.2 工具说明模板精简

`TOOL_INSTRUCTION_TEMPLATE` 移除所有工具的详细参数说明、示例、注意事项，仅保留：

```
当前你正在以下会话中聊天：
{context_list}

你可以使用上述工具与其他人互动。各工具的详细用法和规则已在你可用的函数描述中提供，请仔细阅读并遵守。
```

### 4.3 工具 Schema 描述扩充

将原 `TOOL_INSTRUCTION_TEMPLATE` 中的详细规则、示例、注意事项全部迁移到对应工具的 `description` 字段：

- `send_message`：补充 `target_id` 必须使用完整 session_id、只能回复列出的会话等规则
- `start_private_chat`：补充对方名称精确匹配、调用成功后获得新会话等规则
- `update_relationship`：补充 `old_text` 精确匹配、200字限制、静态关系定位 vs 动态记忆的区别、示例
- `update_memory`：补充 `self`/`other` 区别、`old_text` 精确匹配、字数限制、示例
- `create_timer`：补充单次/循环触发方式说明
- `delete_timer`：补充任务ID来源说明

---

## 5. 多轮对话详细流程

### 5.1 算法

```
初始化 messages = [system, user]
for round in 0..max_rounds:
    response = provider.chat_raw(messages, tools)
    
    // 必须追加 assistant message（含 tool_calls），供后续 tool 消息关联
    messages.push(assistant_message)
    
    if response.tool_calls.is_empty():
        final_content = response.content
        break  // 正常结束
    
    for tc in response.tool_calls:
        result = executor.execute_single(agent_id, tc, session_pages)
        messages.push(tool_message { role: "tool", tool_call_id: tc.id, content: result })
        executed.push({ tc, result })
    
    if round == max_rounds - 1:
        log WARN "达到上限"
        break  // 强制结束
```

### 5.2 关键约束

- **Assistant message 必须保留**：标准 OpenAI Tool Calling 要求 `role: "tool"` 消息之前必须有对应的 `role: "assistant"` 消息包含 `tool_calls` 字段。
- **Tool result content**：成功返回 `"执行成功: ..."`，失败返回 `"执行失败: ..."`，模型据此决定下一轮行为。
- **单工具失败不阻断同轮其他工具**。

### 5.3 调用点改造

| 场景 | 改造方式 |
|------|---------|
| `trigger_chat` | `PromptAssembler::assemble()` → `LlmConversation::run(&parts.system, &parts.user, tools, 5, ...)` |
| `trigger_special` | `PromptAssembler::assemble()` → `user = format!("{}\n\n{}", parts.user, special_layer)` → `LlmConversation::run(&parts.system, &user, tools, 5, ...)` |
| `overflow_summary` | `build_summary_system()` + `build_summary_user()` → `LlmConversation::run(&system, &user, tools, 5, ...)` |

---

## 6. 错误处理

| 场景 | 处理策略 |
|------|---------|
| Round 0 LLM 调用失败 | 直接返回 `Err`，不执行任何工具 |
| 第 N 轮 LLM 调用失败（N ≥ 1） | 前 N-1 轮已执行的工具保留，返回 `Err` |
| 单个工具执行失败 | 错误信息回传给模型，不阻断同轮其他工具 |
| 达到 5 轮上限 | 强制终止，返回已执行结果，日志记录 `WARN` |
| 模型返回空 content + 空 tool_calls | 视为正常结束 |
| 模型返回 content + 同时有 tool_calls | 同时保留，最终 content 以最后一轮为准 |
| OpenAI 返回 tool_call JSON 解析失败 | 视为该工具执行失败，错误回传 |
| Persona Generation | 暂不改造，继续使用旧 `chat()` 接口 |

---

## 7. 测试策略

### 7.1 `LlmConversation` 单元测试

使用 mock provider + mock executor：

- `test_zero_round_no_tools`：LLM 首次即返回空 tool_calls → 验证 total_rounds=1, executed=0
- `test_one_round_single_tool`：Round0 1个tool → Round1 空 → 验证 total_rounds=2, executed=1
- `test_two_rounds_multiple_tools`：Round0 2个 → Round1 1个 → Round2 空 → 验证 total_rounds=3, executed=3
- `test_max_rounds_limit`：LLM 每轮固定返回 tool_call → 验证 total_rounds=5, executed=5
- `test_tool_execution_error_propagation`：Round0 失败 → Round1 空 → 验证错误正确回传

### 7.2 `PromptAssembler` 测试更新

- 断言 `system` 包含 `SYSTEM_PROMPT` + `【你的角色设定】`
- 断言 `user` 包含 `【历史聊天记录】` + `【工具使用说明】`
- 断言 `system` 不包含详细工具参数说明

### 7.3 编译检查

- `cargo check` 0 错误
- `svelte-check` 不受影响

---

## 8. 改造文件清单

| 文件 | 改造内容 |
|------|---------|
| `src-tauri/src/llm/conversation.rs` | **新增**：`LlmConversation`、`PromptParts`、`ConversationResult` |
| `src-tauri/src/llm/mod.rs` | 导出 `conversation` 模块 |
| `src-tauri/src/llm/openai.rs` | 新增 `chat_raw()`；旧 `chat()` 改为包装器 |
| `src-tauri/src/llm/prompt.rs` | `assemble()` 返回 `PromptParts`；`build_instruction()` 精简 |
| `src-tauri/src/llm/prompt_templates.rs` | `SYSTEM_PROMPT` 精简工具列表；`TOOL_INSTRUCTION_TEMPLATE` 移除详细说明 |
| `src-tauri/src/llm/tool.rs` | 各 schema `description` 扩充；新增 `execute_single()` |
| `src-tauri/src/scheduler/mod.rs` | 3 个调用点改为 `LlmConversation::run` |

---

## 9. 术语

- **Round**：一次完整的 LLM 请求-响应周期。Round 0 为初始调用，后续为延续调用。
- **PromptParts**：`{ system, user }` 结构，代表拆分后的两段内容。
- **ConversationResult**：多轮对话结束后的汇总结果，包含最终文本、已执行工具、总轮次。
