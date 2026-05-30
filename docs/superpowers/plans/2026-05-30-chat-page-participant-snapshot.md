# Chat Page Participant Snapshot Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reset session generates a participant snapshot into `chat_page_participants`; history view and history-mode chat use snapshot state instead of current session members.

**Architecture:** Add a `chat_page_participants` snapshot table keyed by `chat_page_id`. On `reset_session`, copy current session members into the snapshot. `resolve_history_target_agents`, `HistoryPromptAssembler`, and message rendering query the snapshot, falling back gracefully for pre-migration pages.

**Tech Stack:** Rust (Tauri v2), SQLite, Svelte 5, TailwindCSS v4

**Scope Clarifications (post-review):**
- `HistorySessionList` snapshot display: **Out of scope** for this cycle. Design doc lists it as "optional enhancement"; the current implementation uses live session members which is acceptable for now.
- Data migration for existing `page_index=0` pages: **Out of scope**. These pages automatically fall back to current session members (backward compatibility by design).

---

## File Map

| File | Role |
|------|------|
| `src-tauri/src/db/schema.rs` | Add `chat_page_participants` DDL to `BASE_SCHEMA` and `MIGRATION_V22` |
| `src-tauri/src/db/migration.rs` | Register V22 migration |
| `src-tauri/src/db/chat_page_participant.rs` | **New** — Repository: `insert_snapshot`, `list_by_chat_page`, `get_chat_page_id` |
| `src-tauri/src/db/mod.rs` | Export `chat_page_participant` module |
| `src-tauri/src/db/session.rs` | `reset_session` inserts snapshot after creating new `chat_page` |
| `src-tauri/src/commands/message.rs` | `resolve_history_target_agents` queries snapshot; new `get_chat_page_id` helper |
| `src-tauri/src/llm/history_prompt.rs` | `HistoryPromptAssembler` uses snapshot participants for sender names **and** participant introduction layer |
| `src-tauri/tests/chat_page_snapshot_tests.rs` | **New** — Integration tests for snapshot + history query |
| `src-tauri/src/commands/chat_page.rs` | **New** — Tauri commands: `get_chat_page_id`, `list_chat_page_participants` |
| `src-tauri/src/lib.rs` | Register new Tauri commands in `generate_handler!` |
| `src/lib/stores/sessionStore.svelte.ts` | Add `pageParticipants: Map<chatPageId, Participant[]>` and `loadPageParticipants` |
| `src/lib/components/ChatView.svelte` | History mode: load snapshots; pass snapshot data to `MessageBubble` |
| `src/lib/components/MessageBubble.svelte` | Accept `snapshotName`/`snapshotAvatar` props; fallback to "未知角色" |

---

## Task 1: Database Schema and Migration V22

**Files:**
- Modify: `src-tauri/src/db/schema.rs`
- Modify: `src-tauri/src/db/migration.rs`

- [ ] **Step 1: Add `chat_page_participants` to `BASE_SCHEMA`**

Insert after `chat_pages` table definition in `BASE_SCHEMA`:

```rust
-- ========== 13a. chat_page_participants ==========
CREATE TABLE chat_page_participants (
    chat_page_id TEXT NOT NULL,
    participant_id TEXT NOT NULL,
    participant_type TEXT NOT NULL CHECK(participant_type IN ('user', 'agent')),
    participant_name TEXT NOT NULL,
    participant_avatar TEXT,
    PRIMARY KEY (chat_page_id, participant_id, participant_type),
    FOREIGN KEY (chat_page_id) REFERENCES chat_pages(id) ON DELETE CASCADE
);
```

- [ ] **Step 2: Add `MIGRATION_V22`**

```rust
pub const MIGRATION_V22: &str = r#"
-- V22: Chat page participant snapshots
CREATE TABLE chat_page_participants (
    chat_page_id TEXT NOT NULL,
    participant_id TEXT NOT NULL,
    participant_type TEXT NOT NULL CHECK(participant_type IN ('user', 'agent')),
    participant_name TEXT NOT NULL,
    participant_avatar TEXT,
    PRIMARY KEY (chat_page_id, participant_id, participant_type),
    FOREIGN KEY (chat_page_id) REFERENCES chat_pages(id) ON DELETE CASCADE
);
"#;
```

- [ ] **Step 3: Register V22 in `migration.rs`**

Add to `MIGRATIONS` array after V21, and update the "V1~V20" comment to "V1~V21".

- [ ] **Step 4: Verify**

Run: `cargo check`
Expected: `Finished` with no errors.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db/schema.rs src-tauri/src/db/migration.rs
git commit -m "feat(db): add chat_page_participants table and V22 migration"
```

---

## Task 2: Repository Layer

**Files:**
- Create: `src-tauri/src/db/chat_page_participant.rs`
- Modify: `src-tauri/src/db/mod.rs`

- [ ] **Step 1: Create repository file**

```rust
use rusqlite::{Connection, params};

