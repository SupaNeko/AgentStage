# 私聊参与者对称重构与提示词视角化 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 `private_sessions` 重构为对称双边结构，支持提示词中私聊名称视角化（"和{对方}的私聊"）和用户人设替换，同时修复 Layer 3 重复用户条目。

**Architecture:** Migration V7 重建 `private_sessions` 表（4 个 participant 字段 + UNIQUE 约束）；DB 层所有查询改为对称匹配；PromptAssembler 增加 `viewer_agent_id` 参数实现视角化名称；运行时查询 `user_personas` 替换人设。

**Tech Stack:** Rust + rusqlite + Tauri v2

---

## 文件结构映射

| 文件 | 职责 |
|------|------|
| `src-tauri/src/db/schema.rs` | Migration V7 SQL |
| `src-tauri/src/models/session.rs` | `PrivateSession` struct 更新 |
| `src-tauri/src/db/session.rs` | 所有 `private_sessions` CRUD 查询重构 |
| `src-tauri/src/db/message.rs` | `get_visible_messages_for_agent` 中 `private_sessions` 查询适配 |
| `src-tauri/src/commands/session.rs` | `create_private_session` 排序逻辑 |
| `src-tauri/src/commands/message.rs` | `send_user_message` 中 `private_sessions` agent 查询适配 |
| `src-tauri/src/llm/prompt_templates.rs` | 删除 `LAYER_PARTICIPANTS_USER_LINE`，新增默认值常量 |
| `src-tauri/src/llm/prompt.rs` | `get_user_persona`、`get_session_name`、`get_agent_sessions`、`get_participants` 改造 |
| `src-tauri/src/llm/history_prompt.rs` | 同步修改 `get_session_name` 等 |

---

### Task 1: Migration V7 — 数据库 Schema 变更

**Files:**
- Modify: `src-tauri/src/db/schema.rs`

- [ ] **Step 1: 在 MIGRATION_V6 之后添加 MIGRATION_V7**

在 `schema.rs` 文件末尾（`MIGRATION_V6` 常量之后）添加：

```rust
pub const MIGRATION_V7: &str = r#"
-- V7: 私聊会话对称重构
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

DROP TABLE private_sessions;
ALTER TABLE private_sessions_new RENAME TO private_sessions;

CREATE INDEX idx_private_sessions_p1 ON private_sessions(participant_1_type, participant_1_id);
CREATE INDEX idx_private_sessions_p2 ON private_sessions(participant_2_type, participant_2_id);
"#;
```

- [ ] **Step 2: 确认 Migration V7 位于文件末尾且常量名正确**

运行 `cargo check` 确认无编译错误（此时常量未被引用，不影响编译）。

---

### Task 2: PrivateSession 结构体更新

**Files:**
- Modify: `src-tauri/src/models/session.rs`

- [ ] **Step 1: 替换 PrivateSession struct**

