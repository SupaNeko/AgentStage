# Session Config Panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a slide-in configuration panel for private and group chats, supporting history limit, message limit, mute, member management, reset session, and disband group.

**Architecture:** Use a unified `session_settings` table for all session-level configuration. Keep `agent_message_count` and `current_chat_page` in their original tables. Add `page_index` to `messages` for reset session support via `chat_pages`. Frontend uses a `SessionSettingsPanel` drawer component overlaying the member list.

**Tech Stack:** Tauri v2 + Rust + SQLite (rusqlite), Svelte 5 + TailwindCSS v4

---

## File Structure

### New Files
- `src/lib/components/ConfirmDialog.svelte` — Reusable confirm dialog with title/content/primary action
- `src/lib/components/AddMemberModal.svelte` — Modal to add agents to a group
- `src/lib/components/SessionSettingsPanel.svelte` — Right-slide configuration drawer

### Modified Files
- `src-tauri/src/db/schema.rs` — Add MIGRATION_V4
- `src-tauri/src/db/migration.rs` — Register V4 migration
- `src-tauri/src/db/session.rs` — Add session_settings CRUD, reset_session, disband, member mgmt
- `src-tauri/src/db/message.rs` — Add page_index filtering to queries
- `src-tauri/src/models/session.rs` — Add SessionConfig, UpdateSessionConfigRequest
- `src-tauri/src/commands/session.rs` — Add 6 new Tauri commands
- `src-tauri/src/lib.rs` — Register new commands
- `src-tauri/src/llm/prompt.rs` — Read history_limit from session_settings, filter by page_index
- `src-tauri/src/scheduler/mod.rs` — Read mute/message_limit from session_settings
- `src/lib/types.ts` — Add TypeScript types for config and requests
- `src/lib/stores/sessionStore.svelte.ts` — Add reset/disband helpers
- `src/lib/components/ChatView.svelte` — Add settings button and integrate panel
- `docs/feature_list.md` — Update status after completion

---

## Task 1: V4 Database Migration

**Files:**
- Modify: `src-tauri/src/db/schema.rs`
- Modify: `src-tauri/src/db/migration.rs`

- [ ] **Step 1: Add MIGRATION_V4 to schema.rs**

Append after MIGRATION_V3:

```rust
pub const MIGRATION_V4: &str = r#"
-- V4: Session configuration panel
-- 1. Create unified session_settings table
CREATE TABLE IF NOT EXISTS session_settings (
    session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
    history_limit INTEGER,
    message_limit INTEGER,
    message_limit_enabled INTEGER DEFAULT 1 CHECK(message_limit_enabled IN (0, 1)),
    mute_enabled INTEGER DEFAULT 0 CHECK(mute_enabled IN (0, 1)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- 2. Migrate existing config from private_sessions and group_sessions
INSERT OR IGNORE INTO session_settings (session_id, history_limit, message_limit, message_limit_enabled, mute_enabled, created_at, updated_at)
SELECT 
    ps.session_id,
    NULL as history_limit,
    ps.message_limit,
    ps.message_limit_enabled,
    0 as mute_enabled,
    ps.created_at,
    ps.created_at
FROM private_sessions ps
LEFT JOIN session_settings ss ON ps.session_id = ss.session_id
WHERE ss.session_id IS NULL;

INSERT OR IGNORE INTO session_settings (session_id, history_limit, message_limit, message_limit_enabled, mute_enabled, created_at, updated_at)
SELECT 
    gs.session_id,
    NULL as history_limit,
    gs.message_limit,
    gs.message_limit_enabled,
    gs.mute_enabled,
    gs.created_at,
    gs.created_at
FROM group_sessions gs
LEFT JOIN session_settings ss ON gs.session_id = ss.session_id
WHERE ss.session_id IS NULL;

-- 3. Add page_index to messages for chat page support
ALTER TABLE messages ADD COLUMN page_index INTEGER DEFAULT 0;

-- 4. Initialize default chat_pages for existing sessions
INSERT OR IGNORE INTO chat_pages (id, session_id, page_index, name, is_active, message_count, created_at, updated_at)
SELECT 
    lower(hex(randomblob(16))),
    s.id,
    0,
    '默认',
    1,
    0,
    s.created_at,
    s.created_at
FROM sessions s
LEFT JOIN chat_pages cp ON s.id = cp.session_id AND cp.page_index = 0
WHERE cp.id IS NULL;
"#;
```

- [ ] **Step 2: Register migration in migration.rs**

Add to MIGRATIONS slice after V3:

```rust
    Migration {
        version: 4,
        name: "session_config_panel",
        sql: super::schema::MIGRATION_V4,
    },
```

Update test helper `init_test_db()` in `db/session.rs` to also execute MIGRATION_V4.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/db/schema.rs src-tauri/src/db/migration.rs
git commit -m "db: add V4 migration for session_settings, page_index, chat_pages"
```

---

## Task 2: Backend Models

**Files:**
- Modify: `src-tauri/src/models/session.rs`

- [ ] **Step 1: Add SessionConfig and UpdateSessionConfigRequest**

Append to `src-tauri/src/models/session.rs`:

```rust
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

#[derive(Debug, Clone, Deserialize)]
pub struct ResetSessionRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AddGroupMemberRequest {
    pub session_id: String,
    pub agent_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RemoveGroupMemberRequest {
    pub session_id: String,
    pub agent_id: String,
}
```

- [ ] **Step 2: Commit**

```bash
git add src-tauri/src/models/session.rs
git commit -m "models: add SessionConfig and config request structs"
```

---

## Task 3: Backend Repository — Session Settings & Session Management

**Files:**
- Modify: `src-tauri/src/db/session.rs`

- [ ] **Step 1: Add session_settings helpers to db/session.rs**

Add these functions after `get_group_members`:

```rust
pub fn get_session_config(conn: &Connection, session_id: &str, session_type: &str) -> Result<crate::models::session::SessionConfig> {
    let defaults = if session_type == "private" {
        (30, 10)
    } else {
        (80, 30)
    };
    
    conn.query_row(
        "SELECT session_id, COALESCE(history_limit, ?3), COALESCE(message_limit, ?4), 
                message_limit_enabled, mute_enabled 
         FROM session_settings WHERE session_id = ?1",
        rusqlite::params![session_id, defaults.0, defaults.1],
        |row| {
            Ok(crate::models::session::SessionConfig {
                session_id: row.get(0)?,
                history_limit: row.get(1)?,
                message_limit: row.get(2)?,
                message_limit_enabled: row.get::<_, i32>(3)? != 0,
                mute_enabled: row.get::<_, i32>(4)? != 0,
            })
        },
    )
}

pub fn init_session_settings(conn: &Connection, session_id: &str, session_type: &str) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let (history_limit, message_limit) = if session_type == "private" {
        (30, 10)
    } else {
        (80, 30)
    };
    conn.execute(
        "INSERT OR IGNORE INTO session_settings (session_id, history_limit, message_limit, message_limit_enabled, mute_enabled, created_at, updated_at) 
         VALUES (?1, ?2, ?3, 1, 0, ?4, ?4)",
        rusqlite::params![session_id, history_limit, message_limit, now],
    )?;
    Ok(())
}

