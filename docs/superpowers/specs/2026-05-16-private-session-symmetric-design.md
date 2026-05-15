# 私聊参与者对称重构与提示词视角化设计

## 1. 背景与目标

### 当前问题
1. **`private_sessions` 是单边结构**：只有 `agent_id` 字段，默认另一方永远是用户。无法支持 Agent-vs-Agent 私聊。
2. **提示词中私聊名称不自然**：从角色视角看，私聊显示为对方的角色名（如"测试-远坂凛"），而非"和用户的私聊"。
3. **Layer 3 用户条目重复且矛盾**：`get_participants` 硬编码了一行"用户（私聊对象）"，然后 `assemble` 又追加了一行"用户（真实用户）"。
4. **不支持用户人设切换**：用户选择了人设（如"伊莉雅"）后，提示词中仍显示"用户"。

### 目标
- 重构 `private_sessions` 为对称双边结构，支持 User-Agent 和 Agent-Agent 私聊。
- 提示词中的私聊名称从**当前角色视角**显示为"和{对方名称}的私聊"。
- Layer 3 用户条目支持人设替换，且只出现一次，relation 统一为"好友"。
- 用户人设切换时无需修改 `private_sessions` 数据，运行时查询替换。

---

## 2. 数据库变更（Migration V7）

### 2.1 新表结构

```sql
-- V7: 私聊会话对称重构

-- 1. 创建新结构的 private_sessions 表
CREATE TABLE private_sessions_new (
    session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
    
    participant_1_type TEXT NOT NULL CHECK(participant_1_type IN ('user', 'agent')),
    participant_1_id TEXT NOT NULL,
    participant_2_type TEXT NOT NULL CHECK(participant_2_type IN ('user', 'agent')),
    participant_2_id TEXT NOT NULL,
    
    message_limit INTEGER,
    message_limit_enabled INTEGER DEFAULT 1 CHECK(message_limit_enabled IN (0, 1)),
    agent_message_count INTEGER DEFAULT 0,
    last_reset_at INTEGER DEFAULT 0,
    current_chat_page INTEGER DEFAULT 0,
    
    created_at INTEGER NOT NULL,
    
    UNIQUE(participant_1_type, participant_1_id, participant_2_type, participant_2_id)
);

-- 2. 迁移数据：现有数据均为 User-Agent，用户固定为 participant_1
INSERT INTO private_sessions_new (
    session_id, 
    participant_1_type, participant_1_id,
    participant_2_type, participant_2_id,
    message_limit, message_limit_enabled, agent_message_count, last_reset_at, current_chat_page,
    created_at
)
SELECT 
    session_id,
    'user', 'user',
    'agent', agent_id,
    message_limit, message_limit_enabled, agent_message_count, last_reset_at, current_chat_page,
    created_at
FROM private_sessions;

-- 3. 删除旧表，重命名新表
DROP TABLE private_sessions;
ALTER TABLE private_sessions_new RENAME TO private_sessions;

-- 4. 创建新索引（替代旧的 idx_private_sessions_agent）
CREATE INDEX idx_private_sessions_p1 ON private_sessions(participant_1_type, participant_1_id);
CREATE INDEX idx_private_sessions_p2 ON private_sessions(participant_2_type, participant_2_id);
```

### 2.2 排序规则（INSERT 时强制执行）

私聊创建时（`create_private_session` 或系统创建 Agent-Agent 私聊），按以下规则排序后写入：

1. **如果一方 type='user'**：`user` 固定为 `participant_1`，Agent 固定为 `participant_2`。
2. **如果双方都是 agent**：按 `participant_id` 字符串字典序排序，小的为 `participant_1`。

此规则配合 `UNIQUE` 约束，可彻底防止重复创建（A-B 与 B-A 被视为同一个私聊）。

### 2.3 字段语义

- `participant_1/2_type`：`'user'` 或 `'agent'`。
- `participant_1/2_id`：
  - 当 `type='user'` 时，固定为 `'user'`（特殊标记，不指向任何表）。
  - 当 `type='agent'` 时，为 `agents.id`。
- `agent_message_count`：**会话级双方合计**，无论谁发言都累加到此计数器。重置时清零。

---

## 3. 数据模型更新

