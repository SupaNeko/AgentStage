# AGT-18 记忆系统数据模型与 UI 配置 — 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为每个角色添加长期记忆（自身长文本）和对他人的记忆（按目标分组短文本），在角色配置页新增【记忆】标签页，包含启用开关、重置记忆、记忆编辑功能。

**Architecture:** 采用表合并方案——在 `agent_relationships` 表增加 `memory_text` 字段，与 `relationship_text` 共存但独立更新。`agents` 表增加 `long_term_memory` 和 `memory_enabled`。前端复用 `list_agent_relationships` 的对象列表在记忆标签页渲染记忆输入框。

**Tech Stack:** Rust (Tauri v2) + rusqlite + Svelte 5 + TypeScript + TailwindCSS v4

---

## 文件结构映射

| 文件 | 变更 | 职责 |
|------|------|------|
| `src-tauri/src/db/schema.rs` | 新增 MIGRATION_V13 | 数据库 Schema 变更 |
| `src-tauri/src/db/migration.rs` | 注册 V13 | 迁移注册 |
| `src-tauri/src/models/agent.rs` | 修改 | Agent/Create/Update/Response 增加 `long_term_memory` + `memory_enabled` |
| `src-tauri/src/models/agent_relationship.rs` | 修改 | RelationshipItem 增加 `memory_text` |
| `src-tauri/src/db/agent.rs` | 修改 | `get_by_id`/`update` 支持新字段；新增 `clear_long_term_memory` |
| `src-tauri/src/db/agent_relationship.rs` | 修改 | `list_relationships_by_observer` 返回 `memory_text`；新增 `upsert_memory` + `clear_memories_by_observer` |
| `src-tauri/src/commands/agent_relationship.rs` | 新增 | `update_agent_memory` 命令 |
| `src-tauri/src/commands/agent.rs` | 修改 | `update_agent` 支持新字段；新增 `reset_agent_memory` 命令 |
| `src-tauri/src/lib.rs` | 修改 | 注册新命令到 `generate_handler!` |
| `src/lib/types.ts` | 修改 | TypeScript 类型增加新字段 |
| `src/lib/components/AgentMemoryPanel.svelte` | **新建** | 记忆标签页核心组件 |
| `src/lib/components/ConfirmResetMemoryModal.svelte` | **新建** | 重置记忆二次确认弹窗 |
| `src/lib/components/AgentDetail.svelte` | 修改 | 增加 `memory` 标签页和 `long_term_memory` 表单绑定 |
| `docs/feature_list.md` | 修改 | AGT-18 状态更新 |

---

## Task 1: Migration V13 — Schema 变更

**Files:**
- Modify: `src-tauri/src/db/schema.rs`
- Modify: `src-tauri/src/db/migration.rs`

- [ ] **Step 1: 在 schema.rs 末尾添加 MIGRATION_V13**

在 `MIGRATION_V12` 之后、`CREATE_MIGRATIONS_TABLE` 之前（实际是在所有 MIGRATION 常量之后），添加：

```rust
pub const MIGRATION_V13: &str = r#"
-- V13: 记忆系统基础数据层
-- 1. agents 表增加长期记忆和记忆开关
ALTER TABLE agents ADD COLUMN long_term_memory TEXT DEFAULT '';
ALTER TABLE agents ADD COLUMN memory_enabled INTEGER DEFAULT 1 CHECK(memory_enabled IN (0, 1));

-- 2. agent_relationships 表增加对他人的记忆
ALTER TABLE agent_relationships ADD COLUMN memory_text TEXT NOT NULL DEFAULT '';
"#;
```

- [ ] **Step 2: 在 migration.rs 的 MIGRATIONS 数组中注册 V13**

在 V12 之后插入：

```rust
    Migration {
        version: 13,
        name: "memory_system_base",
        sql: super::schema::MIGRATION_V13,
    },
```

- [ ] **Step 3: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: 通过（无编译错误）

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db/schema.rs src-tauri/src/db/migration.rs
git commit -m "feat(db): add migration V13 for memory system (long_term_memory, memory_enabled, memory_text)"
```

---

## Task 2: 模型层变更

**Files:**
- Modify: `src-tauri/src/models/agent.rs`
- Modify: `src-tauri/src/models/agent_relationship.rs`
- Modify: `src/lib/types.ts`

- [ ] **Step 1: 修改 `Agent` 结构体**

在 `src-tauri/src/models/agent.rs` 的 `Agent` 结构体中，`frequency_penalty` 之后、`api_key_encrypted` 之前，添加：

```rust
    pub long_term_memory: Option<String>,
    pub memory_enabled: bool,
```

- [ ] **Step 2: 修改 `AgentResponse` 结构体**

在同一文件的 `AgentResponse` 中，`frequency_penalty` 之后、`api_key` 之前，添加：

```rust
    pub long_term_memory: Option<String>,
    pub memory_enabled: bool,
```

- [ ] **Step 3: 修改 `AgentResponse::from(Agent)`**

在 `impl From<Agent> for AgentResponse` 中，`frequency_penalty` 映射之后、`api_key` 之前，添加：

```rust
            long_term_memory: agent.long_term_memory,
            memory_enabled: agent.memory_enabled,
