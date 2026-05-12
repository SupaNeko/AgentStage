# 会话配置面板设计文档

> 为私聊和群聊提供统一的配置面板，支持历史提示条数、消息限制、禁言、成员管理、重置会话和解散群聊。

---

## 一、需求概述

在 ChatView 右上角添加配置按钮，点击后从页面最右侧滑出配置面板（抽屉式，非弹窗）。面板覆盖在群聊成员列表之上。

### 1.1 配置项清单

| # | 配置项 | 私聊 | 群聊 | 默认值 | 说明 |
|---|--------|------|------|--------|------|
| 1 | 历史提示条数 | ✅ | ✅ | 私聊 30 / 群聊 80 | 影响 Prompt 组装时该会话历史消息的最大条数 |
| 2 | 自动消息限制 | ✅ | ✅ | 私聊 10 / 群聊 30 | 角色消息条数上限，可开关 |
| 3 | 禁言 | ✅ | ✅ | 关闭 | 开启后角色不会自动回复，用户仍可发送消息 |
| 4 | 成员管理 | ❌ | ✅ | — | 增删群聊成员 |
| 5 | 重置会话 | ✅ | ✅ | — | 归档当前聊天记录，以相同成员开启新会话 |
| 6 | 解散群聊 | ❌ | ✅ | — | 群聊从列表移除，聊天记录保留在历史记录 |

---

## 二、数据库架构（V4 迁移）

### 2.1 新建 `session_settings` 表

统一存放所有会话级配置，取代分散在 `private_sessions` 和 `group_sessions` 中的配置字段。

```sql
CREATE TABLE IF NOT EXISTS session_settings (
    session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
    history_limit INTEGER,
    message_limit INTEGER,
    message_limit_enabled INTEGER DEFAULT 1 CHECK(message_limit_enabled IN (0, 1)),
    mute_enabled INTEGER DEFAULT 0 CHECK(mute_enabled IN (0, 1)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
```

### 2.2 迁移现有配置数据

V4 迁移脚本从 `private_sessions` 和 `group_sessions` 中提取 `message_limit`、`message_limit_enabled`、`mute_enabled`（群聊），写入 `session_settings`。旧表中的字段保留但不再被新代码读取。

### 2.3 `messages` 表添加分页关联

```sql
ALTER TABLE messages ADD COLUMN page_index INTEGER DEFAULT 0;
```

现有消息默认 `page_index = 0`。

### 2.4 `chat_pages` 表初始化

为每个已有 session 自动插入一条默认 page 记录（`page_index = 0`），与现有消息对应。

**重置会话流程**：创建新的 `chat_page`（`page_index` 递增），更新 `private_sessions.current_chat_page` / `group_sessions.current_chat_page`，后续新消息写入新的 `page_index`。加载消息时只查询当前 `page_index` 的消息。

---

## 三、后端 API 设计

### 3.1 新增模型

```rust
// src-tauri/src/models/session.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub session_id: String,
    pub history_limit: i32,
    pub message_limit: i32,
    pub message_limit_enabled: bool,
    pub mute_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateSessionConfigRequest {
    pub session_id: String,
    pub history_limit: Option<i32>,
    pub message_limit: Option<i32>,
    pub message_limit_enabled: Option<bool>,
    pub mute_enabled: Option<bool>,
}
```

### 3.2 新增 Tauri 命令

| 命令 | 参数 | 返回值 | 说明 |
|------|------|--------|------|
| `get_session_config` | `session_id: String` | `SessionConfig` | 读取会话配置，不存在时返回默认值 |
| `update_session_config` | `req: UpdateSessionConfigRequest` | `()` | 更新会话配置，仅修改传入的字段 |
| `reset_session` | `session_id: String` | `String` | 返回新 page 的 id |
| `disband_group` | `session_id: String` | `()` | 软删除群聊 |
| `add_group_member` | `session_id: String, agent_id: String` | `()` | 添加成员并更新 friendships |
| `remove_group_member` | `session_id: String, agent_id: String` | `()` | 物理删除 group_members 记录 |

### 3.3 关键行为

- **`reset_session`**：
  1. 查询当前 session 的最大 `page_index`
  2. 创建新的 `chat_page`（`page_index + 1`，`name` 可选）
  3. 更新 `private_sessions.current_chat_page` / `group_sessions.current_chat_page`
  4. 重置 `agent_message_count = 0`
  5. 返回新 page 的 id

- **`disband_group`**：
  1. 将 `sessions.is_deleted = 1`，`deleted_at = now()`
  2. 保留所有聊天记录和 members 记录

- **`add_group_member`**：
  1. 插入 `group_members`（`participant_type = 'agent'`）
  2. 如该 agent 与群聊中其他 agent 不存在 friendship，自动创建双向 friendship

- **`remove_group_member`**：
  1. 从 `group_members` 物理删除记录
  2. 不删除 friendships（保留历史关系）

---

## 四、前端 UI/UX 设计

### 4.1 布局结构

- **ChatView Header 右上角**新增齿轮图标（`Settings` from lucide-svelte）
- 点击后右侧滑出 `SessionSettingsPanel.svelte`，宽度 `w-72`，带 `translate-x` 滑入动画（约 200ms ease-out）
- 面板覆盖在群聊成员列表之上（`z-50`），点击面板外部或再次点击齿轮关闭

