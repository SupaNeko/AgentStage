# Phase 2: 1-on-1 私聊核心 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development.

**Goal:** 实现 AgentStage 的 1-on-1 私聊核心功能：会话创建、用户发消息、角色自动回复（LLM 调用 + Tool 执行）、Prompt 组装、全局触发间隔、消息上限保护、SSE 实时推送。

**Architecture:** 后端新增 Session/Message/Settings Repository → Tauri Commands → OpenAI-compatible LLM Provider → Prompt Assembler → Scheduler（内存 pending_queue + tokio sleep 定时器 + 后台检查任务）→ SSE 事件推送。前端新增 SessionList + ChatView 组件。

**Tech Stack:** Rust (Tauri v2, rusqlite, tokio, reqwest), Svelte 5, TypeScript, Tailwind v4

---

## 设计约束（所有任务共享）

1. **LLM Provider**：Phase 2 只实现 `OpenAI-compatible` provider（覆盖 OpenAI/Kimi/MiniMax/Custom）。Anthropic 专属格式标记为 P1。
2. **Prompt 格式**：PromptAssembler 五层拼接。历史消息格式 `[HH:MM] 发送者名称: 内容`。最新消息标注来源会话。
3. **调度器**：全局 Scheduler，内存 `HashMap<agent_id, Vec<PendingMessage>>`。`tokio::spawn` + `sleep` 做延迟触发，每 5 秒后台任务扫描 pending_queue。
4. **send_message Tool 触发链**：Tool 执行写入消息表 → 走正常消息触发逻辑（私聊触发对方角色）。
5. **错误处理**：LLM API 超时 60s，重试 3 次（指数退避 1s→2s→4s）。失败时 SSE 推送 `agent_error` 事件。
6. **Cargo 依赖**：添加 `reqwest = { version = "0.12", features = ["json"] }` 和 `async-trait = "0.1"`。
7. **SSE 事件**：`new_message`, `agent_triggered`, `agent_completed`, `agent_error`, `system_notice`。
8. **消息上限计数器**：`send_message` Tool 执行成功后递增 `agent_message_count`。用户发消息后自动重置为 0。
9. **DbState 改为 Arc**：`DbState(pub Arc<tokio::sync::Mutex<Connection>>)` 实现 Clone，供 Scheduler 后台任务使用。

---

## 文件结构

### 后端
- `src/models/session.rs` — Session/PrivateSession/SessionResponse/CreatePrivateSessionRequest
- `src/models/message.rs` — Message/MessageResponse/SendMessageRequest
- `src/models/settings.rs` — AppSettings
- `src/db/session.rs` — Session repo（创建私聊、列表、详情、软删除、更新最后消息）
- `src/db/message.rs` — Message repo（插入、按会话查询、可见消息、pending 消息）
- `src/db/settings.rs` — Settings repo（读取/创建默认设置）
- `src/db/trigger_state.rs` — TriggerState repo（读取/更新 last_trigger_time）
- `src/commands/session.rs` — create_private_session, list_sessions, get_session, delete_session
- `src/commands/message.rs` — send_user_message, get_session_messages
- `src/llm/mod.rs` — LLM 模块导出
- `src/llm/provider.rs` — `LlmProvider` async trait
- `src/llm/openai.rs` — OpenAI-compatible provider（chat completions + tool parsing）
- `src/llm/prompt.rs` — `PromptAssembler`（五层 Prompt 拼接）
- `src/llm/tool.rs` — `send_message` tool schema + `ToolCall` + `LlmResponse`
- `src/scheduler/mod.rs` — `Scheduler`（pending_queue、触发决策、延迟定时器、后台扫描任务、Tool 执行、SSE 发射）
- `src/lib.rs` — 注册 commands、管理 DbState + Scheduler、启动后台任务

### 前端
- `src/lib/types.ts` — Session, Message, Settings TypeScript 类型
- `src/lib/stores/sessionStore.svelte.ts` — 会话列表状态
- `src/lib/stores/messageStore.svelte.ts` — 当前会话消息状态
- `src/lib/components/SessionList.svelte` — 中间列会话列表
- `src/lib/components/ChatView.svelte` — 右侧聊天界面（消息流 + 输入框）
- `src/lib/components/MessageBubble.svelte` — 消息气泡
- `src/App.svelte` — 集成 SessionList 和 ChatView

---

## Task 1: 数据模型（Session / Message / Settings）

**Files:** Create `src/models/session.rs`, `src/models/message.rs`, `src/models/settings.rs`; Modify `src/models/mod.rs`