将现有的：
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivateSession {
    pub session_id: String,
    pub agent_id: String,
    pub message_limit: Option<i32>,
    pub message_limit_enabled: bool,
    pub agent_message_count: i32,
    pub last_reset_at: i64,
    pub created_at: i64,
}
```

替换为：
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

- [ ] **Step 2: 运行 `cargo check` 查看所有因字段缺失导致的编译错误**

记录所有报错位置，供后续任务修复。

---

### Task 3: db/session.rs 查询重构

**Files:**
- Modify: `src-tauri/src/db/session.rs`

- [ ] **Step 1: 更新 SELECT_COLUMNS 和 row_to_session_response**

将第 5 行的 `SELECT_COLUMNS`：
```rust
const SELECT_COLUMNS: &str = "s.id, s.session_type, s.last_message_at, s.last_message_preview, s.unread_count, ps.agent_id, a.name, a.avatar_path, gs.name, gs.avatar_path, gs.mute_enabled";
```

改为：
```rust
const SELECT_COLUMNS: &str = "s.id, s.session_type, s.last_message_at, s.last_message_preview, s.unread_count, ps.participant_2_id, a.name, a.avatar_path, gs.name, gs.avatar_path, gs.mute_enabled";
```

**说明**：当前所有私聊都是 User-Agent 且用户固定在 p1，因此 agent 固定在 p2。`participant_2_id` 即为 `agent_id`，保持前端兼容。

- [ ] **Step 2: 更新 JOIN 条件**

将所有 SQL 中：
```sql
LEFT JOIN agents a ON ps.agent_id = a.id
```

替换为：
```sql
LEFT JOIN agents a ON ps.participant_2_type = 'agent' AND ps.participant_2_id = a.id
```

需要修改的位置（通过搜索 `ps.agent_id = a.id` 定位）：
- `get_private_session_by_agent_id`
- `get_session_by_id`
- `list_sessions`

- [ ] **Step 3: 更新 get_private_session_by_agent_id 的 WHERE 条件**

将第 30 行：
```sql
WHERE s.is_deleted = 0 AND ps.agent_id = ?1 AND s.session_type = 'private'
```

改为：
```sql
WHERE s.is_deleted = 0 AND ps.participant_2_id = ?1 AND ps.participant_2_type = 'agent' AND s.session_type = 'private'
```

- [ ] **Step 4: 更新 create_private_session 的 INSERT 语句**

将第 54-57 行：
```rust
conn.execute(
    "INSERT INTO private_sessions (session_id, agent_id, message_limit_enabled, created_at) VALUES (?1, ?2, 1, ?3)",
    (&session_id, agent_id, now),
)?;
```

改为：
```rust
conn.execute(
    "INSERT INTO private_sessions (session_id, participant_1_type, participant_1_id, participant_2_type, participant_2_id, message_limit_enabled, created_at) VALUES (?1, 'user', 'user', 'agent', ?2, 1, ?3)",
    (&session_id, agent_id, now),
)?;
```

- [ ] **Step 5: 搜索并修复文件中所有剩余引用 `ps.agent_id` 的 SQL**

使用搜索确认文件内无剩余 `ps.agent_id` 引用。常见位置：
- `reset_session` 中的 UPDATE
- `get_session_config` 中的 JOIN
- 测试辅助函数中的 INSERT

- [ ] **Step 6: 运行 `cargo check` 确认 db/session.rs 编译通过**

---

### Task 4: db/message.rs 和 commands/message.rs 适配

**Files:**
- Modify: `src-tauri/src/db/message.rs`
- Modify: `src-tauri/src/commands/message.rs`

- [ ] **Step 1: 更新 db/message.rs 中的 private_sessions 查询**

搜索 `private_sessions WHERE agent_id` 和 `private_sessions WHERE session_id`。

对于 `get_visible_messages_for_agent`（约第 109 行）：
```sql
SELECT session_id FROM private_sessions WHERE agent_id = ?1
```
改为：
```sql
SELECT session_id FROM private_sessions WHERE (participant_1_type = 'agent' AND participant_1_id = ?1) OR (participant_2_type = 'agent' AND participant_2_id = ?1)
```

对于 `get_messages_after_time`（约第 130 行），同样修改。

- [ ] **Step 2: 更新 commands/message.rs 中的 agent_id 查询**

搜索 `"SELECT agent_id FROM private_sessions WHERE session_id = ?1"`。

大约在第 109 行，将：
```rust
let agent_id: String = conn.query_row(
    "SELECT agent_id FROM private_sessions WHERE session_id = ?1",
    [&session_id],
    |row| row.get(0),
)?;
```

改为查询 participant_2_id（当前兼容方案）：
```rust
let agent_id: String = conn.query_row(
    "SELECT participant_2_id FROM private_sessions WHERE session_id = ?1 AND participant_2_type = 'agent'",
    [&session_id],
    |row| row.get(0),
)?;
```

- [ ] **Step 3: 运行 `cargo check` 确认编译通过**

---

### Task 5: commands/session.rs create_private_session 排序逻辑

**Files:**
- Modify: `src-tauri/src/commands/session.rs`

- [ ] **Step 1: 确认 create_private_session 命令无需修改**

`create_private_session` 调用 `session_repo::create_private_session`，排序逻辑已在 Task 3 Step 4 的 repository 层实现。本 Task 只需确认 commands 层无需额外变更。

- [ ] **Step 2: 运行 `cargo check` 确认**

---

### Task 6: prompt_templates.rs 更新

**Files:**
- Modify: `src-tauri/src/llm/prompt_templates.rs`

- [ ] **Step 1: 删除 LAYER_PARTICIPANTS_USER_LINE**

删除第 7 行：
```rust
pub const LAYER_PARTICIPANTS_USER_LINE: &str = "- 用户（真实用户）：正在与你聊天的真实用户。";
```

- [ ] **Step 2: 新增用户默认值常量**

在第 28 行之后（`UNKNOWN_SESSION` 之前或文件末尾常量区）添加：

```rust
pub const USER_NAME_DEFAULT: &str = "用户";
pub const USER_PERSONA_DEFAULT: &str = "正在与你聊天的真实用户";
```

- [ ] **Step 3: 运行 `cargo check` 查找所有引用 LAYER_PARTICIPANTS_USER_LINE 的位置**

---

### Task 7: prompt.rs 核心改造

**Files:**
- Modify: `src-tauri/src/llm/prompt.rs`

- [ ] **Step 1: 新增 get_user_persona 方法**

在 `PromptAssembler` impl 中新增（放在 `apply_variables` 附近）：

```rust
fn get_user_persona(conn: &Connection) -> (String, String) {
    let result: Result<(String, Option<String>), rusqlite::Error> = conn.query_row(
        "SELECT name, description FROM user_personas WHERE is_default = 1 LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    );
    match result {
        Ok((name, desc)) => (name, desc.unwrap_or_else(|| prompt_templates::USER_PERSONA_DEFAULT.to_string())),
        Err(_) => (
            prompt_templates::USER_NAME_DEFAULT.to_string(),
            prompt_templates::USER_PERSONA_DEFAULT.to_string(),
        ),
    }
}
```

- [ ] **Step 2: 改造 get_session_name（增加 viewer_agent_id 参数）**

将现有 `get_session_name` 替换为：

```rust
fn get_session_name(
    conn: &Connection,
    session_id: &str,
    viewer_agent_id: &str,
) -> Result<String, String> {
    // 先尝试群聊
    let result: Result<String, rusqlite::Error> = conn.query_row(
        "SELECT name FROM group_sessions WHERE session_id = ?1",
        [session_id],
        |row| row.get(0),
    );
    if let Ok(name) = result {
        return Ok(name);
    }

    // 私聊：查询参与者，从 viewer 视角显示对方名称
    let result: Result<(String, String, String, String), rusqlite::Error> = conn.query_row(
        "SELECT participant_1_type, participant_1_id, participant_2_type, participant_2_id \
         FROM private_sessions WHERE session_id = ?1",
        [session_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    );

    if let Ok((p1_type, p1_id, p2_type, p2_id)) = result {
        // 确定对方
        let other = if p1_type == "agent" && p1_id == viewer_agent_id {
            (p2_type, p2_id)
        } else if p2_type == "agent" && p2_id == viewer_agent_id {
            (p1_type, p1_id)
        } else {
            // viewer 不是该私聊的 agent（异常情况），默认显示 p2
            (p2_type, p2_id)
        };

        let other_name = if other.0 == "user" {
            Self::get_user_persona(conn).0
        } else {
            conn.query_row(
                "SELECT name FROM agents WHERE id = ?1 AND is_deleted = 0",
                [other.1],
                |row| row.get(0),
            ).unwrap_or_else(|_| prompt_templates::UNKNOWN_SESSION.to_string())
        };

        return Ok(format!("和{}的私聊", other_name));
    }

    Ok(prompt_templates::UNKNOWN_SESSION.to_string())
}
```

- [ ] **Step 3: 更新 assemble 中 get_session_name 的调用点**

在 `assemble` 方法中（约第 126 行）：
```rust
let session_name = Self::get_session_name(conn, &current_session)?;
```
改为：
```rust
let session_name = Self::get_session_name(conn, &current_session, agent_id)?;
```

- [ ] **Step 4: 改造 get_agent_sessions（增加 viewer_agent_id 参数）**

将 `get_agent_sessions` 签名改为：
```rust
fn get_agent_sessions(
    conn: &Connection,
    agent_id: &str,
) -> Result<Vec<(String, String, String)>, String> {
```

方法体中，私聊查询部分改为：

```rust
// 私聊会话
let mut stmt = conn
    .prepare(
        "SELECT s.id, ps.participant_1_type, ps.participant_1_id, ps.participant_2_type, ps.participant_2_id \
         FROM sessions s \
         JOIN private_sessions ps ON s.id = ps.session_id \
         WHERE s.is_deleted = 0 \
         AND ((ps.participant_1_type = 'agent' AND ps.participant_1_id = ?1) \
           OR (ps.participant_2_type = 'agent' AND ps.participant_2_id = ?1))"
    )
    .map_err(|e| e.to_string())?;
let rows = stmt
    .query_map([agent_id], |row| {
        let sid: String = row.get(0)?;
        let p1_type: String = row.get(1)?;
        let p1_id: String = row.get(2)?;
        let p2_type: String = row.get(3)?;
        let p2_id: String = row.get(4)?;
        
        let other = if p1_type == "agent" && p1_id == agent_id {
            (p2_type, p2_id)
        } else {
            (p1_type, p1_id)
        };
        
        let other_name = if other.0 == "user" {
            Self::get_user_persona(conn).0
        } else {
            conn.query_row(
                "SELECT name FROM agents WHERE id = ?1 AND is_deleted = 0",
                [other.1],
                |row| row.get(0),
            ).unwrap_or_else(|_| prompt_templates::UNKNOWN_SESSION.to_string())
        };
        
        Ok((sid, other_name, "private".to_string()))
    })
    .map_err(|e| e.to_string())?;
for row in rows {
    sessions.push(row.map_err(|e| e.to_string())?);
}
drop(stmt);
```

- [ ] **Step 5: 更新 build_instruction 中 get_agent_sessions 的调用**

在 `build_instruction` 中（约第 162 行）：
```rust
let sessions = Self::get_agent_sessions(conn, agent_id)?;
```
此调用无需修改，因为签名中 agent_id 已在第一位。

- [ ] **Step 6: 改造 get_participants**

将 `get_participants` 方法体完全替换为：

```rust
fn get_participants(
    conn: &Connection,
    agent_id: &str,
) -> Result<Vec<(String, String, String)>, String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut participants: Vec<(String, String, String)> = Vec::new();

    // 1. 收集私聊对象（通过对称结构查询）
    let mut stmt = conn
        .prepare(
            "SELECT ps.participant_1_type, ps.participant_1_id, ps.participant_2_type, ps.participant_2_id \
             FROM private_sessions ps \
             JOIN sessions s ON ps.session_id = s.id \
             WHERE s.is_deleted = 0 \
             AND ((ps.participant_1_type = 'agent' AND ps.participant_1_id = ?1) \
               OR (ps.participant_2_type = 'agent' AND ps.participant_2_id = ?1))"
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([agent_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let (user_name, user_persona) = Self::get_user_persona(conn);

    for row in rows {
        let (p1_type, p1_id, p2_type, p2_id) = row.map_err(|e| e.to_string())?;
        let other = if p1_type == "agent" && p1_id == agent_id {
            (p2_type, p2_id)
        } else {
            (p1_type, p1_id)
        };

        if other.0 == "user" {
            if seen.insert("__user__".to_string()) {
                participants.push((user_name.clone(), "好友".to_string(), user_persona.clone()));
            }
        } else if seen.insert(other.1.clone()) {
            let name: Result<String, rusqlite::Error> = conn.query_row(
                "SELECT name FROM agents WHERE id = ?1 AND is_deleted = 0",
                [other.1],
                |row| row.get(0),
            );
            if let Ok(name) = name {
                let persona: Result<String, rusqlite::Error> = conn.query_row(
                    "SELECT simplified_persona FROM agents WHERE id = ?1 AND is_deleted = 0",
                    [other.1],
                    |row| row.get(0),
                );
                participants.push((name, "好友".to_string(), persona.unwrap_or_default()));
            }
        }
    }
    drop(stmt);

    // 2. 收集群聊成员
    let mut stmt = conn
        .prepare(
            "SELECT DISTINCT gm.participant_id \
             FROM group_members gm \
             JOIN sessions s ON gm.session_id = s.id \
             WHERE gm.session_id IN ( \
                 SELECT session_id FROM group_members WHERE participant_id = ?1 AND participant_type = 'agent' \
             ) AND gm.participant_type = 'agent' AND gm.participant_id != ?1 AND s.is_deleted = 0"
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([agent_id], |row| {
            Ok(row.get::<_, String>(0)?)
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        let id = row.map_err(|e| e.to_string())?;
        if seen.insert(id.clone()) {
            let name: Result<String, rusqlite::Error> = conn.query_row(
                "SELECT name FROM agents WHERE id = ?1 AND is_deleted = 0",
                [&id],
                |row| row.get(0),
            );
            if let Ok(name) = name {
                let persona: Result<String, rusqlite::Error> = conn.query_row(
                    "SELECT simplified_persona FROM agents WHERE id = ?1 AND is_deleted = 0",
                    [&id],
                    |row| row.get(0),
                );
                participants.push((name, "好友".to_string(), persona.unwrap_or_default()));
            }
        }
    }

    Ok(participants)
}
```

- [ ] **Step 7: 删除 assemble 中 LAYER_PARTICIPANTS_USER_LINE 的追加**

在 `assemble` 方法中（约第 33-38 行），将：
```rust
for (name, relation, persona) in participants {
    layer.push_str(&format!("- {}（{}）：{}\n", name, relation, persona));
}
layer.push_str(prompt_templates::LAYER_PARTICIPANTS_USER_LINE);
```

改为：
```rust
for (name, relation, persona) in participants {
    layer.push_str(&format!("- {}（{}）：{}\n", name, relation, persona));
}
```

（删除 `layer.push_str(prompt_templates::LAYER_PARTICIPANTS_USER_LINE);`）

- [ ] **Step 8: 运行 `cargo check` 确认 prompt.rs 编译通过**

---

### Task 8: history_prompt.rs 同步修改

**Files:**
- Modify: `src-tauri/src/llm/history_prompt.rs`

- [ ] **Step 1: 更新 get_session_name 调用**

搜索 `get_session_name` 的调用点，将其改为传入 `agent_id`：
```rust
let session_name = PromptAssembler::get_session_name(conn, &current_session, agent_id)?;
```

- [ ] **Step 2: 如有其他私聊名称相关逻辑，同步修改**

搜索文件中的 `"private"` 或 `agents.name` 相关查询，确保与 `prompt.rs` 的视角化逻辑一致。

- [ ] **Step 3: 运行 `cargo check` 确认**

---

### Task 9: 测试修复与新增

**Files:**
- Modify: `src-tauri/src/llm/prompt.rs` (tests 模块)
- Modify: `src-tauri/src/llm/history_prompt.rs` (tests 模块)
- Modify: `src-tauri/src/db/session.rs` (tests 模块)

- [ ] **Step 1: 更新 prompt.rs 测试中的 insert_private_session 辅助函数**

将测试辅助函数：
```rust
fn insert_private_session(conn: &Connection, session_id: &str, agent_id: &str, page: i32) {
    conn.execute(
        "INSERT INTO private_sessions (session_id, agent_id, created_at, current_chat_page) VALUES (?1, ?2, ?3, ?4)",
        (session_id, agent_id, 0i64, page),
    ).unwrap();
}
```

改为：
```rust
fn insert_private_session(conn: &Connection, session_id: &str, agent_id: &str, page: i32) {
    conn.execute(
        "INSERT INTO private_sessions (session_id, participant_1_type, participant_1_id, participant_2_type, participant_2_id, created_at, current_chat_page) VALUES (?1, 'user', 'user', 'agent', ?2, ?3, ?4)",
        (session_id, agent_id, 0i64, page),
    ).unwrap();
}
```

- [ ] **Step 2: 更新 db/session.rs 测试中的 INSERT 语句**

搜索 `INSERT INTO private_sessions` 并更新为对称结构格式。

- [ ] **Step 3: 运行 `cargo check --tests` 查找所有编译错误并修复**

- [ ] **Step 4: 新增视角化名称测试**

在 `prompt.rs` 的 tests 模块末尾新增：

```rust
#[test]
fn test_private_session_name_from_agent_perspective() {
    let conn = init_test_db();
    insert_agent(&conn, "agent1", "远坂凛", "远坂家的继承人");
    insert_session(&conn, "sess1", "private");
    insert_private_session(&conn, "sess1", "agent1", 0);
    insert_session_settings(&conn, "sess1", 50);

    let msg = Message {
        id: "msg1".to_string(), session_id: "sess1".to_string(),
        sender_type: "user".to_string(), sender_id: "user".to_string(),
        content: "Hello".to_string(), created_at: 1000,
        message_type: "text".to_string(), tool_call_data: None,
        generation_info: None, is_deleted: false,
        sender_name: "用户".to_string(), sender_avatar: None, page_index: 0,
    };
    insert_message(&conn, &msg);

    let prompt = PromptAssembler::assemble(&conn, "agent1", None, None, &[]).unwrap();
    assert!(prompt.contains("和用户的私聊"), "Private session name should be '和用户的私聊' from agent perspective, got:\n{}", prompt);
    assert!(!prompt.contains("远坂凛"), "Prompt should NOT contain agent name as session name");
}

#[test]
fn test_user_persona_replacement() {
    let conn = init_test_db();
    insert_agent(&conn, "agent1", "远坂凛", "远坂家的继承人");
    insert_session(&conn, "sess1", "private");
    insert_private_session(&conn, "sess1", "agent1", 0);
    insert_session_settings(&conn, "sess1", 50);

    // 插入用户人设
    conn.execute(
        "INSERT INTO user_personas (id, name, description, is_default, created_at, updated_at) VALUES (?1, ?2, ?3, 1, ?4, ?4)",
        ("persona1", "伊莉雅", "魔伊世界观中的小学生魔术师", 0i64),
    ).unwrap();

    let msg = Message {
        id: "msg1".to_string(), session_id: "sess1".to_string(),
        sender_type: "user".to_string(), sender_id: "user".to_string(),
        content: "Hello".to_string(), created_at: 1000,
        message_type: "text".to_string(), tool_call_data: None,
        generation_info: None, is_deleted: false,
        sender_name: "用户".to_string(), sender_avatar: None, page_index: 0,
    };
    insert_message(&conn, &msg);

    let prompt = PromptAssembler::assemble(&conn, "agent1", None, None, &[]).unwrap();
    assert!(prompt.contains("和伊莉雅的私聊"), "Private session name should use persona name");
    assert!(prompt.contains("伊莉雅（好友）：魔伊世界观中的小学生魔术师"), "Participant list should use persona");
}

#[test]
fn test_no_duplicate_user_entry() {
    let conn = init_test_db();
    insert_agent(&conn, "agent1", "远坂凛", "远坂家的继承人");
    insert_session(&conn, "sess1", "private");
    insert_private_session(&conn, "sess1", "agent1", 0);
    insert_session_settings(&conn, "sess1", 50);

    let msg = Message {
        id: "msg1".to_string(), session_id: "sess1".to_string(),
        sender_type: "user".to_string(), sender_id: "user".to_string(),
        content: "Hello".to_string(), created_at: 1000,
        message_type: "text".to_string(), tool_call_data: None,
        generation_info: None, is_deleted: false,
        sender_name: "用户".to_string(), sender_avatar: None, page_index: 0,
    };
    insert_message(&conn, &msg);

    let prompt = PromptAssembler::assemble(&conn, "agent1", None, None, &[]).unwrap();
    let user_count = prompt.matches("用户（好友）").count();
    assert_eq!(user_count, 1, "User entry should appear exactly once in participants, got {} occurrences", user_count);
}
```

- [ ] **Step 5: 运行 `cargo check --tests` 确认所有测试编译通过**

---

### Task 10: 最终验证与提交

**Files:**
- 所有已修改文件

- [ ] **Step 1: 运行 `cargo check`（不含 tests）确认主代码编译通过**

```bash
cd src-tauri && cargo check
```

- [ ] **Step 2: 运行 `cargo check --tests` 确认测试代码编译通过**

```bash
cd src-tauri && cargo check --tests
```

- [ ] **Step 3: 检查未暂存变更**

```bash
git status
```

- [ ] **Step 4: 提交所有变更**

```bash
git add -A
git commit -m "refactor: symmetric private_sessions + prompt perspective names

- Migration V7: rebuild private_sessions with participant_1/2_type/id
- User always in participant_1, agent in participant_2 (for User-Agent)
- UNIQUE constraint prevents duplicate private chats
- Update all SQL queries in db/session.rs, db/message.rs, commands/message.rs
- PromptAssembler: add get_user_persona() for runtime persona replacement
- get_session_name now returns '和{对方}的私聊' from viewer perspective
- get_agent_sessions: private chat names are perspective-aware
- get_participants: user entry uses persona name/description, relation='好友'
- Fix duplicate user entry in Layer 3 (remove hardcoded push + LAYER_PARTICIPANTS_USER_LINE)
- HistoryPromptAssembler synced with same changes
- Add tests for perspective names, persona replacement, no duplicate user"
```

---

## Self-Review Checklist

- [x] **Spec coverage**: Migration V7 (Task 1) ✓, PrivateSession struct (Task 2) ✓, DB queries (Tasks 3-5) ✓, prompt_templates (Task 6) ✓, PromptAssembler perspective (Task 7) ✓, History sync (Task 8) ✓, tests (Task 9) ✓
- [x] **Placeholder scan**: 无 TBD/TODO/"implement later"
- [x] **Type consistency**: `get_session_name` 签名 `(conn, session_id, viewer_agent_id)` 在所有调用点一致；`get_user_persona` 返回 `(String, String)` 一致
- [x] **DRY**: `get_user_persona` 只定义一次，被 `get_session_name`、`get_agent_sessions`、`get_participants` 共享
