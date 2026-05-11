# AgentStage 消息管理系统 V2 设计文档

> 版本：v2.0  
> 日期：2026-05-10  
> 范围：重构前后端消息管理、Prompt 组装、Tool Calling、Scheduler 调度  
> 参考：SillyTavern (PromptManager/TokenHandler/MessageCollection/ToolManager), RisuAI (PromptItem/formatingOrder/sendChat/groupOrder)

---

## 一、当前问题回顾

### 1.1 用户层面
- **发送消息无回复**：LLM 返回了 tool call，但系统没有保存 agent 回复消息
- **FOREIGN KEY 错误弹窗**：前端收到原始 SQLite 错误 `"FOREIGN KEY constraint failed"`

### 1.2 系统层面
| 问题 | 根因 | 影响 |
|------|------|------|
| Prompt 中缺失 `session_id` | `PromptAssembler` 只注入会话名称，未注入 session_id | LLM 调用 `send_message` 时无法知道正确的 `target_id`，只能编造 UUID，导致 `insert_message` FOREIGN KEY 失败 |
| 并发 Race Condition | `on_new_message` 与后台扫描同时触发 `trigger_agent` | 同一个 agent 被并行调用两次，浪费 API token |
| 消息移除时机不当 | `trigger_agent` 先 `remove` pending queue 再 `insert_message` | LLM 成功但 insert 失败时，pending 已清空，消息永久丢失，agent 再也不会回复 |
| 错误直接暴露给用户 | `map_err(|e| e.to_string())` 逐层传播原始数据库错误 | 用户看到无法理解的 FOREIGN KEY 错误 |
| 无 Token 预算管理 | `PromptAssembler` 无截断逻辑 | 历史消息无限增长，上下文超长，API 成本失控 |
| 无消息元数据 | messages 表结构过于简单 | 无法支持 swipe（多回复候选）、token 计数、可见性标记等高级功能 |

---

## 二、参考方案核心机制提炼

### 2.1 SillyTavern — 消息组装与预算管理

**Message 类** (`openai.js`):
```javascript
class Message {
    role = 'user' | 'assistant' | 'system'
    content = string
    name = string          // 角色名（function calling 用）
    token_count = number   // 预计算的 token 数
}
```

**MessageCollection 类**:
- 支持排序、截断、预算检查
- 按优先级丢弃旧消息（保留系统提示词、最近消息）

**TokenHandler 类**:
- 分配 token 预算给每个 prompt 块
- 当总 token 超过 `max_context` 时，按优先级截断

**Prompt 分块管理** (`PromptManager.js`):
```javascript
class Prompt {
    identifier: string   // 'main', 'charDescription', 'chatHistory', ...
    role: string
    content: string
    position: number     // 排序位置
    injection_depth: number  // 在 chatHistory 中的相对插入深度
}
```

**ToolManager 类** (`tool-calling.js`):
```javascript
class ToolManager {
    static RECURSE_LIMIT = 5
    static registerFunctionTool({name, displayName, description, parameters, action})
    static async handleToolCalls(response)
}
```

### 2.2 RisuAI — 分层组装与群聊调度

**PromptItem 类型系统** (`prompt.ts`):
```typescript
type PromptItem =
  | { type: 'plain' | 'jailbreak' | 'cot', role, content }
  | { type: 'persona' | 'description' | 'lorebook' | 'postEverything' | 'memory', role, content }
  | { type: 'chat', rangeStart, rangeEnd, role, content }
  | { type: 'authornote', role, content }
  | { type: 'chatML', role, content }
  | { type: 'cache', role, content }
```

**sendChat 组装逻辑** (`index.svelte.ts`):
```typescript
let unformated = {
    'main': [],          // 主系统提示词
    'jailbreak': [],     // 越狱提示词
    'chats': [],         // 聊天历史
    'lorebook': [],      // Lorebook 内容
    'globalNote': [],    // 全局备注
    'authorNote': [],    // 作者备注
    'lastChat': [],      // 最后一条消息
    'description': [],   // 角色描述
    'postEverything': [],// 后置指令
    'personaPrompt': []  // 用户人设
}
// 按 formatingOrder 拼接为最终 OpenAIChat[]
```