```

- [ ] **Step 4: 修改 `CreateAgentRequest`**

在 `CreateAgentRequest` 结构体末尾（`thinking_mode` 之后），添加：

```rust
    pub long_term_memory: Option<String>,
    pub memory_enabled: Option<bool>,
```

- [ ] **Step 5: 修改 `UpdateAgentRequest`**

在 `UpdateAgentRequest` 结构体末尾（`thinking_mode` 之后），添加：

```rust
    pub long_term_memory: Option<String>,
    pub memory_enabled: Option<bool>,
```

- [ ] **Step 6: 修改 `RelationshipItem`**

在 `src-tauri/src/models/agent_relationship.rs` 的 `RelationshipItem` 中，`relationship_text` 之后、`updated_at` 之前，添加：

```rust
    pub memory_text: String,
```

- [ ] **Step 7: 修改 TypeScript 类型**

在 `src/lib/types.ts` 的 `Agent` 接口中，`api_key` 之后、`is_deleted` 之前，添加：

```typescript
    long_term_memory?: string;
    memory_enabled?: boolean;
```

在同一文件的 `RelationshipItem` 接口中，`relationship_text` 之后、`updated_at` 之前，添加：

```typescript
    memory_text: string;
```

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/models/agent.rs src-tauri/src/models/agent_relationship.rs src/lib/types.ts
git commit -m "feat(models): add long_term_memory, memory_enabled, memory_text fields to Agent and RelationshipItem"
```

---

## Task 3: Repository 层 — agent.rs

**Files:**
- Modify: `src-tauri/src/db/agent.rs`

- [ ] **Step 1: 更新 SELECT_COLUMNS**

将常量从：
```rust
const SELECT_COLUMNS: &str = "id, name, avatar_path, detailed_persona, simplified_persona, personality, scenario, example_messages, first_message, creator_notes, tags, model_provider, model_name, base_url, temperature, max_tokens, top_p, presence_penalty, frequency_penalty, api_key_encrypted, thinking_mode, is_deleted, deleted_at, created_at, updated_at";
```
改为：
```rust
const SELECT_COLUMNS: &str = "id, name, avatar_path, detailed_persona, simplified_persona, personality, scenario, example_messages, first_message, creator_notes, tags, model_provider, model_name, base_url, temperature, max_tokens, top_p, presence_penalty, frequency_penalty, long_term_memory, memory_enabled, api_key_encrypted, thinking_mode, is_deleted, deleted_at, created_at, updated_at";
```

注意：`long_term_memory, memory_enabled` 插入在 `frequency_penalty` 之后、`api_key_encrypted` 之前。

- [ ] **Step 2: 更新 `row_to_agent`**

在 `row_to_agent` 函数中，`frequency_penalty` 映射之后、`api_key_encrypted` 之前，添加：

```rust
        long_term_memory: row.get(18)?,
        memory_enabled: row.get::<_, i32>(19)? != 0,
        api_key_encrypted: row.get(20)?,
        thinking_mode: row.get::<_, i32>(21)? != 0,
        is_deleted: row.get::<_, i32>(22)? != 0,
        deleted_at: row.get(23)?,
        created_at: row.get(24)?,
        updated_at: row.get(25)?,
```

注意：后续索引全部后移 2 位。

- [ ] **Step 3: 更新 `create` 函数**

修改 INSERT 语句，增加 `long_term_memory` 和 `memory_enabled` 字段：

将 SQL 从：
```rust
            temperature, max_tokens, api_key_encrypted, thinking_mode, created_at, updated_at
```
改为：
```rust
            temperature, max_tokens, long_term_memory, memory_enabled, api_key_encrypted, thinking_mode, created_at, updated_at
```

将参数从：
```rust
            req.temperature.unwrap_or(0.7), req.max_tokens.unwrap_or(2048),
            &api_key_encrypted, req.thinking_mode.unwrap_or(false) as i32, now, now,
```
改为：
```rust
            req.temperature.unwrap_or(0.7), req.max_tokens.unwrap_or(2048),
            &req.long_term_memory, req.memory_enabled.unwrap_or(true) as i32,
            &api_key_encrypted, req.thinking_mode.unwrap_or(false) as i32, now, now,
```

- [ ] **Step 4: 更新 `update` 函数**

在 UPDATE 语句中，`frequency_penalty = COALESCE(?18, frequency_penalty),` 之后、`api_key_encrypted = COALESCE(?19, api_key_encrypted),` 之前，插入：

```rust
            long_term_memory = COALESCE(?19, long_term_memory),
            memory_enabled = COALESCE(?20, memory_enabled),
```

注意：后续参数编号需要重新对齐。最终 SQL 参数编号调整如下：

```rust
        rusqlite::params![
            &req.id, &req.name, &req.avatar_path, &req.detailed_persona, &req.simplified_persona,
            &req.personality, &req.scenario, &req.example_messages, &req.first_message, &req.creator_notes,
            &req.tags, &req.model_provider, &req.model_name, &req.base_url,
            req.temperature, req.max_tokens,
            req.long_term_memory, req.memory_enabled.map(|v| v as i32),
            api_key_encrypted,
            req.thinking_mode.map(|v| v as i32),
            now,
        ],
```