**Requirements:**
- `Session` 结构体映射 `sessions` 表所有字段（含 is_deleted bool 转换）
- `PrivateSession` 结构体映射 `private_sessions` 表
- `SessionResponse` 统一返回格式（含 agent_id/agent_name/agent_avatar/group_name/group_avatar/mute_enabled，私聊/群聊各自字段可为 None）
- `CreatePrivateSessionRequest` 只含 `agent_id: String`
- `Message` 映射 `messages` 表
- `MessageResponse` 含 sender_name（前端展示用）
- `SendMessageRequest` 含 `session_id` + `content`
- `AppSettings` 映射 `app_settings` 表（bool 从 i32 转换）
- 更新 `models/mod.rs` 导出三个新模块

**Verification:** `cargo check` passes.

---

## Task 2: Session Repository

**Files:** Create `src/db/session.rs`; Modify `src/db/mod.rs`

**Requirements:**
- `create_private_session(conn, agent_id)`：事务中插入 sessions + private_sessions，返回 SessionResponse
- `get_session_by_id(conn, session_id)`：LEFT JOIN agents/group_sessions，返回 SessionResponse
- `list_sessions(conn)`：所有未删除会话，按 last_message_at DESC
- `soft_delete_session(conn, session_id)`：更新 is_deleted=1, deleted_at=now
- `update_session_last_message(conn, session_id, preview)`：更新 last_message_at/last_message_preview/updated_at
- `db/mod.rs` 添加 `pub mod session;`

**Verification:** `cargo check` passes.

---

## Task 3: Message Repository

**Files:** Create `src/db/message.rs`; Modify `src/db/mod.rs`

**Requirements:**
- `insert_message(conn, session_id, sender_type, sender_id, content, message_type)`：插入并返回 Message
- `get_message_by_id(conn, id)`：查询单条消息
- `get_messages_by_session(conn, session_id, limit, offset)`：按时间倒序分页
- `get_visible_messages_for_agent(conn, agent_id)`：该角色参与的所有会话的消息，按 created_at 正序
- `get_pending_messages_for_agent(conn, agent_id, last_trigger_time)`：`created_at > last_trigger_time` 且排除 `sender_id = agent_id`
- `db/mod.rs` 添加 `pub mod message;`

**Verification:** `cargo check` passes.

---

## Task 4: Settings & TriggerState Repository

**Files:** Create `src/db/settings.rs`, `src/db/trigger_state.rs`; Modify `src/db/mod.rs`

**Requirements:**
- Settings：`get_or_create_settings(conn)` 读取 id=1，不存在则插入默认值并重新读取。bool 字段从 i32 转换。
- TriggerState：`get_last_trigger_time(conn, agent_id)` 返回 i64，不存在返回 0；`update_trigger_time(conn, agent_id)` UPSERT；`init_trigger_state(conn, agent_id)` INSERT OR IGNORE。
- `db/mod.rs` 添加 `pub mod settings; pub mod trigger_state;`

**Verification:** `cargo check` passes.

---

## Task 5: Session & Message Tauri Commands

**Files:** Create `src/commands/session.rs`, `src/commands/message.rs`; Modify `src/commands/mod.rs`, `src/lib.rs`

**Requirements:**
- `create_private_session(state, CreatePrivateSessionRequest)` → SessionResponse
- `list_sessions(state)` → Vec<SessionResponse>
- `get_session(state, id)` → Option<SessionResponse>
- `delete_session(state, id)` → bool
- `send_user_message(state, SendMessageRequest)` → Message：插入 user 消息，更新会话预览，**不直接触发调度器**（调度器在 Task 8 集成，此处留 TODO 注释）
- `get_session_messages(state, session_id, limit, offset)` → Vec<Message>
- `commands/mod.rs` 导出两个新模块
- `lib.rs` 在 `generate_handler!` 中注册 6 个新命令

**Verification:** `cargo check` passes.

---

## Task 6: LLM Provider 抽象层 + OpenAI-compatible Provider

**Files:** Create `src/llm/mod.rs`, `src/llm/provider.rs`, `src/llm/openai.rs`, `src/llm/tool.rs`; Modify `src/lib.rs`, `Cargo.toml`

**Requirements:**
- Cargo.toml 添加 `reqwest = { version = "0.12", features = ["json"] }` 和 `async-trait = "0.1"`
- `tool.rs`：定义 `send_message_tool_schema()` 返回 serde_json::Value；定义 `ToolCall { id, name, arguments }`；定义 `LlmResponse { content, tool_calls, usage }`
- `provider.rs`：`#[async_trait] trait LlmProvider { async fn chat(&self, system_prompt, messages, tools) -> Result<LlmResponse, String>; }`
- `openai.rs`：`OpenAiCompatibleProvider` 结构体含 client/api_key/base_url/model/temperature/max_tokens；`new()` 方法（base_url 默认 `https://api.openai.com/v1`）；`chat()` 方法构造 OpenAI Chat Completions 请求体（含 tools + tool_choice="auto"），解析 choices[0].message 中的 content 和 tool_calls，返回 LlmResponse。HTTP 超时 60s。
- `llm/mod.rs` 导出所有子模块
- `lib.rs` 添加 `pub mod llm;`

