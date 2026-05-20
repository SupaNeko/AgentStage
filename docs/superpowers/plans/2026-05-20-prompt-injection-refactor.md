# CHAT-39-ext Prompt 注入与关系描述重构 — 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 PromptAssembler 中注入角色长期记忆，重构参与者介绍为结构化格式（[印象] / [记忆]），并复用 list_relationships_by_observer 作为参与者数据源。

**Architecture:** `get_participants` 重构为调用 `list_relationships_by_observer` 返回 `Vec<RelationshipItem>`。为此 `RelationshipItem` 需新增 `target_simplified_persona` 字段。Layer 2 后增加长期记忆层。Layer 3 格式从单行重构为多行结构化输出。

**Tech Stack:** Rust (Tauri v2) + rusqlite + Svelte 5

---

## 文件结构映射

| 文件 | 变更 | 职责 |
|------|------|------|
| `src-tauri/src/models/agent_relationship.rs` | 修改 | `RelationshipItem` 新增 `target_simplified_persona` |
| `src-tauri/src/db/agent_relationship.rs` | 修改 | `list_relationships_by_observer` 返回 `target_simplified_persona` |
| `src-tauri/src/llm/prompt_templates.rs` | 修改 | 新增 `LAYER_MEMORY_TITLE`、`MEMORY_PREFIX` 常量 |
| `src-tauri/src/llm/prompt.rs` | 修改 | 重构 `get_participants`，修改 Layer 2 和 Layer 3 的拼接逻辑 |
| `src/lib/types.ts` | 修改 | `RelationshipItem` TypeScript 类型新增字段 |

---

## 关键设计决策

`list_relationships_by_observer` 当前返回的 `RelationshipItem` 不包含 `simplified_persona`（简易人设），而 PromptAssembler 的 Layer 3 需要它。解决方案：

- `RelationshipItem` 新增 `target_simplified_persona: String`
- `list_relationships_by_observer` 在三个 UNION ALL 子查询中增加 `COALESCE(a.simplified_persona, '') as target_simplified_persona`
- `get_participants` 直接调用 `list_relationships_by_observer`，无需再自行查询
- 前端 `RelationshipItem` 类型同步新增字段（不影响现有 UI，因为前端可忽略此字段）

---

## Task 1: 扩展 RelationshipItem 和 list_relationships_by_observer

**Files:**
- Modify: `src-tauri/src/models/agent_relationship.rs`
- Modify: `src-tauri/src/db/agent_relationship.rs`
- Modify: `src/lib/types.ts`

- [ ] **Step 1: 修改 `RelationshipItem` 结构体**

在 `src-tauri/src/models/agent_relationship.rs` 中，在 `target_label` 之后、`relationship_text` 之前，添加：

```rust
    pub target_simplified_persona: String,
```

- [ ] **Step 2: 修改 `list_relationships_by_observer`**

在 `src-tauri/src/db/agent_relationship.rs` 中：

1. 外层 SELECT 增加 `target_simplified_persona`：
```rust
        SELECT target_id, target_type, target_name, target_avatar, target_label, target_simplified_persona, relationship_text, memory_text, updated_at
```

2. 三个 UNION ALL 子查询中，每个都在 `target_label` 之后增加：
```rust
                COALESCE(a.simplified_persona, '') as target_simplified_persona,
```

对于用户人设子查询（第一个 UNION），`a` 表不存在，使用空字符串：
```rust
                '' as target_simplified_persona,
```

3. `query_map` 闭包中，在 `target_label` 之后、`relationship_text` 之前，添加：
```rust
            target_simplified_persona: row.get("target_simplified_persona")?,
```

- [ ] **Step 3: 修改 TypeScript 类型**

在 `src/lib/types.ts` 中，`RelationshipItem` 接口在 `target_label` 之后、`relationship_text` 之前，添加：
```typescript
    target_simplified_persona: string;
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/models/agent_relationship.rs src-tauri/src/db/agent_relationship.rs src/lib/types.ts
git commit -m "feat(models): add target_simplified_persona to RelationshipItem"
```

---

## Task 2: prompt_templates.rs 新增常量

**Files:**
- Modify: `src-tauri/src/llm/prompt_templates.rs`

- [ ] **Step 1: 在文件末尾新增常量**

在 `pub const UNKNOWN_TYPE_PREFIX` 之后，添加：

```rust
pub const LAYER_MEMORY_TITLE: &str = "【关于你的记忆】";
pub const MEMORY_PREFIX: &str = "\n  [记忆]：";
```

- [ ] **Step 2: Commit**

```bash
git add src-tauri/src/llm/prompt_templates.rs
git commit -m "feat(prompt): add LAYER_MEMORY_TITLE and MEMORY_PREFIX constants"
```

---

