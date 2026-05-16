# Agent-to-Agent 私聊功能 实施计划

> **For agentic workers:** 使用 subagent-driven-development 分批次执行。

**Goal:** 实现角色自主开启 Agent-Agent 私聊的 `start_private_chat` Tool，支持用户视角旁观所有角色间私聊，统一 `SessionResponse` 为 `participants` 数组。

**Architecture:** 后端新增 Tool + 改造对称私聊创建逻辑 + 修复 scheduler target 查询；前端统一 `Session` 类型为 `participants` 数组，Agent-Agent 会话固定站位 + 禁用输入。

**Tech Stack:** Tauri v2 (Rust + SQLite), Svelte 5, TypeScript, TailwindCSS v4

---

## 文件结构映射

### 后端（Rust）
| 文件 | 职责 |
|------|------|
| `src-tauri/src/models/session.rs` | `SessionResponse` 改造，`SessionParticipant` 新增 |
| `src-tauri/src/db/session.rs` | `list_sessions` / `create_private_session` / `get_private_session_by_agent_id` 通用化；查询改为 JOIN 双方 participant 信息 |
| `src-tauri/src/db/agent.rs` | 新增 `get_agent_by_name`（精确查询，is_deleted=0） |
| `src-tauri/src/llm/tool.rs` | 新增 `start_private_chat_tool_schema` + `execute_start_private_chat`；改造 `resolve_target_id` 以支持新逻辑 |
| `src-tauri/src/scheduler/mod.rs` | 修复 `get_target_agents` symmetric 私聊查询 |
| `src-tauri/src/commands/session.rs` | 适配新的 `SessionResponse` |
| `src-tauri/src/lib.rs` | 注册新的 Tauri commands（如有新增） |

### 前端（Svelte/TS）
| 文件 | 职责 |
|------|------|
| `src/lib/types.ts` | `SessionParticipant` + `Session` 改造，删除旧字段 |
| `src/lib/stores/sessionStore.svelte.ts` | 适配 `participants` 数组 |
| `src/lib/components/SessionList.svelte` | 从 `participants` 渲染；Agent-Agent 显示组合名 |
| `src/lib/components/ChatView.svelte` | Header 双头像；消息固定站位；Agent-Agent 禁用输入 |
| `src/lib/components/MessageBubble.svelte` | 可能需适配固定站位样式 |
| `src/lib/components/SessionSettingsPanel.svelte` | 适配 `participants` |
| `src/App.svelte` | `new_message` 事件增加新会话检测 |
| `src/lib/components/CreateGroupModal.svelte` | 如有引用旧字段需适配 |

---

## 实施批次

### Batch 1: 后端模型与数据库层
1. `models/session.rs`：新增 `SessionParticipant`，改造 `SessionResponse`
2. `db/session.rs`：
   - 重写 `list_sessions` 查询，支持 `participants` 数组（JSON 聚合或多次 JOIN）
   - 改造 `create_private_session` 为通用 core（支持 User-Agent 和 Agent-Agent）
   - 新增 `create_agent_agent_session`
   - 新增 `get_private_session_between_agents`
   - 更新 `row_to_session_response`
3. `db/agent.rs`：新增 `get_agent_by_name`
4. 更新 `commands/session.rs` 适配新的 `SessionResponse`
5. 更新所有现有测试（`db/session.rs` tests, `prompt.rs` tests）

### Batch 2: 后端 Tool + Scheduler
1. `llm/tool.rs`：
   - 新增 `start_private_chat_tool_schema`
   - 新增 `execute_start_private_chat`（查名→查/建会话→互加好友→写消息）
   - `call_llm` 处 tools 数组加入新 schema
2. `scheduler/mod.rs`：
   - 修复 `get_target_agents` 私聊 symmetric 查询
3. 新增 Tool 测试

### Batch 3: 后端集成测试与审查
1. `cargo check --tests` 确保无编译错误
2. 运行 Rust 单元测试（如环境允许）
3. 审查所有改动点

### Batch 4: 前端类型与 Store
1. `types.ts`：定义 `SessionParticipant`，改造 `Session`
2. `sessionStore.svelte.ts`：适配 `participants`
3. 更新所有引用旧字段的辅助函数

### Batch 5: 前端 UI 改造
1. `SessionList.svelte`：从 `participants` 取信息；Agent-Agent 组合名称
2. `ChatView.svelte`：
   - Header 双头像渲染
   - 消息气泡 `isOnRightSide` 固定站位逻辑
   - Agent-Agent 禁用输入框
   - typing indicator 适配
3. `App.svelte`：`new_message` 新增会话检测
4. `SessionSettingsPanel.svelte` 等引用旧字段的文件适配

### Batch 6: 前端测试
1. 更新 Vitest 测试（已有测试引用旧类型）
2. Playwright E2E 测试（如环境允许）

---

## 不确定点记录（决策备忘）

见 `docs/superpowers/uncertainties/2026-05-16-agent-to-agent-chat.md`