**群聊发言顺序** (`group.ts`):
```typescript
function groupOrder(chars: GroupOrder[], input: string): GroupOrder[] {
    // 1. 关键词匹配（消息中提到角色名则优先）
    // 2. 概率抽样（按 talkness 决定是否发言）
    // 3. 保底（至少选一个）
}
```

### 2.3 text-generation-webui — 历史消息结构

**历史消息二维数组** (`chat.py`):
```python
history = [
    [user_msg, assistant_msg, tool_msg, metadata],  # 回合 1
    [user_msg, assistant_msg, tool_msg, metadata],  # 回合 2
]
```

**关键设计**：每条消息/每个回合都有 `metadata`，存储额外信息（如是否已同步、token 数、生成参数等）。

---

## 三、新架构设计（Message System V2）

### 3.1 核心原则

1. **Prompt 是契约**：LLM 必须能在 Prompt 中直接看到所有需要使用的 ID（session_id, agent_id）
2. **触发即锁定**：一旦开始 `trigger_agent`，该 agent 的 pending 消息立即被锁定，不可被其他触发器重复处理
3. **失败可恢复**：LLM 调用成功但后续写入失败时，pending 消息必须能被恢复重试
4. **错误不外泄**：内部数据库错误必须转换为友好的业务错误，不能暴露原始 SQLite 错误
5. **Token 有预算**：历史消息必须按 token 预算截断，防止上下文无限增长

### 3.2 整体流程图

```
用户发送消息
    │
    ▼
┌─────────────────┐
│ send_user_message│  ← Tauri Command
│ 1. insert user msg│
│ 2. update session │
│ 3. scheduler.on_new_message() │
└─────────────────┘
    │
    ▼
┌─────────────────┐
│ Scheduler::on_new_message  │
│ 1. 查询 session_type, agent_id│
│ 2. 检查消息上限            │
│ 3. 重置计数器（用户消息时） │
│ 4. 推入 pending_queue       │
│ 5. try_trigger_agent()      │
└─────────────────┘
    │
    ▼
┌─────────────────┐
│ try_trigger_agent │
│ 检查 last_trigger + interval │
│ 满足 → trigger_agent()      │
│ 不满足 → 等待后台扫描       │
└─────────────────┘
    │
    ▼
┌─────────────────┐
│ trigger_agent (V2) │
│ 1. 原子锁定：取出并删除 pending  │ ← 关键改进
│ 2. 检查消息上限              │
│ 3. PromptAssembler::assemble_v2()│
│ 4. call_llm()                │
│ 5. ToolExecutor::execute()   │ ← 新组件
│ 6. 更新 trigger_time         │
│ 7. 失败时恢复 pending        │ ← 关键改进
└─────────────────┘
```

---

## 四、数据库 Schema 变更

### 4.1 messages 表增强

```sql
-- 当前字段保留，新增以下字段
ALTER TABLE messages ADD COLUMN extra TEXT DEFAULT '{}';
-- extra JSON 格式：
-- {
--   "token_count": 123,           // 本条消息的 token 数（可选）
--   "swipes": ["content1", "content2"],  // 多回复候选（ swipe 功能预留）
--   "swipe_id": 0,               // 当前选中的 swipe 索引
--   "generation_info": {          // 生成信息
--     "model": "MiniMax-M2.7",
--     "temperature": 0.7,
--     "finish_reason": "stop"
--   },
--   "tool_call": {                // 如果是 tool_call 消息
--     "name": "send_message",
--     "arguments": "{...}"
--   }
-- }
```

### 4.2 新增 chat_pages 表（多聊天页支持）

参考 RisuAI 的 `chats[]` + `chatPage` 设计：

```sql
CREATE TABLE IF NOT EXISTS chat_pages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    page_index INTEGER NOT NULL DEFAULT 0,  -- 聊天页序号
    name TEXT,                               -- 聊天页名称（如"主线","IF线"）
    is_active INTEGER DEFAULT 1 CHECK(is_active IN (0, 1)),  -- 当前是否激活
    message_count INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    UNIQUE(session_id, page_index)
);
```