**Verification:** `cargo check` passes.

---

## Task 7: Prompt Assembler

**Files:** Create `src/llm/prompt.rs`

**Requirements:**
- `PromptAssembler::assemble(conn, agent_id, pending_messages)` → String
- 五层结构严格拼接：
  1. System Prompt（全局硬编码常量）：说明角色是 IM 参与者，可同时参与多会话，根据上下文判断回复哪个会话，可多次调用 send_message
  2. 自身人设：`agent.detailed_persona`
  3. 参与者简介：查询 `friendships` + `agents`，使用 `simplified_persona`，标注好友/群友关系，最后追加 "用户（真实用户）：正在与你聊天的真实用户"
  4. 历史消息：调用 `message_repo::get_visible_messages_for_agent`，按会话分组，格式 `[HH:MM] 发送者: 内容`
  5. 最新消息：pending_messages 单独高亮，格式 `[HH:MM] 发送者 在 会话名 中说：内容`
- 辅助方法：`get_session_name(conn, session_id)`、`get_sender_name(conn, sender_type, sender_id)`、`format_time(timestamp_ms)` → `%H:%M`
- 若可见消息为空，历史消息层省略

**Verification:** `cargo check` passes.

---

## Task 8: Scheduler（调度器 + 反循环 + Tool 执行 + SSE）

**Files:** Create `src/scheduler/mod.rs`; Modify `src/db/connection.rs`, `src/lib.rs`, `src/commands/message.rs`