### 4.2 配置项排列

从上到下依次为：

1. **历史提示条数**
   - 数字输入框（`type="number"`），范围 1-200
   - 标签旁带问号图标 hover 提示："角色在 Prompt 中能看到该会话的最近 N 条消息"

2. **自动消息限制**
   - 数字输入框 + 右侧 Toggle 开关
   - 开关关闭时输入框禁用（置灰）
   - 标签说明："角色在此会话中最多发送 N 条消息后自动停止"

3. **禁言**
   - 大 Toggle 开关
   - 说明文字："开启后角色不会自动回复，但你仍可发送消息"

4. **成员管理（仅群聊）**
   - 当前成员列表（头像 + 名称），每个成员右侧有 `X` 移除按钮
   - 底部"添加成员"按钮，点击弹出 `AddMemberModal`（复用 CreateGroupModal 中的多选逻辑）
   - 至少保留 2 名成员，少于 2 人时禁用"移除"并提示

5. **重置会话**
   - 红色文字按钮"重置当前会话"
   - 点击弹出 `ConfirmDialog`：
     - 标题："重置会话"
     - 内容："重置后当前聊天记录将被归档，相同成员开启新会话。此操作不可撤销。"
     - 按钮："取消" / "确认重置"

6. **解散群聊（仅群聊）**
   - 红色填充按钮"解散群聊"
   - 点击弹出 `ConfirmDialog`：
     - 标题："解散群聊"
     - 内容："解散后群聊将从列表中移除，聊天记录保留在历史记录中。"
     - 按钮："取消" / "确认解散"

### 4.3 私聊差异

- 隐藏第 4、6 项（成员管理、解散群聊）
- 第 5 项文案改为"重置会话"，说明去掉群聊相关描述

### 4.4 交互细节

- **自动保存**：配置修改后 debounce 500ms 自动调用 `update_session_config`，保存成功显示 Toast"已保存"
- **重置/解散后**：自动关闭面板，刷新 `SessionList`，如当前会话被解散则清空 `selectedSessionId`
- **禁言实时生效**：调度器在触发前读取 `mute_enabled`，无需重启
- **历史提示条数实时生效**：下一次 Prompt 组装即生效

---

## 五、PromptAssembler 适配

`PromptAssembler` 在组装历史消息时，需要：

1. 从 `session_settings` 读取 `history_limit`（而非固定值）
2. 同时过滤 `page_index = current_chat_page` 的消息
3. 按时间倒序取最近 N 条，再正序排列注入 Prompt

```rust
// 伪代码
let config = session_repo.get_config(session_id)?;
let limit = config.history_limit;
let page = private_sessions.current_chat_page; // or group_sessions
let messages = message_repo.get_messages_by_session_and_page(session_id, page, limit)?;
```

---

## 六、边界与异常处理

| 场景 | 处理 |
|------|------|
| `history_limit` 输入非法（<1 或 >200） | 前端校验，后端兜底 clamp 到 1-200 |
| 重置会话时创建 page 失败 | 事务回滚，返回错误 Toast |
| 解散群聊时 session 不存在 | 返回 404 错误，前端提示"群聊已不存在" |
| 移除成员后群聊只剩 1 人 | 前端禁用移除按钮并提示"群聊至少需要 2 名成员" |
| 添加已在群聊中的成员 | 后端忽略并返回成功（幂等） |
| 禁言群聊中用户发送消息 | 允许发送，调度器不触发其他角色 |

---

## 七、文件变更清单

### 后端
- `src-tauri/src/db/schema.rs` — 新增 MIGRATION_V4
- `src-tauri/src/db/migration.rs` — 注册 V4 迁移
- `src-tauri/src/db/session.rs` — 新增 session_settings CRUD、reset_session、disband_group、add/remove member
- `src-tauri/src/models/session.rs` — 新增 SessionConfig、UpdateSessionConfigRequest
- `src-tauri/src/commands/session.rs` — 新增 6 个 Tauri 命令
- `src-tauri/src/llm/prompt.rs` — 适配 history_limit 和 page_index 过滤
- `src-tauri/src/scheduler/mod.rs` — 从 session_settings 读取 mute_enabled 和 message_limit

### 前端
- `src/lib/types.ts` — 新增 SessionConfig、UpdateSessionConfigRequest 类型
- `src/lib/components/ChatView.svelte` — 右上角添加齿轮按钮，集成 SessionSettingsPanel
- `src/lib/components/SessionSettingsPanel.svelte` — **新建**：配置面板抽屉
- `src/lib/components/ConfirmDialog.svelte` — **新建**：二次确认弹窗（可复用）
- `src/lib/components/AddMemberModal.svelte` — **新建**：群聊添加成员弹窗
- `src/lib/stores/sessionStore.svelte.ts` — 添加 resetSession、disbandGroup 方法

---

*设计版本：V1.0*  
*日期：2026-05-12*  
*关联需求：feature_list.md SES-05/07/09, CHAT-04/05, AGT-14*