#[derive(Debug, Clone)]
pub struct ChatPageParticipant {
    pub chat_page_id: String,
    pub participant_id: String,
    pub participant_type: String,
    pub participant_name: String,
    pub participant_avatar: Option<String>,
}

pub fn insert_snapshot(
    conn: &Connection,
    chat_page_id: &str,
    participant_id: &str,
    participant_type: &str,
    participant_name: &str,
    participant_avatar: Option<&str>,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT INTO chat_page_participants (chat_page_id, participant_id, participant_type, participant_name, participant_avatar)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![chat_page_id, participant_id, participant_type, participant_name, participant_avatar],
    )?;
    Ok(())
}

pub fn list_by_chat_page(
    conn: &Connection,
    chat_page_id: &str,
) -> Result<Vec<ChatPageParticipant>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT chat_page_id, participant_id, participant_type, participant_name, participant_avatar
         FROM chat_page_participants
         WHERE chat_page_id = ?1"
    )?;
    let rows = stmt.query_map([chat_page_id], |row| {
        Ok(ChatPageParticipant {
            chat_page_id: row.get(0)?,
            participant_id: row.get(1)?,
            participant_type: row.get(2)?,
            participant_name: row.get(3)?,
            participant_avatar: row.get(4)?,
        })
    })?;
    rows.collect()
}

pub fn get_chat_page_id(
    conn: &Connection,
    session_id: &str,
    page_index: i32,
) -> Result<Option<String>, rusqlite::Error> {
    conn.query_row(
        "SELECT id FROM chat_pages WHERE session_id = ?1 AND page_index = ?2",
        params![session_id, page_index],
        |row| row.get(0),
    ).optional()
}
```

- [ ] **Step 2: Register module in `db/mod.rs`**

Add `pub mod chat_page_participant;` after `pub mod chat_page;`.

- [ ] **Step 3: Verify**

Run: `cargo check`
Expected: `Finished` with no errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db/chat_page_participant.rs src-tauri/src/db/mod.rs
git commit -m "feat(db): add chat_page_participant repository"
```

---

## Task 3: Reset Session Snapshot Generation

**Files:**
- Modify: `src-tauri/src/db/session.rs`

**Prerequisite:** The `reset_session` function in `session.rs` already has a local variable `max_page` (declared via `conn.query_row` returning the current maximum `page_index`). The old page that is being archived has `page_index = max_page`. The new page being created has `page_index = max_page + 1`.

- [ ] **Step 1: Add `use` import**

At the top of `src-tauri/src/db/session.rs`, add:

```rust
use crate::db::chat_page_participant;
```

- [ ] **Step 2: Modify `reset_session` to insert snapshot**

Insert the following block **after** the new `chat_page` INSERT and **before** the session type UPDATE (i.e., after the `conn.execute` that inserts the new page, before `let session_type: String = ...`):

```rust
    // Insert participant snapshot for the old page (max_page)
    if let Ok(Some(old_page_id)) = chat_page_participant::get_chat_page_id(conn, session_id, max_page) {
        // Private session
        if let Ok((p1_type, p1_id, p2_type, p2_id)) = conn.query_row(
            "SELECT participant_1_type, participant_1_id, participant_2_type, participant_2_id FROM private_sessions WHERE session_id = ?1",
            [session_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?)),
        ) {
            for (ptype, pid) in [(p1_type, p1_id), (p2_type, p2_id)] {
                let (name, avatar): (String, Option<String>) = if ptype == "agent" {
                    conn.query_row(
                        "SELECT name, avatar_path FROM agents WHERE id = ?1",
                        [&pid],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    ).unwrap_or_else(|_| ("未知角色".to_string(), None))
                } else {
                    conn.query_row(
                        "SELECT COALESCE(up.name, '用户'), up.avatar_path FROM app_settings LEFT JOIN user_personas up ON up.id = app_settings.active_persona_id WHERE app_settings.id = 1",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    ).unwrap_or_else(|_| ("用户".to_string(), None))
                };
                let _ = chat_page_participant::insert_snapshot(conn, &old_page_id, &pid, &ptype, &name, avatar.as_deref());
            }
        }

        // Group session
        let mut stmt = conn.prepare(
            "SELECT participant_type, participant_id FROM group_members WHERE session_id = ?1"
        )?;
        let rows = stmt.query_map([session_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            if let Ok((ptype, pid)) = row {
                let (name, avatar): (String, Option<String>) = if ptype == "agent" {
                    conn.query_row(
                        "SELECT name, avatar_path FROM agents WHERE id = ?1",
                        [&pid],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    ).unwrap_or_else(|_| ("未知角色".to_string(), None))
                } else {
                    conn.query_row(
                        "SELECT COALESCE(up.name, '用户'), up.avatar_path FROM app_settings LEFT JOIN user_personas up ON up.id = app_settings.active_persona_id WHERE app_settings.id = 1",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    ).unwrap_or_else(|_| ("用户".to_string(), None))
                };
                let _ = chat_page_participant::insert_snapshot(conn, &old_page_id, &pid, &ptype, &name, avatar.as_deref());
            }
        }
    }
```