pub fn update_session_config(conn: &Connection, req: &crate::models::session::UpdateSessionConfigRequest) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    
    // Build dynamic UPDATE
    let mut sets = Vec::new();
    let mut params: Vec<&dyn rusqlite::ToSql> = Vec::new();
    
    if let Some(v) = req.history_limit {
        sets.push("history_limit = ?");
        params.push(&v as &dyn rusqlite::ToSql);
    }
    if let Some(v) = req.message_limit {
        sets.push("message_limit = ?");
        params.push(&v as &dyn rusqlite::ToSql);
    }
    if let Some(v) = req.message_limit_enabled {
        sets.push("message_limit_enabled = ?");
        params.push(&(v as i32) as &dyn rusqlite::ToSql);
    }
    if let Some(v) = req.mute_enabled {
        sets.push("mute_enabled = ?");
        params.push(&(v as i32) as &dyn rusqlite::ToSql);
    }
    
    if sets.is_empty() {
        return Ok(());
    }
    
    sets.push("updated_at = ?");
    params.push(&now as &dyn rusqlite::ToSql);
    params.push(&req.session_id as &dyn rusqlite::ToSql);
    
    let sql = format!("UPDATE session_settings SET {} WHERE session_id = ?", sets.join(", "));
    conn.execute(&sql, rusqlite::params_from_iter(params))?;
    Ok(())
}

pub fn reset_session(conn: &Connection, session_id: &str) -> Result<String> {
    let now = chrono::Utc::now().timestamp_millis();
    let tx = conn.unchecked_transaction()?;
    
    // Get current max page_index for this session
    let max_page: i32 = conn.query_row(
        "SELECT COALESCE(MAX(page_index), 0) FROM chat_pages WHERE session_id = ?1",
        [session_id],
        |row| row.get(0),
    ).unwrap_or(0);
    
    let new_page_index = max_page + 1;
    let page_id = uuid::Uuid::new_v4().to_string();
    
    conn.execute(
        "INSERT INTO chat_pages (id, session_id, page_index, name, is_active, message_count, created_at, updated_at) 
         VALUES (?1, ?2, ?3, '续开', 1, 0, ?4, ?4)",
        rusqlite::params![&page_id, session_id, new_page_index, now],
    )?;
    
    // Update current_chat_page
    let session_type: String = conn.query_row(
        "SELECT session_type FROM sessions WHERE id = ?1",
        [session_id],
        |row| row.get(0),
    )?;
    
    if session_type == "private" {
        conn.execute(
            "UPDATE private_sessions SET current_chat_page = ?1, agent_message_count = 0 WHERE session_id = ?2",
            rusqlite::params![new_page_index, session_id],
        )?;
    } else {
        conn.execute(
            "UPDATE group_sessions SET current_chat_page = ?1, agent_message_count = 0 WHERE session_id = ?2",
            rusqlite::params![new_page_index, session_id],
        )?;
    }
    
    tx.commit()?;
    Ok(page_id)
}

pub fn disband_group(conn: &Connection, session_id: &str) -> Result<bool> {
    let now = chrono::Utc::now().timestamp_millis();
    let rows = conn.execute(
        "UPDATE sessions SET is_deleted = 1, deleted_at = ?2 WHERE id = ?1 AND session_type = 'group'",
        (session_id, now),
    )?;
    Ok(rows > 0)
}

pub fn add_group_member(conn: &Connection, session_id: &str, agent_id: &str) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let tx = conn.unchecked_transaction()?;
    
    conn.execute(
        "INSERT OR IGNORE INTO group_members (session_id, participant_type, participant_id, joined_at) VALUES (?1, 'agent', ?2, ?3)",
        (session_id, agent_id, now),
    )?;
    
    // Ensure friendships exist with other group agents
    let other_agents: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT participant_id FROM group_members 
             WHERE session_id = ?1 AND participant_type = 'agent' AND participant_id != ?2"
        )?;
        stmt.query_map([session_id, agent_id], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect()
    };
    
    for other_id in other_agents {
        conn.execute(
            "INSERT OR IGNORE INTO friendships (id, agent_id_1, agent_id_2, participant_type_2, created_at, source_session_id) 
             VALUES (?1, ?2, ?3, 'agent', ?4, ?5)",
            rusqlite::params![uuid::Uuid::new_v4().to_string(), agent_id, &other_id, now, session_id],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO friendships (id, agent_id_1, agent_id_2, participant_type_2, created_at, source_session_id) 
             VALUES (?1, ?2, ?3, 'agent', ?4, ?5)",
            rusqlite::params![uuid::Uuid::new_v4().to_string(), &other_id, agent_id, now, session_id],
        )?;
    }
    
    tx.commit()?;
    Ok(())
}

pub fn remove_group_member(conn: &Connection, session_id: &str, agent_id: &str) -> Result<bool> {
    let rows = conn.execute(
        "DELETE FROM group_members WHERE session_id = ?1 AND participant_type = 'agent' AND participant_id = ?2",
        (session_id, agent_id),
    )?;
    Ok(rows > 0)
}
```

- [ ] **Step 2: Update create_private_session to init session_settings**

In `create_private_session`, after inserting into `private_sessions`, add:

```rust
    init_session_settings(&conn, &session_id, "private")?;
```

- [ ] **Step 3: Update create_group_session to init session_settings**

In `create_group_session`, after inserting into `group_sessions`, add:

```rust
    init_session_settings(&conn, &session_id, "group")?;
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db/session.rs
git commit -m "db: add session_settings CRUD, reset_session, disband, member management"
```

---

## Task 4: Backend Commands

**Files:**
- Modify: `src-tauri/src/commands/session.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add new commands to commands/session.rs**

Append after `get_group_members`:

```rust
#[tauri::command]
pub async fn get_session_config(
    state: State<'_, DbState>,
    session_id: String,
    session_type: String,
) -> Result<crate::models::session::SessionConfig, String> {
    let conn = get_db(&state).await?;
    session_repo::get_session_config(&conn, &session_id, &session_type)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_session_config(
    state: State<'_, DbState>,
    req: crate::models::session::UpdateSessionConfigRequest,
) -> Result<(), String> {
    let conn = get_db(&state).await?;
    session_repo::update_session_config(&conn, &req)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reset_session(
    state: State<'_, DbState>,
    req: crate::models::session::ResetSessionRequest,
) -> Result<String, String> {
    let conn = get_db(&state).await?;
    session_repo::reset_session(&conn, &req.session_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn disband_group(
    state: State<'_, DbState>,
    session_id: String,
) -> Result<bool, String> {
    let conn = get_db(&state).await?;
    session_repo::disband_group(&conn, &session_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_group_member(
    state: State<'_, DbState>,
    req: crate::models::session::AddGroupMemberRequest,
) -> Result<(), String> {
    let conn = get_db(&state).await?;
    session_repo::add_group_member(&conn, &req.session_id, &req.agent_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_group_member(
    state: State<'_, DbState>,
    req: crate::models::session::RemoveGroupMemberRequest,
) -> Result<bool, String> {
    let conn = get_db(&state).await?;
    session_repo::remove_group_member(&conn, &req.session_id, &req.agent_id)
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Register commands in lib.rs**

Update imports:
```rust
use commands::session::{
    create_group_session, create_private_session, delete_session, get_group_members,
    get_session, list_sessions, get_session_config, update_session_config,
    reset_session, disband_group, add_group_member, remove_group_member,
};
```

Add to `tauri::generate_handler!`:
```rust
            get_session_config,
            update_session_config,
            reset_session,
            disband_group,
            add_group_member,
            remove_group_member,
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/session.rs src-tauri/src/lib.rs
git commit -m "commands: add session config, reset, disband, member management"
```

---

## Task 5: Backend Message Repository — Page Index Support

**Files:**
- Modify: `src-tauri/src/db/message.rs`

- [ ] **Step 1: Update get_messages_by_session to filter by page_index**

Modify `get_messages_by_session` to accept an optional `page_index` parameter. For backwards compatibility, pass 0 when not using pagination.

Replace the function signature and body:

```rust
pub fn get_messages_by_session(
    conn: &Connection,
    session_id: &str,
    page_index: i32,
    limit: i32,
    offset: i32,
) -> Result<Vec<Message>> {
    let mut stmt = conn.prepare(
        "SELECT m.id, m.session_id, m.sender_type, m.sender_id, 
                COALESCE(a.name, CASE WHEN m.sender_type = 'user' THEN '用户' ELSE '未知' END) as sender_name,
                a.avatar_path as sender_avatar,
                m.content, m.created_at, m.message_type, m.tool_call_data, m.generation_info, m.is_deleted
         FROM messages m
         LEFT JOIN agents a ON m.sender_type = 'agent' AND m.sender_id = a.id AND a.is_deleted = 0
         WHERE m.session_id = ?1 AND m.is_deleted = 0 AND m.page_index = ?2
         ORDER BY m.created_at DESC LIMIT ?3 OFFSET ?4"
    )?;
    let rows = stmt.query_map(rusqlite::params![session_id, page_index, limit, offset], |row| {
        Ok(Message {
            id: row.get(0)?,
            session_id: row.get(1)?,
            sender_type: row.get(2)?,
            sender_id: row.get(3)?,
            sender_name: row.get(4)?,
            sender_avatar: row.get(5)?,
            content: row.get(6)?,
            created_at: row.get(7)?,
            message_type: row.get(8)?,
            tool_call_data: row.get(9)?,
            generation_info: row.get(10)?,
            is_deleted: row.get::<_, i32>(11)? != 0,
        })
    })?;
    rows.collect()
}
```

- [ ] **Step 2: Update insert_message to include page_index**

Modify `insert_message` to read the current page from session and include it:

```rust
pub fn insert_message(
    conn: &Connection,
    session_id: &str,
    sender_type: &str,
    sender_id: &str,
    content: &str,
    message_type: &str,
) -> Result<Message> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    // Get current page_index
    let page_index: i32 = conn.query_row(
        "SELECT COALESCE(current_chat_page, 0) FROM private_sessions WHERE session_id = ?1
         UNION ALL
         SELECT COALESCE(current_chat_page, 0) FROM group_sessions WHERE session_id = ?1
         LIMIT 1",
        [session_id],
        |row| row.get(0),
    ).unwrap_or(0);

    conn.execute(
        r#"INSERT INTO messages (
            id, session_id, sender_type, sender_id, content, created_at, message_type, page_index
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"#,
        (id.clone(), session_id, sender_type, sender_id, content, now, message_type, page_index),
    )?;

    get_message_by_id(conn, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}
```

- [ ] **Step 3: Update callers of get_messages_by_session**

Find and update all callers. In `commands/message.rs`:

```rust
// Change:
let messages = message_repo::get_messages_by_session(&conn, &session_id, 100, 0)?;
// To:
let messages = message_repo::get_messages_by_session(&conn, &session_id, 0, 100, 0)?;
```

Also update `messageStore.svelte.ts` if it passes parameters.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db/message.rs src-tauri/src/commands/message.rs
git commit -m "db: add page_index filtering to messages"
```

---

## Task 6: Backend PromptAssembler — History Limit & Page Filter

**Files:**
- Modify: `src-tauri/src/llm/prompt.rs`

- [ ] **Step 1: Update get_visible_messages_for_agent to accept limit and page filter**

Replace the function:

```rust
pub fn get_visible_messages_for_agent(
    conn: &Connection,
    agent_id: &str,
    history_limit: i32,
) -> Result<Vec<Message>> {
    let sql = format!(
        "SELECT {} FROM messages m
         WHERE m.is_deleted = 0 
         AND m.session_id IN ( 
             SELECT session_id FROM private_sessions WHERE agent_id = ?1 
             UNION 
             SELECT session_id FROM group_members WHERE participant_id = ?1 AND participant_type = 'agent' 
         ) 
         AND m.page_index = (
             SELECT COALESCE(current_chat_page, 0) FROM private_sessions ps WHERE ps.session_id = m.session_id
             UNION ALL
             SELECT COALESCE(current_chat_page, 0) FROM group_sessions gs WHERE gs.session_id = m.session_id
             LIMIT 1
         )
         ORDER BY m.created_at ASC LIMIT ?2",
        SELECT_COLUMNS
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![agent_id, history_limit], row_to_message)?;
    rows.collect()
}
```