- [ ] **Step 5: 新增 `clear_long_term_memory`**

在 `soft_delete` 函数之前，添加：

```rust
pub fn clear_long_term_memory(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE agents SET long_term_memory = '' WHERE id = ?1",
        [id],
    )?;
    Ok(())
}
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/db/agent.rs
git commit -m "feat(db): agent repo supports long_term_memory and memory_enabled"
```

---

## Task 4: Repository 层 — agent_relationship.rs

**Files:**
- Modify: `src-tauri/src/db/agent_relationship.rs`

- [ ] **Step 1: 更新 `list_relationships_by_observer` 的 SELECT**

将内部子查询的 SELECT 列表中，每个 `COALESCE(ar.relationship_text, '') as relationship_text,` 之后，添加：

```
                COALESCE(ar.memory_text, '') as memory_text,
```

三个 UNION ALL 分支（用户人设、好友、群友）都需要添加这一行。

- [ ] **Step 2: 更新 `list_relationships_by_observer` 的 query_map**

在 `rows.query_map` 的闭包中，`relationship_text` 映射之后、`updated_at` 之前，添加：

```rust
            memory_text: row.get("memory_text")?,
```

- [ ] **Step 3: 新增 `upsert_memory`**

在 `remove_friendship` 函数之后，添加：

```rust
pub fn upsert_memory(
    conn: &Connection,
    observer_id: &str,
    target_id: &str,
    target_type: &str,
    memory_text: &str,
) -> Result<()> {
    crate::logger::backend("DEBUG", &format!(
        "[DEBUG agent_relationship::upsert_memory] observer_id={}, target_id={}, target_type={}, text_len={}",
        observer_id, target_id, target_type, memory_text.len()
    ));
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO agent_relationships (observer_id, target_id, target_type, memory_text, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(observer_id, target_id, target_type) DO UPDATE SET
             memory_text = excluded.memory_text,
             updated_at = excluded.updated_at",
        (observer_id, target_id, target_type, memory_text, now),
    )?;
    crate::logger::backend("DEBUG", "[DEBUG agent_relationship::upsert_memory] success");
    Ok(())
}
```

- [ ] **Step 4: 新增 `clear_memories_by_observer`**

在 `upsert_memory` 之后，添加：

```rust
pub fn clear_memories_by_observer(conn: &Connection, observer_id: &str) -> Result<()> {
    crate::logger::backend("DEBUG", &format!(
        "[DEBUG agent_relationship::clear_memories_by_observer] observer_id={}", observer_id
    ));
    conn.execute(
        "UPDATE agent_relationships SET memory_text = '' WHERE observer_id = ?1",
        [observer_id],
    )?;
    crate::logger::backend("DEBUG", "[DEBUG agent_relationship::clear_memories_by_observer] success");
    Ok(())
}
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db/agent_relationship.rs
git commit -m "feat(db): agent_relationship repo returns memory_text, adds upsert_memory and clear_memories_by_observer"
```

---

## Task 5: Repository 层测试

**Files:**
- Modify: `src-tauri/src/db/agent_relationship.rs`（在 `#[cfg(test)]` 模块中）
- Modify: `src-tauri/src/db/agent.rs`（在 `#[cfg(test)]` 模块中，如存在）

- [ ] **Step 1: 在 agent_relationship.rs 的测试模块中新增测试**

确保 `init_test_db()` 已包含 `MIGRATION_V13`：

找到 `init_test_db` 函数（在文件底部），在 `MIGRATION_V12` 之后添加：
```rust
        conn.execute_batch(crate::db::schema::MIGRATION_V13).unwrap();
```

在测试模块末尾，添加以下测试：

```rust
    #[test]
    fn test_upsert_memory_only_updates_memory_text() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Agent One", 0i64),
        ).unwrap();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent2", "Agent Two", 0i64),
        ).unwrap();

        // 先插入一条带关系描述的记录
        upsert_relationship(&conn, "agent1", "agent2", "agent", "好朋友").unwrap();
        
        // 再更新 memory_text
        upsert_memory(&conn, "agent1", "agent2", "agent", "他喜欢吃苹果").unwrap();

        // 验证 relationship_text 没有被覆盖
        let rel_text: String = conn.query_row(
            "SELECT relationship_text FROM agent_relationships WHERE observer_id = 'agent1' AND target_id = 'agent2'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(rel_text, "好朋友", "upsert_memory should NOT overwrite relationship_text");

        // 验证 memory_text 已写入
        let mem_text: String = conn.query_row(
            "SELECT memory_text FROM agent_relationships WHERE observer_id = 'agent1' AND target_id = 'agent2'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(mem_text, "他喜欢吃苹果");
    }

    #[test]
    fn test_clear_memories_by_observer_preserves_relationships() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Agent One", 0i64),
        ).unwrap();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent2", "Agent Two", 0i64),
        ).unwrap();

        upsert_relationship(&conn, "agent1", "agent2", "agent", "好朋友").unwrap();
        upsert_memory(&conn, "agent1", "agent2", "agent", "他喜欢吃苹果").unwrap();

        clear_memories_by_observer(&conn, "agent1").unwrap();

        let mem_text: String = conn.query_row(
            "SELECT memory_text FROM agent_relationships WHERE observer_id = 'agent1'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(mem_text, "");

        let rel_text: String = conn.query_row(
            "SELECT relationship_text FROM agent_relationships WHERE observer_id = 'agent1'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(rel_text, "好朋友", "relationship_text should be preserved");
    }

    #[test]
    fn test_list_relationships_includes_memory_text() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO app_settings (id, updated_at) VALUES (1, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO user_personas (id, name, description, created_at, updated_at) VALUES ('up1', 'User', '', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "UPDATE app_settings SET active_persona_id = 'up1' WHERE id = 1",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Agent One", 0i64),
        ).unwrap();

        // 给用户人设写一条记忆
        upsert_memory(&conn, "agent1", "up1", "user_persona", "用户喜欢猫").unwrap();

        let items = list_relationships_by_observer(&conn, "agent1").unwrap();
        let user_item = items.iter().find(|i| i.target_type == "user_persona").unwrap();
        assert_eq!(user_item.memory_text, "用户喜欢猫");
    }
```