**Requirements:**
- **DbState 改为 Arc**：`connection.rs` 中 `DbState(pub Arc<tokio::sync::Mutex<Connection>>)`，实现 `#[derive(Clone)]`，`init_db` 返回 `Ok(DbState(Arc::new(Mutex::new(conn))))`
- **Scheduler 结构体**：`pending_queue: Arc<Mutex<HashMap<String, Vec<PendingMessage>>>>`, `app_handle: Arc<Mutex<Option<AppHandle>>>, `db_state: DbState`
- **`PendingMessage`**：含 session_id/sender_type/sender_id/content/created_at，实现 `From<Message>`
- **`Scheduler::new(db_state)`**：初始化空队列
- **`set_app_handle(handle)`**：供 setup 阶段注入
- **`on_new_message(session_id, message)`**：获取 conn 查出私聊的 agent_id；若 sender_type=="user" 则重置该私聊的 agent_message_count；将消息加入 agent 的 pending_queue；调用 `try_trigger_agent(agent_id)`
- **`try_trigger_agent(agent_id)`**：获取 conn 读取 last_trigger_time 和 global_interval；若间隔满足，调用 `trigger_agent(agent_id)`；若不满足，计算 wait_ms，spawn tokio 任务 sleep 后调用 `trigger_agent(agent_id)`
- **`trigger_agent(agent_id)`**（核心）：
  - 读取阶段（获取 conn 锁）：取出 pending 消息；读取 agent 配置和 settings；检查消息上限（查询 private_sessions.agent_message_count vs COALESCE(message_limit, settings.private_message_limit_default)，若上限满足且 enabled，则 emit_system_notice 并返回）；更新 trigger_states.last_trigger_time；调用 PromptAssembler 组装 prompt；获取 agent 的 api_key_encrypted 并解密。
  - 释放 conn 锁
  - LLM 阶段（无锁）：emit_agent_triggered；调用 provider.chat() 带 send_message tool schema；重试 3 次，指数退避
  - 写入阶段（重新获取 conn 锁）：若 LLM 返回 tool_call 且 name=="send_message"，解析 arguments JSON，提取 target_type/target_id/content；调用 message_repo::insert_message 写入 agent 消息；递增对应 private_sessions 的 agent_message_count；更新会话最后消息预览；emit_new_message；**递归调用 `on_new_message(target_id, &agent_msg)` 继续触发链**
  - emit_agent_completed（无论成功失败）
  - 若 LLM 调用最终失败，emit_agent_error
- **后台扫描任务**：在 `lib.rs` setup 中启动 `tokio::spawn` + `interval(Duration::from_secs(5))`，每次遍历 pending_queue 中所有 agent_id，检查间隔，满足则调用 `trigger_agent`
- **SSE 发射方法**：`emit_new_message`, `emit_agent_triggered`, `emit_agent_completed`, `emit_agent_error`, `emit_system_notice`（通过 `app_handle.emit()`）
- **`send_user_message` Command 集成**：去掉 TODO，调用 `scheduler.on_new_message(&req.session_id, &message).await`
- `lib.rs` 注册 Scheduler 为 managed state

**关键设计点：**
- `trigger_agent` 必须在读取配置和写入结果时分别获取 conn 锁，中间 LLM 调用必须无锁，避免阻塞其他操作
- `db_state` 必须 Clone 以便后台任务和 Scheduler 内部共享

**Verification:** `cargo check` passes.

---

## Task 9: 前端 TypeScript 类型 + API 封装

**Files:** Modify `src/lib/types.ts`；Create `src/lib/api.ts`（或直接在组件中使用 invoke）

**Requirements:**
- `types.ts` 扩展：
  - `Session` 接口：id, session_type, last_message_at, last_message_preview, unread_count, agent_id?, agent_name?, agent_avatar?, group_name?, group_avatar?, mute_enabled?
  - `Message` 接口：id, session_id, sender_type, sender_id, sender_name?, content, created_at, message_type
- 组件中直接使用 `invoke`（不额外封装 api.ts，保持与现有 Agent CRUD 一致的风格）

**Verification:** `npx svelte-check --tsconfig ./tsconfig.json` passes (或至少类型不报错)

---

## Task 10: SessionList 组件

**Files:** Create `src/lib/components/SessionList.svelte`

**Requirements:**
- 接收会话列表，展示头像（agent_avatar 或默认）、名称（agent_name 或 group_name）、最后消息预览、时间、未读徽章
- 点击会话触发 `appState.selectSession(session.id)` + `appState.switchView('chat')`
- 空状态："还没有会话，去角色列表创建一个吧"
- 顶部标题"会话列表"+ 搜索框（按名称过滤，可选，MVP 可简化）
- 在 `App.svelte` 的 `currentView === 'chat'` 分支中替换占位符为 `<SessionList />`

**Verification:** 界面渲染正常（通过 `pnpm tauri dev` 目视检查）

---

## Task 11: ChatView 组件

**Files:** Create `src/lib/components/ChatView.svelte`, `src/lib/components/MessageBubble.svelte`

**Requirements:**
- `ChatView`：
  - 顶部标题栏：显示对方角色名称/头像
  - 中间消息流：加载当前会话消息（调用 `get_session_messages`，limit=50, offset=0），按时间正序展示
  - 底部输入框：textarea，Enter 发送（Shift+Enter 换行），发送按钮
  - 发送时调用 `send_user_message`，成功后清空输入框，重新加载消息列表
  - 监听 SSE 事件 `new_message`：若 session_id 匹配当前会话，追加到消息列表
- `MessageBubble`：
  - 用户消息：靠右，蓝色气泡
  - 角色消息：靠左，白色/灰色气泡，显示角色头像和名称
  - 系统消息：居中，灰色小字
  - 显示时间戳 `%H:%M`
  - 连续消息可合并头像（可选优化）
- 在 `App.svelte` 的 `currentView === 'chat'` 主内容区替换占位符为 `<ChatView />`

**Verification:** 界面渲染正常，能发送消息并显示

---

## Task 12: App.svelte 集成 + 状态管理完善

**Files:** Modify `src/App.svelte`, `src/lib/stores/appState.svelte.ts`

**Requirements:**
- `appState.svelte.ts`：
  - 增加 `sessionList` 数组和 `loadSessions()` 方法（调用 `list_sessions`）
  - 增加 `currentMessages` 数组和 `loadMessages(sessionId)` 方法
  - `switchView('chat')` 时自动 `loadSessions()`
- `App.svelte`：
  - chat 视图：中间列 `<SessionList />`，主内容区 `<ChatView />`
  - agents 视图保持现有 AgentList + AgentDetail
  - 在 `onMount` 中监听 SSE 事件：
    - `new_message`：更新对应会话的消息和预览
    - `system_notice`：toast 通知或 inline 提示
    - `agent_error`：显示错误提示

**Verification:** `pnpm tauri dev` 启动后，能切换视图、创建会话、进入聊天、发送消息

---

## 自审检查表

- [x] Spec 覆盖：所有 Phase 2 PRD 功能（CHAT-01~04, CHAT-10~14, CHAT-06, SES-01）都有对应任务
- [x] 无 Placeholder：所有步骤都有明确的实现要求
- [x] 类型一致性：SessionResponse、Message、PendingMessage 等类型在所有任务中一致
- [x] 安全：API Key 仍只在后端处理，前端不可见
- [x] 反循环：Task 8 中实现了消息上限检查 + 时间间隔检查 + sender 排除

---

*计划完成*