Actually, the subquery per row is inefficient. Use a JOIN approach:

```rust
pub fn get_visible_messages_for_agent(
    conn: &Connection,
    agent_id: &str,
    history_limit: i32,
) -> Result<Vec<Message>> {
    let sql = format!(
        "SELECT {} FROM messages m
         JOIN ( 
             SELECT session_id, COALESCE(current_chat_page, 0) as page 
             FROM private_sessions WHERE agent_id = ?1 
             UNION 
             SELECT session_id, COALESCE(current_chat_page, 0) as page 
             FROM group_sessions gs
             JOIN group_members gm ON gs.session_id = gm.session_id
             WHERE gm.participant_id = ?1 AND gm.participant_type = 'agent' 
         ) sp ON m.session_id = sp.session_id AND m.page_index = sp.page
         WHERE m.is_deleted = 0 
         ORDER BY m.created_at ASC LIMIT ?2",
        SELECT_COLUMNS
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params![agent_id, history_limit], row_to_message)?;
    rows.collect()
}
```

- [ ] **Step 2: Update assemble to read history_limit from session_settings**

In `assemble`, replace the history fetching:

```rust
        // Layer 4: Chat History
        let history = {
            // Default history limit if no config exists
            let default_limit = 50i32;
            let history_limit: i32 = conn.query_row(
                "SELECT COALESCE(history_limit, ?1) FROM session_settings 
                 WHERE session_id IN (
                     SELECT session_id FROM private_sessions WHERE agent_id = ?2
                     UNION
                     SELECT session_id FROM group_members WHERE participant_id = ?2 AND participant_type = 'agent'
                 ) LIMIT 1",
                rusqlite::params![default_limit, agent_id],
                |row| row.get(0),
            ).unwrap_or(default_limit);
            
            crate::db::message::get_visible_messages_for_agent(conn, agent_id, history_limit)
                .map_err(|e| e.to_string())?
        };
```

Wait, this query is wrong — it would return one limit across all sessions. We need per-session limits. Actually, `get_visible_messages_for_agent` fetches messages across ALL sessions for the agent. We need per-session limits.

Better approach: fetch messages per-session with each session's own limit:

```rust
        // Layer 4: Chat History — per session with individual history limits
        let mut session_order: Vec<String> = Vec::new();
        let mut grouped: HashMap<String, Vec<Message>> = HashMap::new();
        
        {
            let mut stmt = conn.prepare(
                "SELECT m.id, m.session_id, m.sender_type, m.sender_id, m.content, m.created_at, 
                        m.message_type, m.tool_call_data, m.generation_info, m.is_deleted,
                        COALESCE(a.name, CASE WHEN m.sender_type = 'user' THEN '用户' ELSE '未知' END) as sender_name,
                        a.avatar_path as sender_avatar
                 FROM messages m
                 JOIN (
                     SELECT session_id, COALESCE(current_chat_page, 0) as page FROM private_sessions WHERE agent_id = ?1
                     UNION
                     SELECT gs.session_id, COALESCE(gs.current_chat_page, 0) as page 
                     FROM group_sessions gs
                     JOIN group_members gm ON gs.session_id = gm.session_id
                     WHERE gm.participant_id = ?1 AND gm.participant_type = 'agent'
                 ) sp ON m.session_id = sp.session_id AND m.page_index = sp.page
                 LEFT JOIN agents a ON m.sender_type = 'agent' AND m.sender_id = a.id AND a.is_deleted = 0
                 WHERE m.is_deleted = 0
                 ORDER BY m.created_at DESC"
            ).map_err(|e| e.to_string())?;
            
            let rows = stmt.query_map([agent_id], |row| {
                Ok(Message {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    sender_type: row.get(2)?,
                    sender_id: row.get(3)?,
                    content: row.get(4)?,
                    created_at: row.get(5)?,
                    message_type: row.get(6)?,
                    tool_call_data: row.get(7)?,
                    generation_info: row.get(8)?,
                    is_deleted: row.get::<_, i32>(9)? != 0,
                    sender_name: row.get(10)?,
                    sender_avatar: row.get(11)?,
                })
            }).map_err(|e| e.to_string())?;
            
            for row in rows {
                let msg = row.map_err(|e| e.to_string())?;
                if !grouped.contains_key(&msg.session_id) {
                    session_order.push(msg.session_id.clone());
                }
                grouped.entry(msg.session_id.clone()).or_default().push(msg);
            }
        }
        
        // Apply per-session history limits
        let mut filtered_messages: Vec<Message> = Vec::new();
        for sid in &session_order {
            if let Some(msgs) = grouped.get_mut(sid) {
                // Reverse to chronological order
                msgs.reverse();
                
                let limit: i32 = conn.query_row(
                    "SELECT COALESCE(history_limit, 50) FROM session_settings WHERE session_id = ?1",
                    [sid],
                    |row| row.get(0),
                ).unwrap_or(50);
                
                let take = msgs.len().min(limit as usize);
                filtered_messages.extend(msgs.drain(..take));
            }
        }
        
        // Re-sort all filtered messages by created_at
        filtered_messages.sort_by_key(|m| m.created_at);
```

This is getting complex. For the plan, I'll simplify: use a single query that joins session_settings and applies per-session limits in SQL using window functions, or fetch all and filter in Rust.

For simplicity in the plan, let's use the Rust filter approach shown above. The subagent can refine the SQL.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/llm/prompt.rs
git commit -m "llm: adapt PromptAssembler for history_limit and page_index"
```

---

## Task 7: Backend Scheduler — Read from session_settings

**Files:**
- Modify: `src-tauri/src/scheduler/mod.rs`

- [ ] **Step 1: Update on_new_message to check mute_enabled from session_settings**

In `on_new_message`, before pushing to pending_queue, check mute:

```rust
        // Check if session is muted
        let is_muted: bool = conn.query_row(
            "SELECT mute_enabled FROM session_settings WHERE session_id = ?1",
            [session_id],
            |row| Ok(row.get::<_, i32>(0)? != 0),
        ).unwrap_or(false);
        
        if is_muted && message.sender_type != "user" {
            // Muted session: don't trigger agents for non-user messages
            // But still allow user messages to be processed normally
        }