- [ ] **Step 2: 在 agent.rs 的测试模块中新增测试（如存在）**

如果 `src-tauri/src/db/agent.rs` 已有 `#[cfg(test)]` 模块，在末尾添加：

```rust
    #[test]
    fn test_agent_long_term_memory_defaults() {
        let conn = init_test_db();
        // 创建一个 agent
        let req = CreateAgentRequest {
            name: "Test".to_string(),
            avatar_path: None,
            detailed_persona: "detail".to_string(),
            simplified_persona: "simple".to_string(),
            personality: None,
            scenario: None,
            example_messages: None,
            first_message: None,
            creator_notes: None,
            tags: None,
            model_provider: "openai".to_string(),
            model_name: "gpt-4".to_string(),
            base_url: None,
            api_key: "sk-test".to_string(),
            temperature: None,
            max_tokens: None,
            thinking_mode: None,
            long_term_memory: None,
            memory_enabled: None,
        };
        let agent = create(&conn, &req).unwrap();
        assert_eq!(agent.long_term_memory, None);
        assert_eq!(agent.memory_enabled, true);
    }
```

如果 agent.rs 没有测试模块，跳过此步骤，在 Task 8 的命令层测试中覆盖。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/db/agent_relationship.rs src-tauri/src/db/agent.rs
git commit -m "test(db): add repository tests for memory fields"
```

---

## Task 6: 命令层 — update_agent_memory

**Files:**
- Modify: `src-tauri/src/commands/agent_relationship.rs`

- [ ] **Step 1: 在 commands/agent_relationship.rs 末尾新增命令**

在 `remove_friendship` 之后，添加：

```rust
#[tauri::command]
pub async fn update_agent_memory(
    state: State<'_, DbState>,
    observer_id: String,
    target_id: String,
    target_type: String,
    memory_text: String,
) -> Result<(), String> {
    crate::logger::backend("DEBUG", &format!(
        "[DEBUG update_agent_memory] observer_id={}, target_id={}, target_type={}, text_len={}",
        observer_id, target_id, target_type, memory_text.len()
    ));

    // 校验长度：500 字（按 Unicode 字符计）
    if memory_text.chars().count() > 500 {
        return Err(format!("记忆内容超过 500 字限制（当前 {} 字）", memory_text.chars().count()));
    }

    let conn = get_db(&state).await?;
    agent_relationship::upsert_memory(&conn, &observer_id, &target_id, &target_type, &memory_text)
        .map_err(|e| e.to_string())?;

    Ok(())
}
```

- [ ] **Step 2: Commit**

```bash
git add src-tauri/src/commands/agent_relationship.rs
git commit -m "feat(commands): add update_agent_memory command with 500-char limit"
```

---

## Task 7: 命令层 — reset_agent_memory + update_agent 扩展

**Files:**
- Modify: `src-tauri/src/commands/agent.rs`

- [ ] **Step 1: 在 commands/agent.rs 中新增 `reset_agent_memory` 命令**

在 `test_api_connection` 之前（或文件末尾），添加：

```rust
#[tauri::command]
pub async fn reset_agent_memory(
    state: State<'_, DbState>,
    agent_id: String,
) -> Result<(), String> {
    crate::logger::backend("DEBUG", &format!("[DEBUG reset_agent_memory] agent_id={}", agent_id));

    let conn = get_db(&state).await?;
    
    // 1. 清空长期记忆
    agent_repo::clear_long_term_memory(&conn, &agent_id)
        .map_err(|e| e.to_string())?;
    
    // 2. 清空所有对他人的记忆
    agent_relationship::clear_memories_by_observer(&conn, &agent_id)
        .map_err(|e| e.to_string())?;

    crate::logger::backend("DEBUG", &format!("[DEBUG reset_agent_memory] success agent_id={}", agent_id));
    Ok(())
}
```

注意：此命令需要在文件顶部添加 `use crate::db::agent_relationship;`（如果尚未导入）。

检查文件顶部已有导入：
```rust
use crate::db::agent as agent_repo;
use crate::db::user_persona as user_persona_repo;
```
需要添加：
```rust
use crate::db::agent_relationship;
```

- [ ] **Step 2: Commit**

```bash
git add src-tauri/src/commands/agent.rs
git commit -m "feat(commands): add reset_agent_memory command"
```

---

## Task 8: 命令层测试

**Files:**
- Modify: `src-tauri/src/commands/agent_relationship.rs`（在 `#[cfg(test)]` 模块中）