**Note:** The variable `max_page` is the existing local variable in `reset_session` that holds the current maximum `page_index`. The snapshot is inserted for this old page before the new page becomes active.

- [ ] **Step 3: Verify**

Run: `cargo check`
Expected: `Finished` with no errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db/session.rs
git commit -m "feat(session): generate participant snapshot on reset"
```

---

## Task 4: History Target Agents Query Snapshot

**Files:**
- Modify: `src-tauri/src/commands/message.rs`

- [ ] **Step 1: Modify `resolve_history_target_agents` to accept `page_index`**

Change signature from `resolve_history_target_agents(conn, session_id)` to `resolve_history_target_agents(conn, session_id, page_index)`.

Replace the body with:

```rust
fn resolve_history_target_agents(
    conn: &rusqlite::Connection,
    session_id: &str,
    page_index: i32,
) -> Result<Vec<String>, String> {
    // Try snapshot first
    if let Ok(Some(chat_page_id)) = crate::db::chat_page_participant::get_chat_page_id(conn, session_id, page_index) {
        let mut stmt = conn.prepare(
            "SELECT cpp.participant_id 
             FROM chat_page_participants cpp
             JOIN agents a ON cpp.participant_id = a.id
             WHERE cpp.chat_page_id = ?1 
               AND cpp.participant_type = 'agent'
               AND a.is_deleted = 0"
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map([&chat_page_id], |row| {
            row.get::<_, String>(0)
        }).map_err(|e| e.to_string())?;
        let agents: Vec<String> = rows.filter_map(|r| r.ok()).collect();
        if !agents.is_empty() {
            return Ok(agents);
        }
    }

    // Fallback: query current session members (backward compatibility for pre-V22 pages)
    let session_type: String = conn.query_row(
        "SELECT session_type FROM sessions WHERE id = ?1 AND is_deleted = 0",
        [session_id],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;

    let mut agents = Vec::new();
    if session_type == "private" {
        let agent_id: String = conn.query_row(
            "SELECT participant_2_id FROM private_sessions WHERE session_id = ?1 AND participant_2_type = 'agent'",
            [session_id],
            |row| row.get(0),
        ).map_err(|e| e.to_string())?;
        agents.push(agent_id);
    } else {
        let mut stmt = conn.prepare(
            "SELECT participant_id FROM group_members WHERE session_id = ?1 AND participant_type = 'agent'"
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map([session_id], |row| {
            row.get::<_, String>(0)
        }).map_err(|e| e.to_string())?;
        for row in rows {
            agents.push(row.map_err(|e| e.to_string())?);
        }
    }
    Ok(agents)
}
```

- [ ] **Step 2: Update call sites**

Find all calls to `resolve_history_target_agents` and add `req.page_index` as the third argument. The call site inside `send_history_message` is the production code path.

Also update the unit tests at the bottom of `message.rs` to pass `page_index: 0` as the third argument.

- [ ] **Step 3: Verify**

Run: `cargo check`
Expected: `Finished` with no errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/message.rs
git commit -m "feat(history): resolve target agents from snapshot with fallback"
```

---

## Task 5: HistoryPromptAssembler Use Snapshot

**Files:**
- Modify: `src-tauri/src/llm/history_prompt.rs`

**Background:** The original plan only replaced sender name resolution with snapshot data. The design doc's `get_participants_for_page` (full participant injection with relationship_text / memory_text) was inadvertently omitted. This task now includes both sender name resolution **and** a snapshot-based participant introduction layer.

- [ ] **Step 1: Add `use` import**

At the top of `src-tauri/src/llm/history_prompt.rs`, add:

```rust
use std::collections::HashMap;
```

- [ ] **Step 2: Extend `HistoryPromptAssembler::assemble` with snapshot participant introduction**

Replace the existing `assemble` method body with the following. The key changes are:
1. Load snapshot participants for the page
2. Build a participant introduction layer using snapshot names + relationship_text / memory_text from `agent_relationships`
3. Use snapshot for sender name resolution in the message loop

```rust
    pub fn assemble(
        conn: &Connection,
        agent_id: &str,
        session_id: &str,
        page_index: i32,
        history_messages: &[Message],
    ) -> Result<String, String> {
        // 1. 获取 Agent 自我设定
        let agent = crate::db::agent::get_by_id(conn, agent_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Agent not found: {}", agent_id))?;

        // 2. 加载快照参与者
        let snapshot_participants = if let Ok(Some(chat_page_id)) = 
            crate::db::chat_page_participant::get_chat_page_id(conn, session_id, page_index) {
            crate::db::chat_page_participant::list_by_chat_page(conn, &chat_page_id)
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let snapshot_map: HashMap<(String, String), crate::db::chat_page_participant::ChatPageParticipant> = 
            snapshot_participants.iter()
                .map(|p| ((p.participant_id.clone(), p.participant_type.clone()), p.clone()))
                .collect();

        // 3. 构建参与者介绍文本（基于快照）
        let mut participants_text = String::new();
        for p in &snapshot_participants {
            if p.participant_id == agent_id && p.participant_type == "agent" {
                continue; // 跳过当前 agent 自身
            }
            participants_text.push_str(&format!("- {}：{}", p.participant_name, 
                if p.participant_type == "agent" { "角色" } else { "用户" }));
            
            if p.participant_type == "agent" {
                // 查询 relationship_text 和 memory_text
                let (rel_text, mem_text): (String, String) = conn.query_row(
                    "SELECT COALESCE(relationship_text, ''), COALESCE(memory_text, '') 
                     FROM agent_relationships 
                     WHERE observer_id = ?1 AND target_id = ?2 AND target_type = 'agent'",
                    (agent_id, &p.participant_id),
                    |row| Ok((row.get(0)?, row.get(1)?)),
                ).unwrap_or_default();
                
                if !rel_text.is_empty() {
                    participants_text.push_str(&format!("\n  [印象]：{}", rel_text));
                }
                if agent.memory_enabled && !mem_text.is_empty() {
                    participants_text.push_str(&format!("\n  [记忆]：{}", mem_text));
                }
            }
            participants_text.push('\n');
        }

        // 4. 格式化当前 session + page 的消息历史（反转使旧消息在上，新消息在下）
        let mut context = String::new();
        for msg in history_messages.iter().rev() {
            let time = crate::llm::prompt::PromptAssembler::format_time(msg.created_at);
            let sender = if msg.sender_type == "agent" && msg.sender_id == agent_id {
                agent.name.clone()
            } else if let Some(snapshot) = snapshot_map.get(&(msg.sender_id.clone(), msg.sender_type.clone())) {
                snapshot.participant_name.clone()
            } else {
                crate::llm::prompt::PromptAssembler::get_sender_name(conn, &msg.sender_type, &msg.sender_id)?
            };
            context.push_str(&format!("[{}] {}: {}\n", time, sender, msg.content));
        }

        // 5. 获取会话类型
        let session_type = crate::db::session::get_session_by_id(conn, session_id)
            .map_err(|e| e.to_string())?
            .map(|s| s.session_type)
            .unwrap_or_else(|| "unknown".to_string());

        // 6. 组装完整 prompt
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let participants_section = if participants_text.is_empty() {
            String::new()
        } else {
            format!("【参与者】\n{}\n", participants_text)
        };

        let instruction = format!(
            "请基于以上对话上下文继续回复。\n\n【工具使用说明】\n你可以使用 send_message 工具向指定会话发送消息。\n当前你正在以下会话中聊天：\n- session_id: {}, 类型: {}\n\n请根据上下文决定是否需要回复。如果需要回复，请调用 send_message 工具，参数如下：\n- target_type: \"{}\"\n- target_id: \"{}\"\n- content: 你要发送的消息内容\n\n注意：target_id 必须是上面列出的 session_id，不能使用名称或其他 ID。",
            session_id, session_type, session_type, session_id
        );

        let full_prompt = format!(
            "{}\n\n{}\n\n{}{}\n\n{}",
            prompt_templates::SYSTEM_PROMPT.replace("{current_time}", &now),
            agent.detailed_persona,
            participants_section,
            context,
            instruction
        );

        // 7. 记录完整 prompt 到日志
        crate::logger::info(&format!(
            "[HistoryPromptAssembler] Full prompt for agent {} | session={} | page={} | prompt_length={}\n---PROMPT START---\n{}\n---PROMPT END---",
            agent_id, session_id, page_index, full_prompt.len(), full_prompt
        ));

        Ok(full_prompt)
    }
```

- [ ] **Step 3: Update tests**

The existing unit tests in `history_prompt.rs` use `page_index: 0` with in-memory databases that do not have `chat_page_participants` entries. Since the snapshot query will return no rows, the behavior falls back to the existing sender name resolution (unchanged for empty snapshots). Therefore the existing tests should continue to pass without modification.

However, add a new test that verifies snapshot-based participant introduction:

```rust
    #[test]
    fn test_history_prompt_uses_snapshot_participants() {
        let conn = init_test_db();
        insert_agent(&conn, "agent1", "Alice", "Persona 1");
        insert_agent(&conn, "agent2", "Bob", "Persona 2");
        insert_session(&conn, "sess1", "group");
        
        // Insert chat_page and snapshot
        conn.execute(
            "INSERT INTO chat_pages (id, session_id, page_index, name, is_active, message_count, created_at, updated_at) VALUES ('cp-0', 'sess1', 0, 'Page 0', 1, 0, 1000, 1000)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO chat_page_participants (chat_page_id, participant_id, participant_type, participant_name, participant_avatar) VALUES ('cp-0', 'agent2', 'agent', 'Snapshot Bob', NULL)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO chat_page_participants (chat_page_id, participant_id, participant_type, participant_name, participant_avatar) VALUES ('cp-0', 'user-1', 'user', 'Snapshot User', NULL)",
            [],
        ).unwrap();

        let msgs = vec![
            Message {
                id: "msg1".to_string(), session_id: "sess1".to_string(),
                sender_type: "user".to_string(), sender_id: "user-1".to_string(),
                content: "Hi".to_string(), created_at: 1000,
                message_type: "text".to_string(), tool_call_data: None,
                generation_info: None, is_deleted: false,
                sender_name: "用户".to_string(), sender_avatar: None, page_index: 0,
            },
            Message {
                id: "msg2".to_string(), session_id: "sess1".to_string(),
                sender_type: "agent".to_string(), sender_id: "agent2".to_string(),
                content: "Hello".to_string(), created_at: 2000,
                message_type: "text".to_string(), tool_call_data: None,
                generation_info: None, is_deleted: false,
                sender_name: "Bob".to_string(), sender_avatar: None, page_index: 0,
            },
        ];

        let prompt = HistoryPromptAssembler::assemble(&conn, "agent1", "sess1", 0, &msgs).unwrap();
        assert!(prompt.contains("Snapshot Bob"), "Prompt should use snapshot name for agent participant");
        assert!(prompt.contains("Snapshot User"), "Prompt should use snapshot name for user participant");
        assert!(prompt.contains("Snapshot User: Hi"), "Message sender name should come from snapshot");
        assert!(prompt.contains("Snapshot Bob: Hello"), "Message sender name should come from snapshot");
    }
```

- [ ] **Step 4: Verify**

Run: `cargo check`
Expected: `Finished` with no errors.

Run: `cargo test --lib llm::history_prompt -- --nocapture`
Expected: All existing + new tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/llm/history_prompt.rs
git commit -m "feat(history): HistoryPromptAssembler uses snapshot participants for names and introduction"
```

---

## Task 6: Backend Integration Tests

**Files:**
- Create: `src-tauri/tests/chat_page_snapshot_tests.rs`

- [ ] **Step 1: Create integration test file**

```rust
use agentstage_lib::db::connection::DbState;
use agentstage_lib::db::session as session_repo;
use agentstage_lib::db::chat_page_participant;
use agentstage_lib::commands::message::resolve_history_target_agents;
use rusqlite::Connection;
use std::sync::Arc;
use tokio::sync::Mutex;

fn init_test_db() -> DbState {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(agentstage_lib::db::schema::BASE_SCHEMA).unwrap();
    DbState(Arc::new(Mutex::new(conn)))
}

async fn setup_test_session(db: &DbState) -> String {
    let conn = db.0.lock().await;
    // Insert agent
    conn.execute(
        "INSERT INTO agents (id, name, detailed_persona, simplified_persona, model_config_id, created_at, updated_at)
         VALUES ('agent-1', 'Test Agent', 'detailed', 'simple', 'model-1', 1000, 1000)",
        [],
    ).unwrap();
    // Insert model config
    conn.execute(
        "INSERT INTO model_configs (id, name, provider, model_name, base_url, api_key_encrypted, created_at, updated_at)
         VALUES ('model-1', 'Test Model', 'openai', 'gpt-4', 'https://api.openai.com', 'key', 1000, 1000)",
        [],
    ).unwrap();
    // Insert session
    conn.execute(
        "INSERT INTO sessions (id, session_type, created_at, updated_at, last_message_at, is_deleted)
         VALUES ('session-1', 'private', 1000, 1000, 1000, 0)",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO private_sessions (session_id, participant_1_type, participant_1_id, participant_2_type, participant_2_id, agent_message_count, created_at)
         VALUES ('session-1', 'user', 'user-1', 'agent', 'agent-1', 0, 1000)",
        [],
    ).unwrap();
    // Insert chat_page
    conn.execute(
        "INSERT INTO chat_pages (id, session_id, page_index, name, is_active, message_count, created_at, updated_at)
         VALUES ('cp-0', 'session-1', 0, 'Page 0', 1, 0, 1000, 1000)",
        [],
    ).unwrap();
    "session-1".to_string()
}

#[tokio::test]
async fn test_reset_session_creates_snapshot() {
    let db = init_test_db();
    let session_id = setup_test_session(&db).await;

    let conn = db.0.lock().await;
    let (page_id, _new_index) = session_repo::reset_session(&conn, &session_id).unwrap();
    drop(conn);

    // Check snapshot was created for old page (cp-0)
    let conn = db.0.lock().await;
    let participants = chat_page_participant::list_by_chat_page(&conn, "cp-0").unwrap();
    assert_eq!(participants.len(), 2, "Snapshot should contain 2 participants");

    let agent = participants.iter().find(|p| p.participant_type == "agent").unwrap();
    assert_eq!(agent.participant_id, "agent-1");
    assert_eq!(agent.participant_name, "Test Agent");

    let user = participants.iter().find(|p| p.participant_type == "user").unwrap();
    assert_eq!(user.participant_id, "user-1");
}

#[tokio::test]
async fn test_deleted_agent_not_in_history_targets() {
    let db = init_test_db();
    let session_id = setup_test_session(&db).await;

    let conn = db.0.lock().await;
    session_repo::reset_session(&conn, &session_id).unwrap();
    // Snapshot for cp-0 should now contain agent-1
    let participants = chat_page_participant::list_by_chat_page(&conn, "cp-0").unwrap();
    assert_eq!(participants.len(), 2);
    
    // Soft delete agent
    conn.execute("UPDATE agents SET is_deleted = 1 WHERE id = 'agent-1'", []).unwrap();
    drop(conn);

    // resolve_history_target_agents should return empty because agent is deleted
    let conn = db.0.lock().await;
    let agents = resolve_history_target_agents(&conn, &session_id, 0).unwrap();
    assert!(agents.is_empty(), "Deleted agent should be excluded from history targets even when in snapshot");
}

#[tokio::test]
async fn test_resolve_history_fallback_for_pre_migration_page() {
    let db = init_test_db();
    let session_id = setup_test_session(&db).await;
    
    // Do NOT call reset_session, so cp-0 has no snapshot
    let conn = db.0.lock().await;
    let agents = resolve_history_target_agents(&conn, &session_id, 0).unwrap();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0], "agent-1");
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --test chat_page_snapshot_tests -- --nocapture`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tests/chat_page_snapshot_tests.rs
git commit -m "test(snapshot): add integration tests for chat page participant snapshot"
```

---

## Task 7: Frontend Snapshot Store and Tauri Commands

**Files:**
- Create: `src-tauri/src/commands/chat_page.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/stores/sessionStore.svelte.ts`

- [ ] **Step 1: Define typed response struct in Rust**

In `src-tauri/src/commands/chat_page.rs`:

```rust
use serde::Serialize;
use tauri::State;
use crate::db::connection::DbState;
use crate::db::chat_page_participant;

#[derive(Serialize)]
pub struct ChatPageParticipantResponse {
    pub participant_id: String,
    pub participant_type: String,
    pub participant_name: String,
    pub participant_avatar: Option<String>,
}

#[tauri::command]
pub async fn get_chat_page_id(
    state: State<'_, DbState>,
    session_id: String,
    page_index: i32,
) -> Result<Option<String>, String> {
    let conn = crate::db::connection::get_db(&state).await?;
    chat_page_participant::get_chat_page_id(&conn, &session_id, page_index)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_chat_page_participants(
    state: State<'_, DbState>,
    chat_page_id: String,
) -> Result<Vec<ChatPageParticipantResponse>, String> {
    let conn = crate::db::connection::get_db(&state).await?;
    let participants = chat_page_participant::list_by_chat_page(&conn, &chat_page_id)
        .map_err(|e| e.to_string())?;
    Ok(participants.into_iter().map(|p| ChatPageParticipantResponse {
        participant_id: p.participant_id,
        participant_type: p.participant_type,
        participant_name: p.participant_name,
        participant_avatar: p.participant_avatar,
    }).collect())
}
```

- [ ] **Step 2: Register commands in `lib.rs`**

In `src-tauri/src/lib.rs`, inside the `tauri::generate_handler!` macro, add:

```rust
            commands::chat_page::get_chat_page_id,
            commands::chat_page::list_chat_page_participants,
```

Also ensure the module is declared. If `commands/chat_page.rs` is a new file, you need to add `pub mod chat_page;` in `src-tauri/src/commands/mod.rs` (create `mod.rs` if it doesn't exist, or add to the existing module declarations).

The final `generate_handler!` block should include the two new commands alongside existing ones like `create_agent`, `list_sessions`, etc.

- [ ] **Step 3: Add frontend store methods**

In `src/lib/stores/sessionStore.svelte.ts`, add to the `SessionStore` class:

```typescript
    pageParticipants = $state<Map<string, Array<{participantId: string, participantType: string, participantName: string, participantAvatar: string | null}>>>(new Map());

    async loadPageParticipants(sessionId: string, pageIndex: number) {
        try {
            const chatPageId = await invoke<string | null>('get_chat_page_id', { sessionId, pageIndex });
            if (!chatPageId) return;
            const participants = await invoke<Array<{participantId: string, participantType: string, participantName: string, participantAvatar: string | null}>>('list_chat_page_participants', { chatPageId });
            this.pageParticipants.set(chatPageId, participants);
        } catch (e) {
            console.error('Failed to load page participants:', e);
        }
    }

    getParticipantName(chatPageId: string | undefined, participantId: string, participantType: string): string {
        if (!chatPageId) return '未知角色';
        const list = this.pageParticipants.get(chatPageId);
        if (!list) return '未知角色';
        const found = list.find(p => p.participantId === participantId && p.participantType === participantType);
        return found?.participantName ?? '未知角色';
    }

    getParticipantAvatar(chatPageId: string | undefined, participantId: string, participantType: string): string | null {
        if (!chatPageId) return null;
        const list = this.pageParticipants.get(chatPageId);
        if (!list) return null;
        const found = list.find(p => p.participantId === participantId && p.participantType === participantType);
        return found?.participantAvatar ?? null;
    }
```

- [ ] **Step 4: Verify**

Run: `cargo check`
Expected: `Finished` with no errors.

Run: `npx svelte-check --tsconfig ./tsconfig.json`
Expected: 0 errors.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/chat_page.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs src/lib/stores/sessionStore.svelte.ts
git commit -m "feat(frontend): add chat page participant store and commands"
```

---

## Task 8: Frontend Message Rendering

**Files:**
- Modify: `src/lib/components/MessageBubble.svelte`
- Modify: `src/lib/components/ChatView.svelte`

- [ ] **Step 1: Modify MessageBubble to accept snapshot props**

In `src/lib/components/MessageBubble.svelte`, update the Props interface and derived display values:

```svelte
<script lang="ts">
    import type { Message } from '$lib/types';
    import { formatTime, resolveAvatarUrl } from '$lib/utils';
    import { User, Bot } from 'lucide-svelte';

    interface Props {
        message: Message;
        isMe: boolean;
        senderName: string;
        snapshotName?: string;
        snapshotAvatar?: string | null;
    }

    let { message, isMe, senderName, snapshotName, snapshotAvatar }: Props = $props();

    const displayName = snapshotName ?? senderName ?? '未知角色';
    const displayAvatar = snapshotAvatar ?? message.sender_avatar;
</script>
```

Update the avatar rendering to use `displayAvatar`:

```svelte
        <div class="w-8 h-8 rounded-full flex items-center justify-center shrink-0 overflow-hidden {message.sender_type === 'user' ? 'bg-gray-300 text-white' : 'bg-primary/10 text-primary'}">
            {#if displayAvatar}
                <img src={resolveAvatarUrl(displayAvatar)} alt={displayName} class="w-full h-full object-cover" />
            {:else if message.sender_type === 'user'}
                <User size={16} />
            {:else}
                <Bot size={16} />
            {/if}
        </div>
```

And update the sender name display:

```svelte
            <span class="text-xs text-text-secondary leading-none">{displayName}</span>
```

- [ ] **Step 2: Modify ChatView history mode to load snapshots and pass to MessageBubble**

In `src/lib/components/ChatView.svelte`, locate the message rendering loop (inside the `#each messageStore.messages` block) and modify it for history mode.

**Where to get `currentChatPageId`:** In history mode, `historyStore.selectedPageIndex` holds the current page index. Call `sessionStore.loadPageParticipants(historyStore.selectedSessionId, historyStore.selectedPageIndex)` when entering history mode or when `selectedPageIndex` changes. The `currentChatPageId` can be obtained either:
- By querying the store: after `loadPageParticipants` resolves, the Map key is the chat_page_id; or
- By calling `invoke('get_chat_page_id', { sessionId: historyStore.selectedSessionId, pageIndex: historyStore.selectedPageIndex })` directly.

For simplicity, use the store approach. After `loadPageParticipants`, find the chat_page_id by looking for the entry whose page index matches. However, the store Map keys are chat_page_ids, not page indexes. A simpler approach: store the current `chatPageId` as a separate reactive state in ChatView when the page changes:

```typescript
    let currentChatPageId = $state<string | undefined>(undefined);

    // In the effect that runs when history page changes:
    $effect(() => {
        if (mode === 'history' && historyStore.selectedSessionId && historyStore.selectedPageIndex !== null) {
            sessionStore.loadPageParticipants(historyStore.selectedSessionId, historyStore.selectedPageIndex);
            // We also need the chat_page_id; load it separately
            invoke<string | null>('get_chat_page_id', { 
                sessionId: historyStore.selectedSessionId, 
                pageIndex: historyStore.selectedPageIndex 
            }).then(id => { currentChatPageId = id ?? undefined; });
        }
    });
```

Then in the message rendering loop:

```svelte
                    {#each messageStore.messages as message (message.id)}
                        {@const rightSide = isOnRightSide(message, selectedSession)}
                        {@const snapName = mode === 'history' 
                            ? sessionStore.getParticipantName(currentChatPageId, message.sender_id, message.sender_type)
                            : undefined}
                        {@const snapAvatar = mode === 'history'
                            ? sessionStore.getParticipantAvatar(currentChatPageId, message.sender_id, message.sender_type)
                            : undefined}
                        <div
                            class="flex px-4 {rightSide ? 'justify-end' : 'justify-start'}"
                        >
                            <MessageBubble
                                {message}
                                isMe={rightSide}
                                senderName={message.sender_name || '未知'}
                                snapshotName={snapName}
                                snapshotAvatar={snapAvatar}
                            />
                        </div>
                    {/each}
```

**Note:** The fallback behavior is: if `currentChatPageId` is undefined (e.g., pre-migration page or no snapshot), `getParticipantName` returns `'未知角色'`. However, for backward compatibility, we want to fall back to `message.sender_name` when no snapshot exists. In the `MessageBubble` component, `snapshotName` will be `'未知角色'` when no snapshot is found, which would override the `senderName` prop. This is incorrect for pre-migration pages.

**Fix:** Modify `getParticipantName` to return `undefined` instead of `'未知角色'` when no snapshot is found, and let `MessageBubble` handle the fallback:

```typescript
    getParticipantName(chatPageId: string | undefined, participantId: string, participantType: string): string | undefined {
        if (!chatPageId) return undefined;
        const list = this.pageParticipants.get(chatPageId);
        if (!list) return undefined;
        const found = list.find(p => p.participantId === participantId && p.participantType === participantType);
        return found?.participantName;
    }
```

And update `MessageBubble`:

```typescript
    const displayName = snapshotName ?? senderName ?? '未知角色';
```

This way:
- If snapshot exists → use snapshot name
- If no snapshot but sender_name exists → use sender_name (backward compatible)
- If neither → "未知角色"

- [ ] **Step 3: Verify**

Run: `npx svelte-check --tsconfig ./tsconfig.json`
Expected: 0 errors.

- [ ] **Step 4: Commit**

```bash
git add src/lib/components/MessageBubble.svelte src/lib/components/ChatView.svelte src/lib/stores/sessionStore.svelte.ts
git commit -m "feat(frontend): history messages use snapshot names and avatars"
```

---

## Task 9: Final Verification

- [ ] **Step 1: Rust compilation**

Run: `cargo check`
Expected: `Finished` with no errors.

- [ ] **Step 2: Svelte type check**

Run: `npx svelte-check --tsconfig ./tsconfig.json`
Expected: 0 errors.

- [ ] **Step 3: Rust tests**

Run: `cargo test --test chat_page_snapshot_tests -- --nocapture`
Expected: All tests pass.

Run: `cargo test --lib llm::history_prompt -- --nocapture`
Expected: All tests pass.

- [ ] **Step 4: Commit final verification**

```bash
git commit --allow-empty -m "chore: final verification complete"
```

---

## Self-Review

### Spec Coverage Check

| Spec Requirement | Plan Task | Notes |
|-----------------|-----------|-------|
| Schema + Migration V22 | Task 1 | |
| Repository layer | Task 2 | Added typed `ChatPageParticipantResponse` for Tauri command |
| `reset_session` snapshot | Task 3 | Uses existing `max_page` variable; clarified variable source |
| `resolve_history_target_agents` snapshot query | Task 4 | |
| `HistoryPromptAssembler` snapshot (full participant injection) | Task 5 | **Fixed:** Now includes both sender name resolution AND participant introduction layer with relationship_text / memory_text |
| Backend tests | Task 6 | **Fixed:** Added actual assertions to `test_deleted_agent_not_in_history_targets`; added fallback test |
| Frontend store + Tauri commands | Task 7 | **Fixed:** Added `ChatPageParticipantResponse` struct; added `generate_handler!` registration details |
| Frontend rendering | Task 8 | **Fixed:** Complete code for `snapshotAvatar` and `currentChatPageId`; backward-compatible fallback |
| Final verification | Task 9 | |

### Out-of-Scope Items (Explicitly Documented)

| Item | Reason |
|------|--------|
| `HistorySessionList` snapshot display | Design doc lists as "optional enhancement"; current live-member display is acceptable |
| Data migration for existing `page_index=0` pages | Backward compatibility handles these via fallback to current session members |

### Placeholder Scan

- No TBD/TODO found.
- All code steps contain complete code.
- No "Similar to Task N" patterns.

### Type Consistency

- `ChatPageParticipant` struct fields match between Rust repository, Tauri command response, and TypeScript store.
- Tauri Command parameter names use camelCase.
- `resolve_history_target_agents` signature updated consistently.
- `MessageBubble` `snapshotName` returns `undefined` when not found, allowing graceful fallback to `sender_name`.

### Import Statements

- Task 3: Added `use crate::db::chat_page_participant;`
- Task 5: Added `use std::collections::HashMap;`
- Task 7: `commands/chat_page.rs` is a new file with complete imports.