## Task 3: prompt.rs — 重构 get_participants 和 Layer 拼接

**Files:**
- Modify: `src-tauri/src/llm/prompt.rs`

- [ ] **Step 1: 修改 `get_participants` 签名和实现**

将 `get_participants` 的签名从：
```rust
fn get_participants(
    conn: &Connection,
    agent_id: &str,
) -> Result<Vec<(String, String, String, String)>, String> {
```
改为：
```rust
fn get_participants(
    conn: &Connection,
    agent_id: &str,
) -> Result<Vec<RelationshipItem>, String> {
```

将 `get_participants` 的整个函数体替换为：
```rust
    crate::db::agent_relationship::list_relationships_by_observer(conn, agent_id)
        .map_err(|e| e.to_string())
```

删除 `get_participants` 中原有的所有私聊/群友查询逻辑（从 `let mut seen: HashSet<String>` 到函数末尾的所有代码，约 100 行）。

- [ ] **Step 2: 修改 Layer 2 注入长期记忆**

找到 Layer 2 的代码：
```rust
        // Layer 2: Self Persona
        let agent = Self::get_agent(conn, agent_id)?;
        layers.push(format!("{}\n{}", prompt_templates::LAYER_PERSONA_TITLE, agent.detailed_persona));
```

改为：
```rust
        // Layer 2: Self Persona
        let agent = Self::get_agent(conn, agent_id)?;
        layers.push(format!("{}\n{}", prompt_templates::LAYER_PERSONA_TITLE, agent.detailed_persona));

        // Layer 2.5: Long-term Memory
        if agent.memory_enabled {
            if let Some(ref mem) = agent.long_term_memory {
                if !mem.is_empty() {
                    layers.push(format!("{}\n{}", prompt_templates::LAYER_MEMORY_TITLE, mem));
                }
            }
        }
```

- [ ] **Step 3: 修改 Layer 3 重构为结构化输出**

找到 Layer 3 的代码：
```rust
        // Layer 3: Participants Introduction
        let participants = Self::get_participants(conn, agent_id)?;
        if !participants.is_empty() {
            let mut layer = String::from(prompt_templates::LAYER_PARTICIPANTS_TITLE);
            layer.push('\n');
                for (name, relation, persona, rel_text) in participants {
                if rel_text.is_empty() {
                    layer.push_str(&format!("- {}（{}）：{}\n", name, relation, persona));
                } else {
                    layer.push_str(&format!("- {}（{}）：{}{}{}\n", name, relation, persona, prompt_templates::RELATIONSHIP_SUFFIX_PREFIX, rel_text));
                }
            }
            layers.push(layer);
        }
```

改为：
```rust
        // Layer 3: Participants Introduction
        let participants = Self::get_participants(conn, agent_id)?;
        if !participants.is_empty() {
            let mut layer = String::from(prompt_templates::LAYER_PARTICIPANTS_TITLE);
            layer.push('\n');
            for item in participants {
                layer.push_str(&format!(
                    "- {}（{}）：{}\n",
                    item.target_name, item.target_label, item.target_simplified_persona
                ));
                if !item.relationship_text.is_empty() {
                    layer.push_str(&format!("  [印象]：{}\n", item.relationship_text));
                }
                if agent.memory_enabled && !item.memory_text.is_empty() {
                    layer.push_str(&format!("  [记忆]：{}\n", item.memory_text));
                }
            }
            layers.push(layer);
        }
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/llm/prompt.rs
git commit -m "feat(prompt): inject long-term memory, refactor participants to structured format"
```

---

## Task 4: 更新 prompt.rs 测试模块

**Files:**
- Modify: `src-tauri/src/llm/prompt.rs`（`#[cfg(test)]` 模块）

- [ ] **Step 1: 更新 `init_test_db` 包含 MIGRATION_V13**

找到 `init_test_db` 函数，在 `MIGRATION_V11` 之后添加：
```rust
        conn.execute_batch(crate::db::schema::MIGRATION_V13).unwrap();
```

- [ ] **Step 2: 新增 3 个测试**

在测试模块末尾，添加：