- [ ] **Step 1: 在 commands/agent_relationship.rs 中添加测试模块**

如果文件底部没有 `#[cfg(test)]` 模块，在文件末尾添加：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use rusqlite::Connection;
    use crate::db::connection::DbState;
    use crate::db::schema::{MIGRATION_V1, MIGRATION_V2, MIGRATION_V3, MIGRATION_V4, MIGRATION_V5, MIGRATION_V6, MIGRATION_V7, MIGRATION_V8, MIGRATION_V9, MIGRATION_V11, MIGRATION_V12, MIGRATION_V13};

    fn init_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = OFF;", []).unwrap();
        conn.execute_batch(MIGRATION_V1).unwrap();
        conn.execute_batch(MIGRATION_V2).unwrap();
        conn.execute_batch(MIGRATION_V3).unwrap();
        conn.execute_batch(MIGRATION_V4).unwrap();
        conn.execute_batch(MIGRATION_V5).unwrap();
        conn.execute_batch(MIGRATION_V6).unwrap();
        conn.execute_batch(MIGRATION_V7).unwrap();
        conn.execute_batch(MIGRATION_V8).unwrap();
        conn.execute_batch(MIGRATION_V9).unwrap();
        conn.execute_batch(MIGRATION_V11).unwrap();
        conn.execute_batch(MIGRATION_V12).unwrap();
        conn.execute_batch(MIGRATION_V13).unwrap();
        conn
    }

    fn make_db_state(conn: Connection) -> DbState {
        DbState(Arc::new(Mutex::new(conn)))
    }

    fn create_test_agent(conn: &Connection, agent_id: &str, name: &str) {
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            (agent_id, name, 0i64),
        ).unwrap();
    }

    #[tokio::test]
    async fn test_update_agent_memory_enforces_500_char_limit() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent1", "Alice");
        create_test_agent(&conn, "agent2", "Bob");
        let db_state = make_db_state(conn);

        let long_text = "a".repeat(501);
        let result = update_agent_memory(
            db_state, "agent1".to_string(), "agent2".to_string(), "agent".to_string(), long_text,
        ).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("500"));
    }

    #[tokio::test]
    async fn test_update_agent_memory_saves_within_limit() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent1", "Alice");
        create_test_agent(&conn, "agent2", "Bob");
        let db_state = make_db_state(conn);

        let result = update_agent_memory(
            db_state, "agent1".to_string(), "agent2".to_string(), "agent".to_string(), "他喜欢吃苹果".to_string(),
        ).await;

        assert!(result.is_ok());
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add src-tauri/src/commands/agent_relationship.rs
git commit -m "test(commands): add tests for update_agent_memory command"
```

---

## Task 9: 注册命令到 lib.rs

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 在 lib.rs 中导入新命令**

找到：
```rust
use commands::agent_relationship::{list_agent_relationships, update_agent_relationship, add_friendships, remove_friendship};
```
改为：
```rust
use commands::agent_relationship::{list_agent_relationships, update_agent_relationship, add_friendships, remove_friendship, update_agent_memory};
```

找到：
```rust
use commands::agent::{create_agent, delete_agent, get_agent, list_agents, update_agent, test_api_connection};
```
改为：
```rust
use commands::agent::{create_agent, delete_agent, get_agent, list_agents, update_agent, test_api_connection, reset_agent_memory};
```

- [ ] **Step 2: 在 generate_handler! 宏中注册**

找到：
```rust
            list_agent_relationships,
            update_agent_relationship,
            add_friendships,
            remove_friendship,
```
改为：
```rust
            list_agent_relationships,
            update_agent_relationship,
            add_friendships,
            remove_friendship,
            update_agent_memory,