```

Actually, the requirement says: "禁言后角色被调用时可以看到该会话，但不能往该会话发送消息". This means mute should NOT block `on_new_message` from adding to pending_queue — it should block the actual trigger in `try_trigger_agent` or `trigger_agent_inner`.

So in `trigger_agent_inner`, after getting pending messages, filter out messages from muted sessions:

```rust
        // Filter out messages from muted sessions
        let conn = self.db_state.0.lock().await;
        let muted_sessions: Vec<String> = {
            let mut stmt = conn.prepare(
                "SELECT session_id FROM session_settings WHERE mute_enabled = 1 AND session_id IN (
                    SELECT DISTINCT session_id FROM messages WHERE id IN (
                        SELECT id FROM messages WHERE session_id IN (SELECT session_id FROM ...)
                    )
                )"
            ).unwrap();
            // Simpler: check each pending message's session
            let mut muted = Vec::new();
            for msg in &pending {
                let m: bool = conn.query_row(
                    "SELECT mute_enabled FROM session_settings WHERE session_id = ?1",
                    [&msg.session_id],
                    |row| Ok(row.get::<_, i32>(0)? != 0),
                ).unwrap_or(false);
                if m {
                    muted.push(msg.session_id.clone());
                }
            }
            muted
        };
        drop(conn);
        
        let pending: Vec<PendingMessage> = pending.into_iter()
            .filter(|p| !muted_sessions.contains(&p.session_id))
            .collect();
```

- [ ] **Step 2: Update trigger_agent_inner to read message_limit from session_settings**

Replace the message limit check section:

```rust
            for sid in pending.iter().map(|p| &p.session_id).collect::<std::collections::HashSet<_>>() {
                let (count, limit, enabled): (i32, Option<i32>, bool) = conn.query_row(
                    "SELECT ps.agent_message_count, COALESCE(ss.message_limit, ?2), ss.message_limit_enabled 
                     FROM private_sessions ps
                     LEFT JOIN session_settings ss ON ps.session_id = ss.session_id
                     WHERE ps.session_id = ?1
                     UNION ALL
                     SELECT gs.agent_message_count, COALESCE(ss.message_limit, ?3), ss.message_limit_enabled 
                     FROM group_sessions gs
                     LEFT JOIN session_settings ss ON gs.session_id = ss.session_id
                     WHERE gs.session_id = ?1
                     LIMIT 1",
                    rusqlite::params![sid, settings.private_message_limit_default, settings.group_message_limit_default],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get::<_, i32>(2)? != 0)),
                ).unwrap_or((0, None, false));
                
                if enabled {
                    if count >= limit.unwrap_or(if session_type == "private" { settings.private_message_limit_default } else { settings.group_message_limit_default }) {
                        limited_sessions.push(sid.clone());
                    }
                }
            }
```

Wait, `session_type` is not available in that scope. We can simplify by using the existing query pattern but joining with session_settings:

```rust
                if let Ok((count, limit, enabled)) = conn.query_row(
                    "SELECT ps.agent_message_count, COALESCE(ss.message_limit, ?2), ss.message_limit_enabled 
                     FROM private_sessions ps
                     LEFT JOIN session_settings ss ON ps.session_id = ss.session_id
                     WHERE ps.session_id = ?1
                     UNION ALL
                     SELECT gs.agent_message_count, COALESCE(ss.message_limit, ?3), ss.message_limit_enabled 
                     FROM group_sessions gs
                     LEFT JOIN session_settings ss ON gs.session_id = ss.session_id
                     WHERE gs.session_id = ?1
                     LIMIT 1",
                    rusqlite::params![sid, settings.private_message_limit_default, settings.group_message_limit_default],
                    |row| Ok((row.get::<_, i32>(0)?, row.get::<_, Option<i32>>(1)?, row.get::<_, i32>(2)? != 0)),
                ) {
                    if enabled {
                        let effective = limit.unwrap_or(settings.private_message_limit_default);
                        if count >= effective {
                            limited_sessions.push(sid.clone());
                        }
                    }
                }
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/scheduler/mod.rs
git commit -m "scheduler: read mute and message_limit from session_settings"
```

---

## Task 8: Frontend Types & Store

**Files:**
- Modify: `src/lib/types.ts`
- Modify: `src/lib/stores/sessionStore.svelte.ts`

- [ ] **Step 1: Add types to types.ts**

Append:

```typescript
export interface SessionConfig {
    session_id: string;
    history_limit: number;
    message_limit: number;
    message_limit_enabled: boolean;
    mute_enabled: boolean;
}

export interface UpdateSessionConfigRequest {
    session_id: string;
    history_limit?: number;
    message_limit?: number;
    message_limit_enabled?: boolean;
    mute_enabled?: boolean;
}
```

- [ ] **Step 2: Update sessionStore**

Add methods to SessionStore:

```typescript
    async resetSession(sessionId: string): Promise<string> {
        try {
            const pageId = await invoke<string>('reset_session', { req: { session_id: sessionId } });
            await this.loadSessions();
            return pageId;
        } catch (err) {
            logger.error('Failed to reset session:', err);
            throw err;
        }
    }

    async disbandGroup(sessionId: string): Promise<boolean> {
        try {
            const result = await invoke<boolean>('disband_group', { sessionId });
            if (result) {
                this.sessions = this.sessions.filter(s => s.id !== sessionId);
                if (this.selectedSessionId === sessionId) {
                    this.selectedSessionId = null;
                }
            }
            return result;
        } catch (err) {
            logger.error('Failed to disband group:', err);
            throw err;
        }
    }

    removeSession(sessionId: string) {
        this.sessions = this.sessions.filter(s => s.id !== sessionId);
        if (this.selectedSessionId === sessionId) {
            this.selectedSessionId = null;
        }
    }
```

- [ ] **Step 3: Commit**

```bash
git add src/lib/types.ts src/lib/stores/sessionStore.svelte.ts
git commit -m "frontend: add SessionConfig types and store methods"
```

---

## Task 9: Frontend Shared Components

**Files:**
- Create: `src/lib/components/ConfirmDialog.svelte`
- Create: `src/lib/components/AddMemberModal.svelte`

- [ ] **Step 1: Create ConfirmDialog.svelte**

```svelte
<script lang="ts">
    interface Props {
        open: boolean;
        title: string;
        content: string;
        confirmText: string;
        confirmClass?: string;
        onConfirm: () => void;
        onCancel: () => void;
    }

    let { open, title, content, confirmText, confirmClass = 'bg-primary text-white', onConfirm, onCancel }: Props = $props();
</script>