**注**：Phase 2（群聊）时再启用。当前 Phase 1（私聊）保持默认 `page_index=0`。

### 4.3 新增 agent_message_views 表（Agent 独立可见历史）

实现 AgentStage 的核心差异化设计——"每个 Agent 独立维护可见消息历史"。

```sql
CREATE TABLE IF NOT EXISTS agent_message_views (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    is_visible INTEGER DEFAULT 1 CHECK(is_visible IN (0, 1)),
    viewed_at INTEGER,  -- Agent 第一次看到这条消息的时间
    created_at INTEGER NOT NULL
);

CREATE INDEX idx_agent_views_agent_session 
    ON agent_message_views(agent_id, session_id, created_at DESC);
CREATE INDEX idx_agent_views_message 
    ON agent_message_views(message_id);
```

**设计说明**：
- 当消息产生时，系统根据可见性规则决定哪些 Agent 能看到这条消息
- 私聊：双方互相可见（user 和该 session 的 agent）
- 群聊：群内所有成员可见（Phase 2）
- `get_visible_messages_for_agent()` 改为从 `agent_message_views` 查询

### 4.4 trigger_states 表增强

```sql
-- 新增字段
ALTER TABLE trigger_states ADD COLUMN is_triggering INTEGER DEFAULT 0 CHECK(is_triggering IN (0, 1));
-- is_triggering = 1 表示该 Agent 当前正在被触发中，用于防止并发触发
```

### 4.5 private_sessions 表增强

```sql
-- 当前聊天页（参考 RisuAI 的 chatPage）
ALTER TABLE private_sessions ADD COLUMN current_chat_page INTEGER DEFAULT 0;
```

---

## 五、Prompt 组装 V2（PromptAssembler）

### 5.1 设计目标

1. **在 Prompt 中明确注入所有 ID**：LLM 必须能看到 `session_id` 和 `agent_id`
2. **Token 预算管理**：按优先级截断历史消息，确保总 token 不超过 `max_context`
3. **分层组装**：参考 RisuAI 的 `unformated` + `formatingOrder` 模式
4. **变量替换**：支持 `{{char}}`, `{{user}}`, `{{group}}` 等模板变量

### 5.2 Prompt 块定义

```rust
pub struct PromptBlock {
    pub identifier: String,   // "system", "persona", "participants", "history", "pending", "instruction"
    pub role: String,         // "system" | "user"
    pub content: String,
    pub priority: i32,        // 截断优先级（越小越重要，越不容易被截断）
    pub estimated_tokens: i32,
}
```

### 5.3 组装顺序（formatingOrder）

```rust
const PROMPT_ORDER: &[&str] = &[
    "system",        // 系统指令
    "persona",       // 角色设定
    "participants",  // 参与者介绍
    "history",       // 历史聊天记录
    "pending",       // 最新消息（待回复）
    "instruction",   // 工具使用指令 + 当前上下文 ID
];
```

### 5.4 关键改进：在 instruction 块中注入 ID

```rust
// Layer 6: Instruction（最底部，紧接 pending 消息）
let instruction = format!(
    r#"【工具使用说明】
你可以使用 send_message 工具发送消息。
当前你正在以下会话中聊天：
{context_list}

请根据上下文决定是否需要回复，以及回复哪个会话。
如果需要回复，请调用 send_message 工具，参数如下：
- target_type: "private" 或 "group"
- target_id: 目标会话的 session_id（必须是上面列出的 ID 之一）
- content: 你要发送的消息内容

注意：你只能向上面列出的会话发送消息。"#,
    context_list = /* 生成所有可见会话的 ID+名称列表 */
);
```

**context_list 示例**：
```
- session_id: ff98c9a7-dc93-429d-84c3-687dc58af861, 名称: 卫宫士郎, 类型: private
```

这样 LLM 就能明确知道应该使用哪个 `target_id`。

### 5.5 Token 预算管理