### 3.1 `PrivateSession` 结构体

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateSession {
    pub session_id: String,
    pub participant_1_type: String,
    pub participant_1_id: String,
    pub participant_2_type: String,
    pub participant_2_id: String,
    pub message_limit: Option<i32>,
    pub message_limit_enabled: bool,
    pub agent_message_count: i32,
    pub last_reset_at: i64,
    pub current_chat_page: i32,
    pub created_at: i64,
}
```

### 3.2 `SessionResponse` 与前端

`SessionResponse` 和前端 UI 保持不变。私聊列表仍显示 `agent_name`（如"测试-远坂凛"），**视角化名称仅在生成提示词时替换**。

---

## 4. PromptAssembler 改造

### 4.1 用户人设运行时查询

新增内部方法：

```rust
fn get_user_persona(conn: &Connection) -> (String, String) {
    // 查询 user_personas 表 is_default=1 的记录
    // 返回: (user_name, user_description)
    // 未选择人设时返回默认值
}
```

默认值常量（`prompt_templates.rs`）：

```rust
pub const USER_NAME_DEFAULT: &str = "用户";
pub const USER_PERSONA_DEFAULT: &str = "正在与你聊天的真实用户";
```

### 4.2 `get_session_name` 视角化改造

**签名变更**：增加 `viewer_agent_id` 参数。

```rust
fn get_session_name(
    conn: &Connection, 
    session_id: &str, 
    viewer_agent_id: &str
) -> Result<String, String> {
    // 1. 查询 private_sessions，获取 participant_1/2
    // 2. 判断 viewer_agent_id 是 p1 还是 p2
    // 3. 确定"对方"：
    //    - 如果对方 type='user': 
    //        调用 get_user_persona()，返回 "和{name}的私聊"
    //    - 如果对方 type='agent':
    //        查询 agents.name，返回 "和{name}的私聊"
    // 4. 群聊保持原逻辑：返回 group_sessions.name
}
```

调用点（`assemble` Layer 4）：

```rust
let session_name = Self::get_session_name(conn, &current_session, agent_id)?;
```

### 4.3 `get_agent_sessions` 视角化改造

同样增加 `viewer_agent_id` 参数。对每个私聊会话，按上述逻辑计算对方名称，生成 context list：

```
- session_id: xxx, 名称: 和用户的私聊, 类型: private
```

### 4.4 `get_participants` 改造

**核心变更**：

1. **删除硬编码的用户 push**：不再在开头 `participants.push(("用户", "私聊对象", ...))`。
2. **删除循环后的追加**：不再追加 `LAYER_PARTICIPANTS_USER_LINE`。
3. **查询当前 agent 的所有私聊对象**：通过对称结构查询 `WHERE (p1_type='agent' AND p1_id=?) OR (p2_type='agent' AND p2_id=?)`，然后取对方。
4. **用户条目走人设查询**：
   - 如果对方是 user：`name = get_user_persona().0`，`persona = get_user_persona().1`，`relation = "好友"`。
   - 如果对方是 agent：查询 `agents` 表，`relation = "好友"`（原"私聊对象"/"群友"统一改为"好友"）。
5. **去重**：使用 `HashSet` 按对方 id 去重（一个 agent 可能同时是私聊对象和群友，只出现一次）。

输出示例（用户选择人设"伊莉雅"）：

```
【你认识的参与者】
- 伊莉雅（好友）：魔伊世界观中的小学生魔术师
- 测试-远坂凛（好友）：远坂家的继承人
```

未选择人设时：

```
【你认识的参与者】
- 用户（好友）：正在与你聊天的真实用户
- 测试-远坂凛（好友）：远坂家的继承人
```

### 4.5 `HistoryPromptAssembler` 同步修改

`HistoryPromptAssembler` 中涉及私聊名称展示的逻辑（`get_session_name`、`get_agent_sessions` 等）与 `PromptAssembler` 保持一致，同步改造。

---

## 5. 创建私聊流程更新

### 5.1 User-Agent 私聊（前端发起）

`CreatePrivateSessionRequest` 仍只传 `agent_id`（用户选择要聊天的 Agent）。后端创建时：

```rust
// 排序规则：user 永远在 participant_1
participant_1_type = "user";
participant_1_id = "user";
participant_2_type = "agent";
participant_2_id = agent_id;
```

### 5.2 Agent-Agent 私聊（系统未来发起）

系统创建时按字典序排序：

```rust
if agent_a_id < agent_b_id {
    p1 = ("agent", agent_a_id);
    p2 = ("agent", agent_b_id);
} else {
    p1 = ("agent", agent_b_id);
    p2 = ("agent", agent_a_id);
}
```

---

## 6. 迁移策略

### 6.1 现有数据迁移

Migration V7 在 `MIGRATION_V6` 之后执行：

1. 创建 `private_sessions_new` 表。
2. 将现有 `private_sessions` 数据迁移：
   - `agent_id` → `participant_2_id`
   - `participant_1_type='user'`, `participant_1_id='user'`
3. 删除旧表，重命名新表。
4. 创建新索引。

### 6.2 代码迁移

按依赖顺序修改：

1. `db/schema.rs` — Migration V7
2. `models/session.rs` — `PrivateSession` struct
3. `db/session.rs` — 所有 private_session 查询重构（CREATE、READ、UPDATE、LIST）
4. `commands/session.rs` — `create_private_session` 新排序逻辑
5. `llm/prompt_templates.rs` — 删除 `LAYER_PARTICIPANTS_USER_LINE`，更新默认值常量
6. `llm/prompt.rs` — `get_session_name`、`get_agent_sessions`、`get_participants`、`assemble`
7. `llm/history_prompt.rs` — 同步修改
8. 测试文件 — 更新测试数据和断言

---

## 7. 受影响的文件清单

| 文件 | 变更内容 |
|------|---------|
| `src-tauri/src/db/schema.rs` | 新增 `MIGRATION_V7` |
| `src-tauri/src/db/session.rs` | 重构所有 `private_sessions` 查询逻辑 |
| `src-tauri/src/models/session.rs` | 更新 `PrivateSession` struct |
| `src-tauri/src/commands/session.rs` | `create_private_session` 排序逻辑 |
| `src-tauri/src/llm/prompt.rs` | `get_session_name`、`get_agent_sessions`、`get_participants` 视角化改造；修复重复用户 |
| `src-tauri/src/llm/history_prompt.rs` | 同步修改私聊名称逻辑 |
| `src-tauri/src/llm/prompt_templates.rs` | 删除 `LAYER_PARTICIPANTS_USER_LINE`；新增 `USER_NAME_DEFAULT`、`USER_PERSONA_DEFAULT` |
| `src-tauri/src/llm/prompt.rs`（tests） | 更新测试数据构造和断言 |
| `src-tauri/src/llm/history_prompt.rs`（tests） | 同步更新测试 |

---

## 8. 测试计划

### 8.1 数据库迁移测试
- 验证现有 User-Agent 数据正确迁移到对称结构（user 在 p1，agent 在 p2）。

### 8.2 排序与防重测试
- 测试 User-Agent 创建：user 始终在 p1。
- 测试 Agent-Agent 创建：id 字典序小的在 p1。
- 测试重复创建：A-B 与 B-A 触发 `UNIQUE` 约束失败。

### 8.3 提示词视角化测试
- 测试 `get_session_name`：
  - User-Agent 私聊，未选择人设 → "和用户的私聊"。
  - User-Agent 私聊，选择人设"伊莉雅" → "和伊莉雅的私聊"。
  - Agent-Agent 私聊 → "和对方Agent名的私聊"。
- 测试 `get_agent_sessions`：私聊名称同样视角化。
- 测试 `get_participants`：
  - 用户未选择人设 → "用户（好友）：正在与你聊天的真实用户"。
  - 用户选择人设 → "伊莉雅（好友）：{description}"。
  - 确认无重复用户条目。

### 8.4 回归测试
- 验证群聊名称不受影响。
- 验证 `message_limit` / `agent_message_count` 逻辑不变。
- 验证所有现有 Rust 测试通过。

---

## 9. 边界情况

| 场景 | 处理 |
|------|------|
| 用户无默认人设 | `get_user_persona` 返回默认值 ("用户", "正在与你聊天的真实用户") |
| Agent-Agent 私聊中一方被删除 | 查询时 `JOIN agents` 过滤 `is_deleted=0`，已删除 Agent 不显示 |
| `get_participants` 中同一 Agent 既是私聊对象又是群友 | `HashSet` 去重，只出现一次，relation 显示为"好友" |
| 私聊中只有一方是 Agent（当前唯一场景） | 正常处理，对方为 user |

---

*Design date: 2026-05-16*
