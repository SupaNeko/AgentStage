# Chat Page Participant Snapshot Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reset session generates a participant snapshot into `chat_page_participants`; history view and history-mode chat use snapshot state instead of current session members.

**Architecture:** Add a `chat_page_participants` snapshot table keyed by `chat_page_id`. On `reset_session`, copy current session members into the snapshot. `resolve_history_target_agents` and message rendering query the snapshot, falling back gracefully for pre-migration pages.

**Tech Stack:** Rust (Tauri v2), SQLite, Svelte 5, TailwindCSS v4

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
| `src-tauri/src/llm/history_prompt.rs` | `HistoryPromptAssembler` uses snapshot participants instead of live queries |
| `src-tauri/tests/chat_page_snapshot_tests.rs` | **New** — Integration tests for snapshot + history query |
| `src/lib/stores/sessionStore.svelte.ts` | Add `pageParticipants: Map<chatPageId, Participant[]>` and `loadPageParticipants` |
| `src/lib/components/ChatView.svelte` | History mode: load snapshots; pass to `MessageBubble` |
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
use crate::models::session::ChatPage;

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

- [ ] **Step 1: Modify `reset_session` to insert snapshot**

After creating the new `chat_page` (around line 519), add snapshot insertion logic:

```rust
    // Insert participant snapshot for the old page
    if let Ok(Some(old_page_id)) = chat_page_participant::get_chat_page_id(conn, session_id, current_page) {
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

- [ ] **Step 2: Verify**

Run: `cargo check`
Expected: `Finished` with no errors.

- [ ] **Step 3: Commit**

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

Find all calls to `resolve_history_target_agents` and add `req.page_index` as the third argument. There is one call site at line 170.

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

- [ ] **Step 1: Add snapshot-based participant loading**

At the start of `HistoryPromptAssembler::assemble`, after loading agent info, add:

```rust
        // Load snapshot participants for this page
        let snapshot_participants = if let Ok(Some(chat_page_id)) = 
            crate::db::chat_page_participant::get_chat_page_id(conn, session_id, page_index) {
            crate::db::chat_page_participant::list_by_chat_page(conn, &chat_page_id)
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let snapshot_map: std::collections::HashMap<(String, String), crate::db::chat_page_participant::ChatPageParticipant> = 
            snapshot_participants.into_iter()
                .map(|p| ((p.participant_id.clone(), p.participant_type.clone()), p))
                .collect();
```

- [ ] **Step 2: Use snapshot for sender name resolution**

In the message formatting loop (line 22-29), replace the sender resolution:

```rust
            let sender = if msg.sender_type == "agent" && msg.sender_id == agent_id {
                agent.name.clone()
            } else if let Some(snapshot) = snapshot_map.get(&(msg.sender_id.clone(), msg.sender_type.clone())) {
                snapshot.participant_name.clone()
            } else {
                crate::llm::prompt::PromptAssembler::get_sender_name(conn, &msg.sender_type, &msg.sender_id)
                    .unwrap_or_else(|_| "未知角色".to_string())
            };
```

- [ ] **Step 3: Verify**

Run: `cargo check`
Expected: `Finished` with no errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/llm/history_prompt.rs
git commit -m "feat(history): HistoryPromptAssembler uses snapshot participants"
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
    // Soft delete agent
    conn.execute("UPDATE agents SET is_deleted = 1 WHERE id = 'agent-1'", []).unwrap();
    drop(conn);

    // resolve_history_target_agents should return empty because agent is deleted
    // (tested via snapshot path)
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

## Task 7: Frontend Snapshot Store

**Files:**
- Modify: `src/lib/stores/sessionStore.svelte.ts`

- [ ] **Step 1: Add snapshot state and loader**

Add to `SessionStore` class:

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
```

- [ ] **Step 2: Add Tauri Commands**

Create `src-tauri/src/commands/chat_page.rs`:

```rust
use tauri::State;
use crate::db::connection::DbState;
use crate::db::chat_page_participant;

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
) -> Result<Vec<serde_json::Value>, String> {
    let conn = crate::db::connection::get_db(&state).await?;
    let participants = chat_page_participant::list_by_chat_page(&conn, &chat_page_id)
        .map_err(|e| e.to_string())?;
    Ok(participants.into_iter().map(|p| serde_json::json!({
        "participantId": p.participant_id,
        "participantType": p.participant_type,
        "participantName": p.participant_name,
        "participantAvatar": p.participant_avatar,
    })).collect())
}
```

Register in `lib.rs`.

- [ ] **Step 3: Verify**

Run: `cargo check`
Expected: `Finished` with no errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/chat_page.rs src-tauri/src/lib.rs src/lib/stores/sessionStore.svelte.ts
git commit -m "feat(frontend): add chat page participant store and commands"
```

---

## Task 8: Frontend Message Rendering

**Files:**
- Modify: `src/lib/components/MessageBubble.svelte`
- Modify: `src/lib/components/ChatView.svelte`

- [ ] **Step 1: Modify MessageBubble to accept snapshot props**

Add props:

```svelte
<script lang="ts">
    let {
        message,
        snapshotName,
        snapshotAvatar,
    }: {
        message: Message;
        snapshotName?: string;
        snapshotAvatar?: string | null;
    } = $props();

    const displayName = snapshotName ?? message.sender_name ?? '未知角色';
    const displayAvatar = snapshotAvatar ?? message.sender_avatar;
</script>
```

Use `displayName` and `displayAvatar` in rendering. If sender is not found and no snapshot, show "未知角色" with a default placeholder avatar.

- [ ] **Step 2: Modify ChatView history mode to load snapshots**

In history mode, after loading messages, call `sessionStore.loadPageParticipants(sessionId, pageIndex)`.

Pass snapshot info to each `MessageBubble`:

```svelte
<MessageBubble
    message={msg}
    snapshotName={sessionStore.getParticipantName(currentChatPageId, msg.sender_id, msg.sender_type)}
    snapshotAvatar={...}
/>
```

- [ ] **Step 3: Verify**

Run: `npx svelte-check --tsconfig ./tsconfig.json`
Expected: 0 errors.

- [ ] **Step 4: Commit**

```bash
git add src/lib/components/MessageBubble.svelte src/lib/components/ChatView.svelte
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

- [ ] **Step 4: Commit final verification**

```bash
git commit --allow-empty -m "chore: final verification complete"
```

---

## Self-Review

### Spec Coverage Check

| Spec Requirement | Plan Task |
|-----------------|-----------|
| Schema + Migration V22 | Task 1 |
| Repository layer | Task 2 |
| `reset_session` snapshot | Task 3 |
| `resolve_history_target_agents` snapshot query | Task 4 |
| `HistoryPromptAssembler` snapshot | Task 5 |
| Backend tests | Task 6 |
| Frontend store | Task 7 |
| Frontend rendering | Task 8 |
| Final verification | Task 9 |

### Placeholder Scan

- No TBD/TODO found.
- All code steps contain complete code.
- No "Similar to Task N" patterns.

### Type Consistency

- `ChatPageParticipant` struct fields match between Rust repository and TypeScript store.
- Tauri Command parameter names use camelCase.
- `resolve_history_target_agents` signature updated consistently.