```rust
pub struct TokenBudget {
    pub max_context: i32,      // 最大上下文 token 数（来自 agent.max_tokens * 2 或设置）
    pub reserved: i32,         // 为 response 预留的 token 数
    pub available: i32,        // 可用于 prompt 的 token 数 = max_context - reserved
}

impl TokenBudget {
    pub fn truncate_blocks(blocks: &mut Vec<PromptBlock>, budget: i32) {
        // 1. 按 priority 排序（priority 大的先被截断）
        // 2. 从 history 块开始截断（移除旧消息）
        // 3. 如果还不够，截断 participants 块
        // 4. 绝不动 system 和 persona 块（priority = 0）
    }
}
```

**估算策略**：
- Phase 1：使用简单字符估算（中文 1 字 ≈ 1 token，英文 1 词 ≈ 1.3 token）
- Phase 2：引入 tiktoken-rs 精确计算

---

## 六、Tool Calling V2（ToolExecutor）

### 6.1 设计目标

1. **严格参数校验**：tool call 参数必须经过校验，不合法时自动修正或拒绝
2. **自动映射 target_id**：如果 LLM 返回了 agent_id 或会话名称作为 target_id，自动映射为正确的 session_id
3. **错误隔离**：tool 执行失败不影响 LLM 调用流程

### 6.2 ToolExecutor 组件

```rust
pub struct ToolExecutor {
    db_state: DbState,
}

impl ToolExecutor {
    pub async fn execute(
        &self,
        agent_id: &str,
        tool_calls: Vec<ToolCall>,
    ) -> Result<Vec<Message>, ToolError> {
        let mut results = Vec::new();
        
        for tc in tool_calls {
            match tc.name.as_str() {
                "send_message" => {
                    let msg = self.execute_send_message(agent_id, &tc.arguments).await?;
                    results.push(msg);
                }
                _ => {
                    log::warn!("Unknown tool call: {}", tc.name);
                }
            }
        }
        
        Ok(results)
    }
    
    async fn execute_send_message(
        &self,
        agent_id: &str,
        arguments: &str,
    ) -> Result<Message, ToolError> {
        let args: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| ToolError::InvalidArguments(e.to_string()))?;
        
        let raw_target_id = args["target_id"].as_str().unwrap_or("");
        let content = args["content"].as_str().unwrap_or("");
        
        if content.is_empty() {
            return Err(ToolError::EmptyContent);
        }
        
        // 自动映射 target_id
        let target_id = self.resolve_target_id(agent_id, raw_target_id).await?;
        
        // 插入消息
        let conn = self.db_state.0.lock().await;
        let msg = message_repo::insert_message(
            &conn, &target_id, "agent", agent_id, content, "text",
        ).map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        
        Ok(msg)
    }
    
    async fn resolve_target_id(
        &self,
        agent_id: &str,
        raw: &str,
    ) -> Result<String, ToolError> {
        let conn = self.db_state.0.lock().await;
        
        // 1. 如果 raw 本身就是合法的 session_id，直接返回
        if let Ok(Some(_)) = session_repo::get_session_by_id(&conn, raw) {
            return Ok(raw.to_string());
        }
        
        // 2. 如果 raw 是 agent_id，查找对应的私聊 session
        if let Ok(Some(session)) = session_repo::get_private_session_by_agent_id(&conn, raw) {
            return Ok(session.id);
        }
        
        // 3. 如果 raw 是会话名称，查找匹配
        // TODO: Phase 2 实现
        
        // 4. 默认：使用该 agent 的默认私聊 session
        if let Ok(Some(session)) = session_repo::get_private_session_by_agent_id(&conn, agent_id) {
            return Ok(session.id);
        }
        
        Err(ToolError::TargetNotFound(raw.to_string()))
    }
}

#[derive(Debug)]
pub enum ToolError {
    InvalidArguments(String),
    EmptyContent,
    TargetNotFound(String),
    DatabaseError(String),
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolError::InvalidArguments(s) => write!(f, "工具参数格式错误: {}", s),
            ToolError::EmptyContent => write!(f, "工具调用内容为空"),
            ToolError::TargetNotFound(s) => write!(f, "找不到目标会话: {}", s),
            ToolError::DatabaseError(s) => write!(f, "保存消息失败: {}", s),
        }
    }
}
```