{#if open}
    <div class="fixed inset-0 z-[100] flex items-center justify-center bg-black/50" onclick={onCancel}>
        <div class="bg-surface rounded-xl p-6 w-80 max-w-full shadow-lg border border-border" onclick={(e) => e.stopPropagation()}>
            <h3 class="text-lg font-semibold mb-2">{title}</h3>
            <p class="text-sm text-text-secondary mb-6">{content}</p>
            <div class="flex justify-end gap-2">
                <button onclick={onCancel} class="px-4 py-2 text-sm rounded-lg hover:bg-bg transition-colors">取消</button>
                <button onclick={onConfirm} class="px-4 py-2 text-sm rounded-lg {confirmClass}">{confirmText}</button>
            </div>
        </div>
    </div>
{/if}
```

- [ ] **Step 2: Create AddMemberModal.svelte**

This reuses the multi-select agent logic from CreateGroupModal.

```svelte
<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { agentStore } from '$lib/stores/agentStore.svelte';
    import { logger } from '$lib/logger';
    import { User, X } from 'lucide-svelte';

    interface Props {
        open: boolean;
        sessionId: string;
        existingMemberIds: string[];
        onClose: () => void;
        onAdded: () => void;
    }

    let { open, sessionId, existingMemberIds, onClose, onAdded }: Props = $props();
    let selectedIds = $state<string[]>([]);
    let loading = $state(false);

    const availableAgents = $derived(
        agentStore.agents.filter(a => !existingMemberIds.includes(a.id) && !a.is_deleted)
    );

    function toggleAgent(id: string) {
        if (selectedIds.includes(id)) {
            selectedIds = selectedIds.filter(x => x !== id);
        } else {
            selectedIds = [...selectedIds, id];
        }
    }

    async function handleAdd() {
        if (selectedIds.length === 0) return;
        loading = true;
        try {
            for (const agentId of selectedIds) {
                await invoke('add_group_member', { req: { session_id: sessionId, agent_id: agentId } });
            }
            selectedIds = [];
            onAdded();
            onClose();
        } catch (err) {
            logger.error('Failed to add members:', err);
        } finally {
            loading = false;
        }
    }
</script>

{#if open}
    <div class="fixed inset-0 z-[100] flex items-center justify-center bg-black/50" onclick={onClose}>
        <div class="bg-surface rounded-xl p-6 w-96 max-w-full shadow-lg border border-border" onclick={(e) => e.stopPropagation()}>
            <div class="flex items-center justify-between mb-4">
                <h3 class="text-lg font-semibold">添加成员</h3>
                <button onclick={onClose} class="p-1 hover:bg-bg rounded-lg"><X size={18} /></button>
            </div>
            <div class="max-h-64 overflow-y-auto space-y-1 mb-4">
                {#each availableAgents as agent}
                    <button
                        onclick={() => toggleAgent(agent.id)}
                        class="w-full flex items-center gap-3 p-2 rounded-lg hover:bg-bg text-left {selectedIds.includes(agent.id) ? 'bg-primary/10 ring-1 ring-primary' : ''}"
                    >
                        <div class="w-8 h-8 rounded-full bg-primary/10 flex items-center justify-center text-primary shrink-0 overflow-hidden">
                            {#if agent.avatar_path}
                                <img src={agent.avatar_path} alt={agent.name} class="w-full h-full object-cover" />
                            {:else}
                                <User size={16} />
                            {/if}
                        </div>
                        <span class="text-sm">{agent.name}</span>
                    </button>
                {:else}
                    <p class="text-sm text-text-secondary p-2">没有可添加的角色</p>
                {/each}
            </div>
            <button
                onclick={handleAdd}
                disabled={selectedIds.length === 0 || loading}
                class="w-full py-2 bg-primary text-white rounded-lg hover:bg-primary-dark disabled:opacity-50"
            >
                {loading ? '添加中...' : `添加 (${selectedIds.length})`}
            </button>
        </div>
    </div>
{/if}
```

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/ConfirmDialog.svelte src/lib/components/AddMemberModal.svelte
git commit -m "frontend: add ConfirmDialog and AddMemberModal components"
```

---

## Task 10: SessionSettingsPanel Component

**Files:**
- Create: `src/lib/components/SessionSettingsPanel.svelte`

- [ ] **Step 1: Create SessionSettingsPanel.svelte**

This is the main drawer component. It's long but straightforward:

```svelte
<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { slide } from 'svelte/transition';
    import { X, User, Trash2, RotateCcw } from 'lucide-svelte';
    import { logger } from '$lib/logger';
    import { sessionStore } from '$lib/stores/sessionStore.svelte';
    import type { SessionConfig, GroupMember } from '$lib/types';
    import ConfirmDialog from './ConfirmDialog.svelte';
    import AddMemberModal from './AddMemberModal.svelte';

    interface Props {
        open: boolean;
        sessionId: string;
        sessionType: string;
        members: GroupMember[];
        onClose: () => void;
        onMembersChange: () => void;
    }

    let { open, sessionId, sessionType, members, onClose, onMembersChange }: Props = $props();

    let config = $state<SessionConfig | null>(null);
    let loading = $state(false);
    let saveTimer: ReturnType<typeof setTimeout> | null = null;
    let showResetConfirm = $state(false);
    let showDisbandConfirm = $state(false);
    let showAddMember = $state(false);

    $effect(() => {
        if (open && sessionId) {
            loadConfig();
        }
    });

    async function loadConfig() {
        try {
            const data = await invoke<SessionConfig>('get_session_config', { sessionId, sessionType });
            config = data;
        } catch (err) {
            logger.error('Failed to load session config:', err);
        }
    }

    function queueSave(updates: Partial<SessionConfig>) {
        if (saveTimer) clearTimeout(saveTimer);
        saveTimer = setTimeout(() => {
            doSave(updates);
        }, 500);
    }

    async function doSave(updates: Partial<SessionConfig>) {
        if (!config) return;
        try {
            await invoke('update_session_config', {
                req: {
                    session_id: sessionId,
                    history_limit: updates.history_limit,
                    message_limit: updates.message_limit,
                    message_limit_enabled: updates.message_limit_enabled,
                    mute_enabled: updates.mute_enabled,
                }
            });
            // Show toast if available, or just update local
        } catch (err) {
            logger.error('Failed to save config:', err);
        }
    }

    async function handleReset() {
        try {
            await sessionStore.resetSession(sessionId);
            showResetConfirm = false;
            onClose();
        } catch (err) {
            logger.error('Reset failed:', err);
        }
    }

    async function handleDisband() {
        try {
            await sessionStore.disbandGroup(sessionId);
            showDisbandConfirm = false;
            onClose();
        } catch (err) {
            logger.error('Disband failed:', err);
        }
    }

    async function handleRemoveMember(agentId: string) {
        if (members.filter(m => m.participant_type === 'agent').length <= 2) {
            alert('群聊至少需要保留 2 名角色成员');
            return;
        }
        try {
            await invoke('remove_group_member', { req: { session_id: sessionId, agent_id: agentId } });
            onMembersChange();
        } catch (err) {
            logger.error('Failed to remove member:', err);
        }
    }
</script>

{#if open}
    <div class="absolute inset-y-0 right-0 w-72 bg-surface border-l border-border z-50 flex flex-col shadow-xl"
         transition:slide={{ duration: 200, axis: 'x' }}>
        <!-- Header -->
        <div class="flex items-center justify-between p-4 border-b border-border">
            <h3 class="font-semibold">会话配置</h3>
            <button onclick={onClose} class="p-1 hover:bg-bg rounded-lg"><X size={18} /></button>
        </div>

        <!-- Scrollable content -->
        <div class="flex-1 overflow-y-auto p-4 space-y-6">
            {#if config}
                <!-- History Limit -->
                <div>
                    <label class="block text-sm font-medium mb-1">历史提示条数</label>
                    <p class="text-xs text-text-secondary mb-2">角色在 Prompt 中能看到该会话的最近 N 条消息</p>
                    <input
                        type="number"
                        min={1}
                        max={200}
                        value={config.history_limit}
                        onchange={(e) => {
                            const v = parseInt(e.currentTarget.value);
                            config = { ...config!, history_limit: v };
                            queueSave({ history_limit: v });
                        }}
                        class="w-full px-3 py-2 bg-bg border border-border rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-primary/20"
                    />
                </div>

                <!-- Message Limit -->
                <div>
                    <div class="flex items-center justify-between mb-1">
                        <label class="text-sm font-medium">自动消息限制</label>
                        <button
                            onclick={() => {
                                const v = !config!.message_limit_enabled;
                                config = { ...config!, message_limit_enabled: v };
                                queueSave({ message_limit_enabled: v });
                            }}
                            class="relative w-10 h-5 rounded-full transition-colors {config.message_limit_enabled ? 'bg-primary' : 'bg-gray-300'}"
                        >
                            <span class="absolute top-0.5 left-0.5 w-4 h-4 bg-white rounded-full transition-transform {config.message_limit_enabled ? 'translate-x-5' : ''}" />
                        </button>
                    </div>
                    <p class="text-xs text-text-secondary mb-2">角色在此会话中最多发送 N 条消息后自动停止</p>
                    <input
                        type="number"
                        min={1}
                        max={999}
                        disabled={!config.message_limit_enabled}
                        value={config.message_limit}
                        onchange={(e) => {
                            const v = parseInt(e.currentTarget.value);
                            config = { ...config!, message_limit: v };
                            queueSave({ message_limit: v });
                        }}
                        class="w-full px-3 py-2 bg-bg border border-border rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-primary/20 disabled:opacity-50"
                    />
                </div>

                <!-- Mute -->
                <div>
                    <div class="flex items-center justify-between mb-1">
                        <label class="text-sm font-medium">禁言</label>
                        <button
                            onclick={() => {
                                const v = !config!.mute_enabled;
                                config = { ...config!, mute_enabled: v };
                                queueSave({ mute_enabled: v });
                            }}
                            class="relative w-10 h-5 rounded-full transition-colors {config.mute_enabled ? 'bg-primary' : 'bg-gray-300'}"
                        >
                            <span class="absolute top-0.5 left-0.5 w-4 h-4 bg-white rounded-full transition-transform {config.mute_enabled ? 'translate-x-5' : ''}" />
                        </button>
                    </div>
                    <p class="text-xs text-text-secondary">开启后角色不会自动回复，但你仍可发送消息</p>
                </div>

                <!-- Member Management (group only) -->
                {#if sessionType === 'group'}
                    <div>
                        <label class="block text-sm font-medium mb-2">成员管理</label>
                        <div class="space-y-1 mb-2">
                            {#each members as member}
                                <div class="flex items-center justify-between p-2 rounded-lg bg-bg">
                                    <div class="flex items-center gap-2">
                                        <div class="w-7 h-7 rounded-full bg-primary/10 flex items-center justify-center text-primary shrink-0 overflow-hidden">
                                            {#if member.avatar_path}
                                                <img src={member.avatar_path} alt={member.name} class="w-full h-full object-cover" />
                                            {:else}
                                                <User size={14} />
                                            {/if}
                                        </div>
                                        <span class="text-sm">{member.name}</span>
                                    </div>
                                    {#if member.participant_type === 'agent'}
                                        <button
                                            onclick={() => handleRemoveMember(member.participant_id)}
                                            class="p-1 text-text-secondary hover:text-red-500 rounded"
                                            title="移除成员"
                                        >
                                            <X size={14} />
                                        </button>
                                    {/if}
                                </div>
                            {/each}
                        </div>
                        <button
                            onclick={() => showAddMember = true}
                            class="w-full py-1.5 text-sm border border-border rounded-lg hover:bg-bg transition-colors"
                        >
                            + 添加成员
                        </button>
                    </div>
                {/if}

                <!-- Reset Session -->
                <div class="pt-4 border-t border-border">
                    <button
                        onclick={() => showResetConfirm = true}
                        class="flex items-center gap-2 text-sm text-red-500 hover:text-red-600"
                    >
                        <RotateCcw size={16} />
                        重置{sessionType === 'group' ? '群聊' : '会话'}
                    </button>
                </div>

                <!-- Disband Group (group only) -->
                {#if sessionType === 'group'}
                    <div class="pt-2">
                        <button
                            onclick={() => showDisbandConfirm = true}
                            class="w-full py-2 bg-red-500 text-white rounded-lg hover:bg-red-600 text-sm flex items-center justify-center gap-2"
                        >
                            <Trash2 size={16} />
                            解散群聊
                        </button>
                    </div>
                {/if}
            {:else}
                <div class="text-sm text-text-secondary">加载中...</div>
            {/if}
        </div>
    </div>
{/if}

<!-- Confirm dialogs -->
<ConfirmDialog
    open={showResetConfirm}
    title="重置{sessionType === 'group' ? '群聊' : '会话'}"
    content="重置后当前聊天记录将被归档，相同成员开启新会话。此操作不可撤销。"
    confirmText="确认重置"
    confirmClass="bg-red-500 text-white hover:bg-red-600"
    onConfirm={handleReset}
    onCancel={() => showResetConfirm = false}
/>

<ConfirmDialog
    open={showDisbandConfirm}
    title="解散群聊"
    content="解散后群聊将从列表中移除，聊天记录保留在历史记录中。"
    confirmText="确认解散"
    confirmClass="bg-red-500 text-white hover:bg-red-600"
    onConfirm={handleDisband}
    onCancel={() => showDisbandConfirm = false}
/>

<AddMemberModal
    open={showAddMember}
    {sessionId}
    existingMemberIds={members.map(m => m.participant_id)}
    onClose={() => showAddMember = false}
    onAdded={onMembersChange}
/>
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/components/SessionSettingsPanel.svelte
git commit -m "frontend: add SessionSettingsPanel drawer component"
```

---

## Task 11: ChatView Integration

**Files:**
- Modify: `src/lib/components/ChatView.svelte`

- [ ] **Step 1: Import and integrate settings panel**

Add imports:
```typescript
import { Settings } from 'lucide-svelte';
import SessionSettingsPanel from './SessionSettingsPanel.svelte';
```

Add state:
```typescript
let settingsOpen = $state(false);
```

In Header section, add settings button:
```svelte
            <div class="flex items-center justify-between w-full">
                <div class="flex items-center gap-3">
                    <!-- existing avatar and name -->
                </div>
                {#if selectedSession}
                    <button
                        onclick={() => settingsOpen = !settingsOpen}
                        class="p-2 hover:bg-bg rounded-lg text-text-secondary transition-colors"
                        title="会话配置"
                    >
                        <Settings size={20} />
                    </button>
                {/if}
            </div>
```

Wrap the main content and member list in a relative container so the panel can overlay:

```svelte
<div class="flex h-full bg-bg relative">
    <!-- settings panel overlays here when open -->
    <SessionSettingsPanel
        open={settingsOpen}
        sessionId={selectedSession.id}
        sessionType={selectedSession.session_type}
        {members}
        onClose={() => settingsOpen = false}
        onMembersChange={() => {
            // reload members
            if (selectedSession?.session_type === 'group') {
                invoke<GroupMember[]>('get_group_members', { sessionId: selectedSession.id })
                    .then((data) => { members = data; })
                    .catch((err) => logger.error('Failed to reload members:', err));
            }
        }}
    />
    <!-- rest of ChatView -->
</div>
```

Also add click-outside to close settings:
```typescript
    function handleClickOutside(e: MouseEvent) {
        if (settingsOpen && !(e.target as HTMLElement).closest('.session-settings-panel')) {
            settingsOpen = false;
        }
    }
```

Bind to window click in onMount or use a svelte action.

- [ ] **Step 2: Commit**

```bash
git add src/lib/components/ChatView.svelte
git commit -m "frontend: integrate SessionSettingsPanel into ChatView"
```

---

## Task 12: Rust Unit Tests

**Files:**
- Modify: `src-tauri/src/db/session.rs`

- [ ] **Step 1: Add tests for session_settings and reset_session**

Add to the existing `tests` module in `db/session.rs`:

```rust
    #[test]
    fn test_session_config_defaults() {
        let conn = init_test_db();
        
        // Insert an agent and a private session
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        let config = get_session_config(&conn, &session.id, "private").unwrap();
        assert_eq!(config.history_limit, 30);
        assert_eq!(config.message_limit, 10);
        assert!(config.message_limit_enabled);
        assert!(!config.mute_enabled);
    }

    #[test]
    fn test_update_session_config() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        update_session_config(&conn, &crate::models::session::UpdateSessionConfigRequest {
            session_id: session.id.clone(),
            history_limit: Some(50),
            message_limit: Some(20),
            message_limit_enabled: Some(false),
            mute_enabled: Some(true),
        }).unwrap();
        
        let config = get_session_config(&conn, &session.id, "private").unwrap();
        assert_eq!(config.history_limit, 50);
        assert_eq!(config.message_limit, 20);
        assert!(!config.message_limit_enabled);
        assert!(config.mute_enabled);
    }

    #[test]
    fn test_reset_session_creates_new_page() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        let page_id = reset_session(&conn, &session.id).unwrap();
        assert!(!page_id.is_empty());
        
        let page_index: i32 = conn.query_row(
            "SELECT page_index FROM chat_pages WHERE id = ?1",
            [&page_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(page_index, 1);
    }
```

- [ ] **Step 2: Run tests**

```bash
cd src-tauri && cargo test
```

Expected: all tests pass (existing + new).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/db/session.rs
git commit -m "test: add session config and reset session tests"
```

---

## Task 13: Documentation & Final Verification

**Files:**
- Modify: `docs/feature_list.md`

- [ ] **Step 1: Update feature_list.md**

Update statuses:
- `SES-05`: 会话配置面板 → `✅ 已实现`
- `SES-07`: 群聊禁言开关 → `✅ 已实现`（纳入配置面板）
- `SES-04`: 会话归档与续开 → `✅ 已实现`（通过重置会话）
- `SES-09`: 解散群聊 → `✅ 已实现`
- `SET-05`: 消息上限设置 → 备注更新为"会话级配置面板已完成"
- `CHAT-04`: 聊天记录分页 → `🚧 部分实现`（chat_pages 基础设施已完成，UI 历史记录查看待完善）

Also update completeness checklist accordingly.

- [ ] **Step 2: Run final checks**

```bash
cd src-tauri && cargo check
cd src-tauri && cargo test
npx svelte-check --tsconfig ./tsconfig.json
```

Expected: 0 errors in cargo check, all tests pass, svelte-check 0 errors.

- [ ] **Step 3: Commit**

```bash
git add docs/feature_list.md
git commit -m "docs: update feature_list for session config panel completion"
```

---

## Spec Coverage Check

| Spec Requirement | Task |
|-----------------|------|
| session_settings 表 | Task 1 |
| page_index + chat_pages | Task 1, 5 |
| get_session_config API | Task 4 |
| update_session_config API | Task 4 |
| reset_session API | Task 3, 4 |
| disband_group API | Task 3, 4 |
| add/remove_group_member API | Task 3, 4 |
| PromptAssembler history_limit | Task 6 |
| Scheduler mute/message_limit from session_settings | Task 7 |
| SessionSettingsPanel drawer UI | Task 10 |
| ChatView integration | Task 11 |
| ConfirmDialog | Task 9 |
| AddMemberModal | Task 9 |

No gaps identified.