```

找到 `delete_agent,` 之后，添加：
```rust
            reset_agent_memory,
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(lib): register update_agent_memory and reset_agent_memory commands"
```

---

## Task 10: 前端 — AgentMemoryPanel 组件

**Files:**
- **Create**: `src/lib/components/AgentMemoryPanel.svelte`

- [ ] **Step 1: 创建 AgentMemoryPanel.svelte**

```svelte
<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { Bot, User } from 'lucide-svelte';
    import { resolveAvatarUrl } from '$lib/utils';
    import { logger } from '$lib/logger';
    import type { RelationshipItem } from '$lib/types';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import ConfirmResetMemoryModal from './ConfirmResetMemoryModal.svelte';

    let { agentId, longTermMemory = $bindable(''), memoryEnabled = $bindable(true) }: {
        agentId: string;
        longTermMemory: string;
        memoryEnabled: boolean;
    } = $props();

    let items = $state<RelationshipItem[]>([]);
    let loading = $state(false);
    let error = $state('');
    let saveTimeouts = $state<Record<string, ReturnType<typeof setTimeout>>>({});
    let showResetModal = $state(false);

    async function loadRelationships() {
        loading = true;
        error = '';
        try {
            const result = await invoke<RelationshipItem[]>('list_agent_relationships', { agentId });
            items = result;
        } catch (err) {
            logger.error('Failed to load relationships for memory panel:', err);
            error = '加载记忆列表失败';
        } finally {
            loading = false;
        }
    }

    async function saveLongTermMemory() {
        try {
            await invoke('update_agent', { req: { id: agentId, long_term_memory: longTermMemory } });
        } catch (err) {
            logger.error('Failed to save long term memory:', err);
            toastStore.show('保存长期记忆失败', 'error');
        }
    }

    async function saveMemory(item: RelationshipItem) {
        try {
            await invoke('update_agent_memory', {
                observerId: agentId,
                targetId: item.target_id,
                targetType: item.target_type,
                memoryText: item.memory_text,
            });
        } catch (err) {
            logger.error('Failed to save memory:', err);
            toastStore.show('保存记忆失败', 'error');
        }
    }

    function handleLongTermInput(value: string) {
        longTermMemory = value;
        if (saveTimeouts['long_term']) {
            clearTimeout(saveTimeouts['long_term']);
        }
        saveTimeouts['long_term'] = setTimeout(() => {
            saveLongTermMemory();
        }, 1000);
    }

    function handleLongTermBlur() {
        if (saveTimeouts['long_term']) {
            clearTimeout(saveTimeouts['long_term']);
            delete saveTimeouts['long_term'];
        }
        saveLongTermMemory();
    }

    function handleMemoryInput(item: RelationshipItem, value: string) {
        item.memory_text = value;
        const key = `${item.target_type}:${item.target_id}`;
        if (saveTimeouts[key]) {
            clearTimeout(saveTimeouts[key]);
        }
        saveTimeouts[key] = setTimeout(() => {
            saveMemory(item);
        }, 500);
    }

    function handleMemoryBlur(item: RelationshipItem) {
        const key = `${item.target_type}:${item.target_id}`;
        if (saveTimeouts[key]) {
            clearTimeout(saveTimeouts[key]);
            delete saveTimeouts[key];
        }
        saveMemory(item);
    }

    async function handleReset() {
        try {
            await invoke('reset_agent_memory', { agentId });
            longTermMemory = '';
            items = items.map(i => ({ ...i, memory_text: '' }));
            toastStore.show('记忆已重置', 'success');
        } catch (err) {
            logger.error('Failed to reset memory:', err);
            toastStore.show('重置记忆失败', 'error');
        }
    }

    async function handleToggleEnabled() {
        memoryEnabled = !memoryEnabled;
        try {
            await invoke('update_agent', { req: { id: agentId, memory_enabled: memoryEnabled } });
        } catch (err) {
            logger.error('Failed to update memory_enabled:', err);
            toastStore.show('更新设置失败', 'error');
            memoryEnabled = !memoryEnabled; // rollback
        }
    }

    $effect(() => {
        if (agentId) {
            loadRelationships();
        }
    });
</script>