### 6.3 工具 Schema 增强

```rust
pub fn send_message_tool_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "send_message",
            "description": "向指定会话发送一条消息。target_id 必须是系统提供的 session_id。",
            "parameters": {
                "type": "object",
                "properties": {
                    "target_type": {
                        "type": "string",
                        "enum": ["private", "group"],
                        "description": "目标会话类型"
                    },
                    "target_id": {
                        "type": "string",
                        "description": "目标会话的 session_id（必须使用系统提供的 ID）"
                    },
                    "content": {
                        "type": "string",
                        "description": "消息内容"
                    }
                },
                "required": ["target_type", "target_id", "content"]
            }
        }
    })
}
```

---

## 七、Scheduler V2

### 7.1 设计目标

1. **原子锁定**：`trigger_agent` 开始时立即锁定该 agent，防止并发触发
2. **失败可恢复**：LLM 成功但写入失败时，恢复 pending 消息
3. **错误不传播**：`trigger_agent` 内部错误不传播到 `send_user_message`

### 7.2 关键改进

#### 改进 1：触发前原子锁定

```rust
pub async fn try_trigger_agent(&self, agent_id: &str) -> Result<(), String> {
    let conn = self.db_state.0.lock().await;
    
    // 检查是否正在触发中（防止并发）
    let is_triggering: bool = conn.query_row(
        "SELECT is_triggering FROM trigger_states WHERE agent_id = ?1",
        [agent_id],
        |row| Ok(row.get::<_, i32>(0)? != 0),
    ).unwrap_or(false);
    
    if is_triggering {
        return Ok(()); // 已经有触发在进行中，跳过
    }
    
    // 检查时间间隔
    let last_trigger = trigger_repo::get_last_trigger_time(&conn, agent_id)?;
    let settings = settings_repo::get_or_create_settings(&conn)?;
    let now = chrono::Utc::now().timestamp_millis();
    
    if now - last_trigger < settings.global_min_trigger_interval as i64 * 1000 {
        return Ok(()); // 间隔未过，等待后台扫描
    }
    
    drop(conn);
    
    self.trigger_agent(agent_id).await
}
```

#### 改进 2：trigger_agent 内部流程

```rust
pub async fn trigger_agent(&self, agent_id: &str) -> Result<(), String> {
    // === 阶段 1：原子取出 pending 消息 ===
    let pending = {
        let mut queue = self.pending_queue.lock().await;
        queue.remove(agent_id).unwrap_or_default()
    };
    
    if pending.is_empty() {
        return Ok(());
    }
    
    // 设置触发中标志
    {
        let conn = self.db_state.0.lock().await;
        conn.execute(
            "INSERT INTO trigger_states (agent_id, is_triggering, last_trigger_time, updated_at) 
             VALUES (?1, 1, ?2, ?2)
             ON CONFLICT(agent_id) DO UPDATE SET is_triggering = 1",
            (agent_id, chrono::Utc::now().timestamp_millis()),
        ).map_err(|e| e.to_string())?;
    }
    
    // === 阶段 2：检查消息上限 ===
    // ...（同前）
    
    // === 阶段 3：组装 Prompt 并调用 LLM ===
    let response = match self.call_llm(agent_id, &pending).await {
        Ok(resp) => resp,
        Err(e) => {
            // LLM 调用失败：恢复 pending 消息
            self.restore_pending(agent_id, pending).await;
            self.clear_triggering_flag(agent_id).await;
            self.emit("agent_error", json!({"agent_id": agent_id, "error": e}));
            return Ok(()); // 错误不传播
        }
    };
    
    // === 阶段 4：执行 Tool Calls ===
    let executor = ToolExecutor::new(self.db_state.clone());
    let agent_messages = match executor.execute(agent_id, response.tool_calls).await {
        Ok(msgs) => msgs,
        Err(e) => {
            // Tool 执行失败：恢复 pending 消息
            self.restore_pending(agent_id, pending).await;
            self.clear_triggering_flag(agent_id).await;
            self.emit("agent_error", json!({"agent_id": agent_id, "error": e.to_string()}));
            return Ok(());
        }
    };
    
    // === 阶段 5：更新计数器和会话预览 ===
    // ...
    
    // === 阶段 6：触发链 ===
    for msg in &agent_messages {
        self.emit("new_message", msg);
        // 推入对方 pending_queue
        // ...
    }
    
    // 清除触发标志
    self.clear_triggering_flag(agent_id).await;
    
    self.emit("agent_completed", json!({"agent_id": agent_id}));
    Ok(())
}
```