```rust
    #[test]
    fn test_prompt_includes_long_term_memory() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, long_term_memory, memory_enabled, created_at, updated_at) VALUES (?1, ?2, ?3, '', '喜欢吃甜食', 1, ?4, ?4)",
            ("agent1", "Test Agent", "A test persona", 0i64),
        ).unwrap();
        insert_session(&conn, "sess1", "private");
        insert_private_session(&conn, "sess1", "agent1", 0);
        insert_session_settings(&conn, "sess1", 50);

        let prompt = PromptAssembler::assemble(&conn, "agent1", None, None, &[], &std::collections::HashSet::new()).unwrap();
        assert!(prompt.contains("【关于你的记忆】"), "Prompt should contain memory section");
        assert!(prompt.contains("喜欢吃甜食"), "Prompt should contain long-term memory content");
    }

    #[test]
    fn test_prompt_skips_memory_when_disabled() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, long_term_memory, memory_enabled, created_at, updated_at) VALUES (?1, ?2, ?3, '', '喜欢吃甜食', 0, ?4, ?4)",
            ("agent1", "Test Agent", "A test persona", 0i64),
        ).unwrap();
        insert_session(&conn, "sess1", "private");
        insert_private_session(&conn, "sess1", "agent1", 0);
        insert_session_settings(&conn, "sess1", 50);

        let prompt = PromptAssembler::assemble(&conn, "agent1", None, None, &[], &std::collections::HashSet::new()).unwrap();
        assert!(!prompt.contains("【关于你的记忆】"), "Prompt should NOT contain memory section when disabled");
        assert!(!prompt.contains("喜欢吃甜食"), "Prompt should NOT contain memory content when disabled");
    }

    #[test]
    fn test_prompt_structured_participants() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, ?3, ' Bob 的简介', ?4, ?4)",
            ("agent1", "Alice", "Alice persona", 0i64),
        ).unwrap();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, ?3, ' Bob 的简介', ?4, ?4)",
            ("agent2", "Bob", "Bob persona", 0i64),
        ).unwrap();
        // 建立好友关系
        let now = 0i64;
        conn.execute(
            "INSERT INTO friendships (id, agent_id_1, agent_id_2, participant_type_2, created_at, source_session_id) VALUES ('f1', 'agent1', 'agent2', 'agent', ?1, NULL)",
            [now],
        ).unwrap();
        // 建立关系描述和记忆
        conn.execute(
            "INSERT INTO agent_relationships (observer_id, target_id, target_type, relationship_text, memory_text, updated_at) VALUES ('agent1', 'agent2', 'agent', '好朋友', '他喜欢吃苹果', ?1)",
            [now],
        ).unwrap();

        insert_session(&conn, "sess1", "private");
        insert_private_session(&conn, "sess1", "agent1", 0);
        insert_session_settings(&conn, "sess1", 50);

        let prompt = PromptAssembler::assemble(&conn, "agent1", None, None, &[], &std::collections::HashSet::new()).unwrap();
        println!("{}", prompt);
        assert!(prompt.contains("- Bob（好友）： Bob 的简介"), "Participant line should include name, label, and simplified persona");
        assert!(prompt.contains("[印象]：好朋友"), "Should contain [印象] line");
        assert!(prompt.contains("[记忆]：他喜欢吃苹果"), "Should contain [记忆] line");
    }
```

注意：`agent1` 的 INSERT 中 `simplified_persona` 是空字符串，`agent2` 的 `simplified_persona` 是 `' Bob 的简介'`（前面有空格，因为 SQL 参数绑定）。实际上应该用正确的参数绑定方式。修正如下：

```rust
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            ("agent1", "Alice", "Alice persona", "", 0i64),
        ).unwrap();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            ("agent2", "Bob", "Bob persona", "Bob 的简介", 0i64),
        ).unwrap();
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/llm/prompt.rs
git commit -m "test(prompt): add tests for memory injection and structured participants"
```

---

## 自我审查

### Spec 覆盖检查

| Spec 要求 | 对应任务 | 状态 |
|-----------|----------|------|
| `RelationshipItem` 新增 `target_simplified_persona` | Task 1 | ✅ |
| `list_relationships_by_observer` 返回 `target_simplified_persona` | Task 1 | ✅ |
| `get_participants` 复用 `list_relationships_by_observer` | Task 3 | ✅ |
| Layer 2 后注入长期记忆 | Task 3 | ✅ |
| Layer 3 重构为 `[印象]` / `[记忆]` 结构化输出 | Task 3 | ✅ |
| `memory_enabled = false` 时跳过所有记忆注入 | Task 3 | ✅ |
| 测试覆盖 | Task 4 | ✅ |

### Placeholder 扫描

- 无 TBD / TODO ✅
- 所有代码块包含完整代码 ✅

### 类型一致性

| 位置 | 字段名 | 类型 | 一致 |
|------|--------|------|------|
| Rust `RelationshipItem` | `target_simplified_persona` | String | ✅ |
| TS `RelationshipItem` | `target_simplified_persona` | string | ✅ |
| `list_relationships_by_observer` 查询 | `target_simplified_persona` | COALESCE(a.simplified_persona, '') | ✅ |

---

## 执行方式选择

**计划已保存到 `docs/superpowers/plans/2026-05-20-prompt-injection-refactor.md`。两个执行选项：**

**1. Subagent-Driven（推荐）** — 每个 Task 分派独立 subagent

**2. Inline Execution** — 在当前会话中直接按顺序执行

你希望采用哪种方式？