<div class="max-w-2xl space-y-6">
    {#if error}
        <div class="mb-4 p-3 bg-red-50 text-red-600 rounded-lg text-sm">{error}</div>
    {/if}

    <!-- Controls -->
    <div class="flex items-center justify-between p-3 bg-surface border border-border rounded-lg">
        <div class="flex items-center gap-3">
            <input
                id="memory-enabled"
                type="checkbox"
                checked={memoryEnabled}
                onchange={handleToggleEnabled}
                class="w-4 h-4 rounded border-border text-primary focus:ring-primary"
            />
            <label for="memory-enabled" class="text-sm font-medium">启用记忆</label>
        </div>
        <button
            onclick={() => showResetModal = true}
            class="text-sm text-red-600 hover:text-red-700 hover:bg-red-50 px-3 py-1.5 rounded-lg transition-colors"
        >
            重置记忆
        </button>
    </div>

    {#if !memoryEnabled}
        <div class="text-sm text-text-secondary bg-gray-50 p-3 rounded-lg">
            记忆功能已关闭，当前内容不会被使用
        </div>
    {/if}

    <!-- Long-term Memory -->
    <div>
        <h3 class="text-sm font-medium text-text-secondary mb-3 uppercase tracking-wide">长期记忆</h3>
        <div class="relative">
            <textarea
                value={longTermMemory}
                oninput={(e) => handleLongTermInput((e.target as HTMLTextAreaElement).value)}
                onblur={handleLongTermBlur}
                rows={8}
                maxlength={3000}
                disabled={!memoryEnabled}
                class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none bg-surface disabled:opacity-50 disabled:cursor-not-allowed"
                placeholder="和该角色有关的记忆"
            ></textarea>
            <div class="absolute bottom-2 right-2 text-[10px] text-text-secondary">
                {longTermMemory.length}/3000
            </div>
        </div>
    </div>

    <!-- Memory about others -->
    <div>
        <h3 class="text-sm font-medium text-text-secondary mb-3 uppercase tracking-wide">对他人的记忆</h3>
        {#if loading}
            <div class="text-text-secondary text-sm py-4">加载中...</div>
        {:else if items.length === 0}
            <div class="text-text-secondary text-sm py-8 text-center">
                <p>该角色尚未与其他参与者建立关联</p>
                <p class="mt-1">在群聊或私聊中会自动显示</p>
            </div>
        {:else}
            <div class="space-y-3">
                {#each items as item (item.target_id + item.target_type)}
                    <div class="flex items-start gap-3 p-3 bg-surface border border-border rounded-lg">
                        <div class="w-9 h-9 rounded-full bg-primary/10 flex-shrink-0 flex items-center justify-center overflow-hidden">
                            {#if item.target_avatar}
                                <img src={resolveAvatarUrl(item.target_avatar)} alt={item.target_name} class="w-full h-full object-cover" />
                            {:else if item.target_type === 'user_persona'}
                                <User size={18} class="text-primary" />
                            {:else}
                                <Bot size={18} class="text-primary" />
                            {/if}
                        </div>
                        <div class="flex-1 min-w-0">
                            <div class="flex items-center gap-2 mb-1.5">
                                <span class="text-sm font-medium truncate">{item.target_name}</span>
                                <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-gray-100 text-text-secondary">
                                    {item.target_label}
                                </span>
                            </div>
                            <div class="relative">
                                <textarea
                                    value={item.memory_text}
                                    oninput={(e) => handleMemoryInput(item, (e.target as HTMLTextAreaElement).value)}
                                    onblur={() => handleMemoryBlur(item)}
                                    rows={3}
                                    maxlength={500}
                                    disabled={!memoryEnabled}
                                    class="w-full px-2.5 py-1.5 text-sm border border-border rounded-md focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none bg-bg disabled:opacity-50 disabled:cursor-not-allowed"
                                    placeholder="关于此人的重要信息，如喜好、习惯、共同经历..."
                                ></textarea>
                                <div class="absolute bottom-1 right-2 text-[10px] text-text-secondary">
                                    {item.memory_text.length}/500
                                </div>
                            </div>
                        </div>
                    </div>
                {/each}
            </div>
        {/if}
    </div>
</div>

<ConfirmResetMemoryModal
    open={showResetModal}
    onClose={() => showResetModal = false}
    onConfirm={async () => {
        await handleReset();
        showResetModal = false;
    }}
/>
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/components/AgentMemoryPanel.svelte
git commit -m "feat(ui): add AgentMemoryPanel component for memory tab"
```

---

## Task 11: 前端 — ConfirmResetMemoryModal 组件

**Files:**
- **Create**: `src/lib/components/ConfirmResetMemoryModal.svelte`

- [ ] **Step 1: 创建 ConfirmResetMemoryModal.svelte**

```svelte
<script lang="ts">
    let { open, onClose, onConfirm }: {
        open: boolean;
        onClose: () => void;
        onConfirm: () => void;
    } = $props();
</script>

{#if open}
    <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
        <div class="bg-surface rounded-xl shadow-lg p-6 w-full max-w-sm mx-4">
            <h3 class="text-lg font-semibold mb-2">确认重置记忆</h3>
            <p class="text-sm text-text-secondary mb-6">
                此操作将清空该角色的长期记忆和所有对他人的记忆，且无法撤销。是否继续？
            </p>
            <div class="flex justify-end gap-3">
                <button
                    onclick={onClose}
                    class="px-4 py-2 text-sm text-text-secondary hover:bg-gray-100 rounded-lg transition-colors"
                >
                    取消
                </button>
                <button
                    onclick={onConfirm}
                    class="px-4 py-2 text-sm bg-red-600 text-white hover:bg-red-700 rounded-lg transition-colors"
                >
                    确认重置
                </button>
            </div>
        </div>
    </div>
{/if}
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/components/ConfirmResetMemoryModal.svelte
git commit -m "feat(ui): add ConfirmResetMemoryModal component"
```

---

## Task 12: 前端 — AgentDetail 标签页扩展

**Files:**
- Modify: `src/lib/components/AgentDetail.svelte`

- [ ] **Step 1: 导入新组件**

在 `<script>` 顶部的 import 区域，添加：

```typescript
    import AgentMemoryPanel from './AgentMemoryPanel.svelte';
```

- [ ] **Step 2: 扩展 activeTab 类型和状态**

将：
```typescript
    let activeTab = $state<'config' | 'relationships'>('config');
```
改为：
```typescript
    let activeTab = $state<'config' | 'relationships' | 'memory'>('config');
```

- [ ] **Step 3: 在 form 状态中添加 long_term_memory**

在 `form` 对象中，`thinking_mode` 之后，添加：

```typescript
        long_term_memory: '',
        memory_enabled: true,
```

- [ ] **Step 4: 在 loadAgent 中加载新字段**

在 `loadAgent` 函数中，`form` 赋值处，`thinking_mode` 之后，添加：

```typescript
                    long_term_memory: result.long_term_memory || '',
                    memory_enabled: result.memory_enabled ?? true,
```

- [ ] **Step 5: 在 Tab 栏添加记忆标签**

在现有两个 `<button>` 之后，添加第三个：

```svelte
                <button
                    onclick={() => activeTab = 'memory'}
                    class="py-2 text-sm font-medium border-b-2 transition-colors {activeTab === 'memory' ? 'border-primary text-primary' : 'border-transparent text-text-secondary hover:text-text-primary'}"
                >
                    记忆
                </button>
```

- [ ] **Step 6: 在内容区域添加记忆标签页内容**

在 `{:else if activeTab === 'relationships'}` 的代码块之后，添加：

```svelte
            {:else if activeTab === 'memory'}
                <AgentMemoryPanel
                    agentId={agent.id}
                    bind:longTermMemory={form.long_term_memory}
                    bind:memoryEnabled={form.memory_enabled}
                />
```

- [ ] **Step 7: 调整 Footer actions**

当前 Footer 的保存按钮只在 `activeTab === 'config'` 时显示。由于长期记忆和对他人的记忆都是自动保存的，记忆标签页不需要额外的保存按钮。但取消按钮仍然应该显示。

找到 Footer 区域：
```svelte
            <div class="flex gap-3">
                <button onclick={() => appState.selectAgent(null)} class="px-4 py-2 text-text-secondary hover:bg-gray-100 rounded-lg transition-colors">
                    取消
                </button>
                {#if activeTab === 'config'}
                    <button onclick={handleSave} disabled={saving}
```

不需要修改——取消按钮始终显示，保存按钮只在配置页显示，这是正确的。记忆标签页的内容通过 debounce 自动保存。

- [ ] **Step 8: Commit**

```bash
git add src/lib/components/AgentDetail.svelte
git commit -m "feat(ui): add memory tab to AgentDetail with AgentMemoryPanel"
```

---

## Task 13: 更新 feature_list.md

**Files:**
- Modify: `docs/feature_list.md`

- [ ] **Step 1: 将 AGT-18 状态从"设计中"更新为"已实现"**

找到：
```
| AGT-18 | 记忆系统数据模型与 UI 配置 | ... | P0 | 🚧 设计中 | ... |
```
改为：
```
| AGT-18 | 记忆系统数据模型与 UI 配置 | ... | P0 | ✅ 已实现 | Migration V13 + agent/agent_relationship repo + update_agent_memory / reset_agent_memory 命令 + AgentMemoryPanel UI |
```

- [ ] **Step 2: Commit**

```bash
git add docs/feature_list.md
git commit -m "docs(feature_list): mark AGT-18 as implemented"
```

---

## 自我审查

### Spec 覆盖检查

| Spec 要求 | 对应任务 | 状态 |
|-----------|----------|------|
| Migration V13：`long_term_memory`, `memory_enabled`, `memory_text` | Task 1 | ✅ |
| Agent 模型增加字段 | Task 2 | ✅ |
| RelationshipItem 增加 `memory_text` | Task 2 | ✅ |
| TypeScript 类型同步 | Task 2 | ✅ |
| agent.rs repo 支持新字段 + `clear_long_term_memory` | Task 3 | ✅ |
| agent_relationship.rs repo 返回 `memory_text` + `upsert_memory` + `clear_memories_by_observer` | Task 4 | ✅ |
| Repository 测试 | Task 5 | ✅ |
| `update_agent_memory` 命令（500字限制） | Task 6 | ✅ |
| `reset_agent_memory` 命令 | Task 7 | ✅ |
| 命令测试 | Task 8 | ✅ |
| lib.rs 注册命令 | Task 9 | ✅ |
| AgentMemoryPanel：开关、重置、长期记忆、他人记忆 | Task 10 | ✅ |
| ConfirmResetMemoryModal | Task 11 | ✅ |
| AgentDetail 标签页扩展 | Task 12 | ✅ |
| feature_list 更新 | Task 13 | ✅ |

### Placeholder 扫描

- 无 TBD / TODO / "implement later" / "fill in details" ✅
- 无 "add appropriate error handling" 等模糊描述 ✅
- 所有代码块包含完整可运行代码 ✅
- 无 "Similar to Task N" 引用 ✅

### 类型一致性检查

| 位置 | 字段名 | 类型 | 一致 |
|------|--------|------|------|
| Schema | `long_term_memory` | TEXT | ✅ |
| Agent struct | `long_term_memory` | Option<String> | ✅ |
| AgentResponse | `long_term_memory` | Option<String> | ✅ |
| UpdateAgentRequest | `long_term_memory` | Option<String> | ✅ |
| TypeScript Agent | `long_term_memory` | string? | ✅ |
| Schema | `memory_enabled` | INTEGER (0/1) | ✅ |
| Agent struct | `memory_enabled` | bool | ✅ |
| TypeScript Agent | `memory_enabled` | boolean? | ✅ |
| Schema | `memory_text` | TEXT | ✅ |
| RelationshipItem | `memory_text` | String | ✅ |
| TypeScript RelationshipItem | `memory_text` | string | ✅ |

---

## 执行方式选择

**计划已保存到 `docs/superpowers/plans/2026-05-20-memory-system-data-model.md`。两个执行选项：**

**1. Subagent-Driven（推荐）** — 每个 Task 分派一个独立的 subagent，我在每轮之间做审查和集成

**2. Inline Execution** — 在当前会话中直接按顺序执行，我可以批量处理相关任务

你希望采用哪种方式？如果选择 **Subagent-Driven**，我会按照 Task 顺序逐个派发 subagent，每个 subagent 只负责一个 Task 的全部步骤（含测试）。