#### 改进 3：错误隔离

`send_user_message` 中：
```rust
// 触发调度器（错误不再影响 send_user_message 的返回）
let scheduler_result = scheduler.on_new_message(&req.session_id, &message).await;
if let Err(e) = scheduler_result {
    crate::logger::backend("WARN", &format!("Scheduler error (non-fatal): {}", e));
    // 不返回错误给前端，调度器错误在后台处理
}
```

---

## 八、前后端交互

### 8.1 send_user_message 返回值不变

```rust
pub async fn send_user_message(...) -> Result<Message, String> {
    // 1. 插入用户消息（失败则返回错误）
    // 2. 更新会话预览
    // 3. 异步触发 scheduler（错误不传播）
    // 4. 立即返回用户消息（前端乐观更新）
}
```

### 8.2 前端事件流

```
用户发送消息
    │
    ▼
前端乐观添加消息 ──→ 显示在聊天界面
    │
    ▼
调用 send_user_message
    │
    ▼
收到返回（成功）──→ 刷新消息列表（拉取确认）
    │
    ▼
监听 "new_message" 事件 ←── 后端触发 agent 回复
    │
    ▼
收到 agent 消息 ──→ 自动添加到聊天界面
    │
    ▼
监听 "agent_error" 事件 ←── 后端触发失败
    │
    ▼
显示 Toast 提示："角色回复失败，将在稍后重试"
```

---

## 九、开发顺序

### Phase A：数据库变更（第 1 步）
1. 新增 `agent_message_views` 表
2. `messages` 表增加 `extra` 字段
3. `trigger_states` 表增加 `is_triggering` 字段
4. `private_sessions` 表增加 `current_chat_page` 字段
5. 更新 migration

### Phase B：PromptAssembler V2（第 2 步）
1. 实现 `PromptBlock` 结构
2. 实现 TokenBudget 截断逻辑
3. 在 instruction 层注入 `session_id`
4. 实现变量替换（`{{char}}`, `{{user}}`）

### Phase C：ToolExecutor（第 3 步）
1. 创建 `ToolExecutor` 组件
2. 实现 `resolve_target_id` 自动映射
3. 严格参数校验
4. 友好的错误类型

### Phase D：Scheduler V2（第 4 步）
1. 实现 `is_triggering` 原子锁定
2. 改进 pending 消息取出时机
3. 实现失败恢复（`restore_pending`）
4. 错误隔离（不传播到 `send_user_message`）

### Phase E：可见性视图（第 5 步）
1. 实现 `agent_message_views` 的 CRUD
2. 重写 `get_visible_messages_for_agent`
3. 消息产生时自动建立视图记录

### Phase F：前端适配（第 6 步）
1. 监听 `agent_error` 事件，显示友好 Toast
2. 确保乐观更新与后端事件不冲突

---

## 十、验收标准

- [ ] 发送消息后，agent 能在 30 秒内回复（满足触发条件时）
- [ ] 不再出现 `FOREIGN KEY constraint failed` 错误
- [ ] 错误提示变为中文业务错误（如"角色回复失败，将在稍后重试"）
- [ ] 同一个 agent 不会被并发触发两次
- [ ] Prompt 中明确包含当前会话的 `session_id`
- [ ] 历史消息按 token 预算截断
- [ ] LLM 调用成功但保存失败时，消息不会丢失（会重试）
