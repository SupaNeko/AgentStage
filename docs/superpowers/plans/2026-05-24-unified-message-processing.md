# Unified Message Processing — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unify agent message post-processing into a shared `handle_agent_response` method, remove `last_message_preview` from all backend layers, and fix taskbar focus detection.

**Architecture:** Extract shared post-LLM handler (`handle_agent_response`) in `scheduler/mod.rs` that both `trigger_agent_inner` and `trigger_special` call. Drop `last_message_preview` DB column + remove it from all Rust models/queries/commands. Frontend takes over preview tracking via `new_message` events + memory preservation in `loadSessions()`.

**Tech Stack:** Rust (rusqlite, Tauri v2), Svelte 5, TypeScript

---

### Task 1: Add `handle_agent_response` shared method

**Files:**
- Modify: `src-tauri/src/scheduler/mod.rs` — insert new method after `trigger_agent_inner`, before `restore_pending`

- [ ] **Step 1: Insert `handle_agent_response` method**

Insert this method after the closing `}` of `trigger_agent_inner` (line 1001), before `restore_pending` (line 1003):

```rust
    /// Unified post-LLM processing for all agent-produced messages.
    /// Called by both trigger_agent_inner (user/chain-triggered) and
    /// trigger_special (proactive/timer-triggered).
    async fn handle_agent_response(
        &self,
        agent_id: &str,
        agent_messages: &[Message],
    ) -> Result<(), String> {
        if agent_messages.is_empty() {
            crate::logger::debug(&format!(
                "[DEBUG handle_agent_response] agent_id={}, no messages, emitting agent_completed",
                agent_id
            ));
            self.emit("agent_completed", serde_json::json!({"agent_id": agent_id}));
            return Ok(());
        }

        // 1. Update agent_message_count for each session
        let mut session_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
        {
            let conn = self.db_state.0.lock().await;
            for msg in agent_messages {
                let rows = conn.execute(
                    "UPDATE private_sessions SET agent_message_count = agent_message_count + 1 WHERE session_id = ?1",
                    [&msg.session_id],
                ).unwrap_or(0);
                if rows == 0 {
                    let _ = conn.execute(
                        "UPDATE group_sessions SET agent_message_count = agent_message_count + 1 WHERE session_id = ?1",
                        [&msg.session_id],
                    );
                }
                session_ids.insert(msg.session_id.clone());
            }
        }

        // 2. Check freeze for each session
        for sid in &session_ids {
            if let Some(session_name) = self.check_and_freeze_if_needed(sid).await {
                self.emit("system_notice", serde_json::json!({
                    "content": format!("{} 已达到消息上限，自动对话已暂停。发送消息或点击重置以继续。", session_name)
                }));
            }
        }

        // 3. Emit new_message + distribute to other agents
        for msg in agent_messages {
            let session_exists: bool = {
                let conn = self.db_state.0.lock().await;
                conn.query_row(
                    "SELECT COUNT(*) FROM sessions WHERE id = ?1 AND is_deleted = 0",
                    [&msg.session_id],
                    |row| Ok(row.get::<_, i32>(0)? > 0),
                ).unwrap_or(false)
            };
            if !session_exists {
                continue;
            }

            self.emit("new_message", msg);
            self.distribute_message(&msg.session_id, msg, agent_id).await?;
        }

        // 4. Emit completion
        self.emit("agent_completed", serde_json::json!({"agent_id": agent_id}));

        Ok(())
    }
```

- [ ] **Step 2: Verify cargo check passes**

```bash
cd src-tauri; cargo check 2>&1
```

Expected: 0 errors. The method references `self.emit`, `self.distribute_message`, `self.check_and_freeze_if_needed`, `self.db_state` — all existing members. `HashSet` is already imported at line 1.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/scheduler/mod.rs
git commit -m "feat: add handle_agent_response shared post-LLM method"
```

---

### Task 2: Refactor `trigger_agent_inner` to use shared method

**Files:**
- Modify: `src-tauri/src/scheduler/mod.rs:905-1001`

- [ ] **Step 1: Replace stage 6 + stage 7 with shared method call**

Replace lines 905-1001 (everything from `// === 阶段 6` through `Ok(())` at the end of `trigger_agent_inner`) with:

```rust
        // === 阶段 6+7：统一后处理（emit, distribute, freeze check, counter） ===
        let post_start = chrono::Utc::now().timestamp_millis();
        self.handle_agent_response(agent_id, &agent_messages).await?;
        let post_elapsed = chrono::Utc::now().timestamp_millis() - post_start;
        crate::logger::debug(&format!(
            "[DEBUG trigger_agent_inner] agent_id={}, unified_post_elapsed_ms={}",
            agent_id, post_elapsed
        ));

        // 更新触发时间
        {
            let conn = self.db_state.0.lock().await;
            trigger_repo::update_trigger_time(&conn, agent_id)
                .map_err(|e| e.to_string())?;
        }

        let inner_elapsed = chrono::Utc::now().timestamp_millis() - inner_start;
        crate::logger::debug(&format!(
            "[DEBUG trigger_agent_inner] END agent_id={}, total_elapsed_ms={}", agent_id, inner_elapsed
        ));

        Ok(())
    }
```

The removed code is:
- Lines 905-929: stage 6 header + `agent_message_count` + `last_message_preview` DB write loop (moved into `handle_agent_response`)
- Lines 931-938: freeze check loop (moved into `handle_agent_response`)
- Lines 946-951: `update_trigger_time` (kept here, above the new block)
- Lines 953-986: stage 7 emit + distribute loop (moved into `handle_agent_response`)
- Lines 995-998: `agent_completed` emit (moved into `handle_agent_response`)

- [ ] **Step 2: Also remove the debug log that uses `truncate_preview`**

At lines 898-903, the debug log uses `truncate_preview(&msg.content, 80)`. Replace with a simple char-based truncation:

```rust
        for (i, msg) in agent_messages.iter().enumerate() {
            let preview: String = msg.content.chars().take(80).collect();
            crate::logger::debug(&format!(
                "[DEBUG trigger_agent_inner] agent_id={}, agent_message[{}]: session_id={}, content_preview={}",
                agent_id, i, msg.session_id, preview
            ));
        }
```

- [ ] **Step 3: Verify cargo check passes**

```bash
cd src-tauri; cargo check 2>&1
```

Expected: 0 errors. `handle_agent_response` exists. Old stage 6/7 references are gone.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/scheduler/mod.rs
git commit -m "refactor: trigger_agent_inner uses handle_agent_response"
```

---

### Task 3: Refactor `trigger_special` to use shared method

**Files:**
- Modify: `src-tauri/src/scheduler/mod.rs:1244-1247`

- [ ] **Step 1: Replace manual emit loop with shared method**

Replace lines 1244-1247:

```rust
            // Emit new_message for each message produced by tool execution (so frontend gets notified)
            for msg in &result.messages {
                scheduler.emit("new_message", msg.clone());
            }
```

With:

```rust
            // Unified post-processing (emit, distribute, freeze check, counter)
            if let Err(e) = scheduler.handle_agent_response(&agent_id_owned, &result.messages).await {
                crate::logger::error(&format!(
                    "[trigger_special] handle_agent_response failed for agent_id={}: {}", agent_id_owned, e
                ));
            }
```

- [ ] **Step 2: Verify cargo check passes**

```bash
cd src-tauri; cargo check 2>&1
```

Expected: 0 errors.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/scheduler/mod.rs
git commit -m "refactor: trigger_special uses handle_agent_response for full post-processing"
```

---

### Task 4: Remove `truncate_preview` function and tests

**Files:**
- Modify: `src-tauri/src/scheduler/mod.rs` — remove lines 1903-1910 + test block lines 2226-2251

- [ ] **Step 1: Remove `truncate_preview` public function**

Delete lines 1903-1910:

```rust
/// 安全截断字符串，按字符计数，避免 UTF-8 切片 panic
pub fn truncate_preview(content: &str, max_chars: usize) -> String {
    if content.chars().count() > max_chars {
        content.chars().take(max_chars).collect::<String>() + "..."
    } else {
        content.to_string()
    }
}
```

- [ ] **Step 2: Remove its test cases**

Delete the test functions `test_truncate_preview_chinese_no_panic`, `test_truncate_preview_exact_boundary`, `test_truncate_preview_short`, `test_truncate_preview_empty` (the 4 functions starting near line 2226).

- [ ] **Step 3: Verify cargo check and cargo test compile**

```bash
cd src-tauri; cargo check 2>&1
```

Expected: 0 errors. No remaining references to `truncate_preview`.

```bash
cd src-tauri; cargo test --no-run 2>&1
```

Expected: tests compile successfully (runtime may not work in dev env, compile-only check).

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/scheduler/mod.rs
git commit -m "refactor: remove truncate_preview function (no longer needed)"
```

---

### Task 5: Database migration to drop `last_message_preview` column

**Files:**
- Modify: `src-tauri/src/db/schema.rs` — add MIGRATION_V16, update MIGRATION_V1
- Modify: `src-tauri/src/db/migration.rs` — add V16 entry

- [ ] **Step 1: Add MIGRATION_V16 to schema.rs**

Insert after the MIGRATION_V15 block (before the final blank line at end of file):

```rust
pub const MIGRATION_V16: &str = r#"
-- V16: Drop last_message_preview (frontend maintains preview state)
ALTER TABLE sessions DROP COLUMN last_message_preview;
"#;
```

If `DROP COLUMN` fails at runtime on older SQLite, the fallback will be added in a follow-up commit if needed. Modern rusqlite on Windows supports it.

- [ ] **Step 2: Remove `last_message_preview` from MIGRATION_V1 DDL**

In `MIGRATION_V1`, line 52, change the sessions table:
```sql
    last_message_preview TEXT,
```

Remove this line entirely. The `,` on the preceding line (`last_message_at INTEGER,`) should be kept — it becomes the line before `unread_count INTEGER DEFAULT 0,`.

- [ ] **Step 3: Add V16 to MIGRATIONS array in migration.rs**

After the V15 entry (line 76-80), add:

```rust
    Migration {
        version: 16,
        name: "drop_last_message_preview",
        sql: super::schema::MIGRATION_V16,
    },
```

- [ ] **Step 4: Verify cargo check passes**

```bash
cd src-tauri; cargo check 2>&1
```

Expected: 0 errors.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db/schema.rs src-tauri/src/db/migration.rs
git commit -m "feat: add migration V16 to drop last_message_preview column"
```

---

### Task 6: Remove `last_message_preview` from Rust models

**Files:**
- Modify: `src-tauri/src/models/session.rs` — lines 10, 45

- [ ] **Step 1: Remove from `Session` model (line 10)**

Delete line 10:
```rust
    pub last_message_preview: Option<String>,
```

- [ ] **Step 2: Remove from `SessionResponse` model (line 45)**

Delete line 45:
```rust
    pub last_message_preview: Option<String>,
```

- [ ] **Step 3: Verify cargo check**

```bash
cd src-tauri; cargo check 2>&1
```

Expected: Compile errors where `last_message_preview` is still referenced in `db/session.rs` — expected, we'll fix those in Task 7.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/models/session.rs
git commit -m "refactor: remove last_message_preview from Session and SessionResponse models"
```

---

### Task 7: Update session repository queries and remove preview writes

**Files:**
- Modify: `src-tauri/src/db/session.rs` — 3 SELECT queries, `build_session_response_from_row`, remove `update_session_last_message`, remove preview clear in `reset_session`

- [ ] **Step 1: Update `get_session_by_id` SQL (lines 101-112)**

Remove `s.last_message_preview` from the SELECT. Since it's at column index 3, all subsequent indices stay the same (we'll handle the mapping function separately):

```rust
    let mut stmt = conn.prepare(
        "SELECT s.id, s.session_type, s.last_message_at, s.unread_count,
                COALESCE(ps.current_chat_page, gs.current_chat_page, 0),
                ss.mute_enabled,
                gs.name,
                gs.avatar_path,
                gs.is_dissolved
         FROM sessions s
         LEFT JOIN private_sessions ps ON s.id = ps.session_id
         LEFT JOIN group_sessions gs ON s.id = gs.session_id
         LEFT JOIN session_settings ss ON s.id = ss.session_id
         WHERE s.id = ?1 AND s.is_deleted = 0"
    )?;
```

- [ ] **Step 2: Update `list_sessions` SQL (lines 124-138)**

Same change — remove `s.last_message_preview`:

```rust
    let mut stmt = conn.prepare(
        "SELECT s.id, s.session_type, s.last_message_at, s.unread_count,
                COALESCE(ps.current_chat_page, gs.current_chat_page, 0),
                ss.mute_enabled,
                gs.name,
                gs.avatar_path,
                gs.is_dissolved
         FROM sessions s
         LEFT JOIN private_sessions ps ON s.id = ps.session_id
         LEFT JOIN group_sessions gs ON s.id = gs.session_id
         LEFT JOIN session_settings ss ON s.id = ss.session_id
         WHERE s.is_deleted = 0
         ORDER BY s.last_message_at DESC"
    )?;
```

- [ ] **Step 3: Update `list_history_sessions` SQL (lines 149-162)**

Same change:

```rust
    let mut stmt = conn.prepare(
        "SELECT s.id, s.session_type, s.last_message_at, s.unread_count,
                COALESCE(ps.current_chat_page, gs.current_chat_page, 0),
                ss.mute_enabled,
                gs.name,
                gs.avatar_path,
                gs.is_dissolved
         FROM sessions s
         LEFT JOIN private_sessions ps ON s.id = ps.session_id
         LEFT JOIN group_sessions gs ON s.id = gs.session_id
         LEFT JOIN session_settings ss ON s.id = ss.session_id
         WHERE s.is_deleted = 0
           AND (SELECT COUNT(*) FROM chat_pages cp WHERE cp.session_id = s.id) > 1
         ORDER BY s.last_message_at DESC"
    )?;
```

- [ ] **Step 4: Update `build_session_response_from_row` (lines 84-98)**

Remove `last_message_preview: row.get(3)?` line. Since `last_message_preview` was column index 3:
- `unread_count` is now at index 3 (was 4)
- `current_chat_page` at index 4 (was 5)
- `mute_enabled` at index 5 (was 6)
- `group_name` at index 7 (was 7, unchanged — but wait, 7 is after 6, so all indices >= 3 shift by 1)

Actually wait, let me recalculate. Original column order:
0: s.id
1: s.session_type
2: s.last_message_at
3: s.last_message_preview ← REMOVED
4: s.unread_count               → becomes 3
5: current_chat_page            → becomes 4
6: ss.mute_enabled              → becomes 5
7: gs.name                      → becomes 6
8: gs.avatar_path               → becomes 7
9: gs.is_dissolved              → becomes 8

Update the function:

```rust
fn build_session_response_from_row(row: &rusqlite::Row) -> Result<SessionResponse> {
    Ok(SessionResponse {
        id: row.get(0)?,
        session_type: row.get(1)?,
        last_message_at: row.get(2)?,
        unread_count: row.get(3)?,
        participants: Vec::new(),
        group_name: row.get(6)?,
        group_avatar: crate::db::resolve_avatar_path(row.get(7)?),
        mute_enabled: row.get::<_, Option<i32>>(5)?.map(|v| v != 0),
        current_chat_page: row.get(4)?,
        is_dissolved: row.get::<_, Option<i32>>(8)?.map(|v| v != 0).unwrap_or(false),
    })
}
```

- [ ] **Step 5: Remove `update_session_last_message` function (lines 318-325)**

Delete the entire function. Also update its callers — but those are in Task 8.

- [ ] **Step 6: Remove preview clear in `reset_session` (lines 561-564)**

Delete these 4 lines:

```rust
    // 清空会话最后消息预览（新 page 没有消息）
    conn.execute(
        "UPDATE sessions SET last_message_preview = '', updated_at = ?1 WHERE id = ?2",
        (now, session_id),
    )?;
```

(Keep everything else in `reset_session` — it still resets other state.)

- [ ] **Step 7: Verify cargo check passes**

```bash
cd src-tauri; cargo check 2>&1
```

Expected: May have errors from `commands/message.rs` referencing `update_session_last_message`. We'll fix that in Task 8.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/db/session.rs
git commit -m "refactor: remove last_message_preview from session queries and update_session_last_message"
```

---

### Task 8: Remove `last_message_preview` usage from commands

**Files:**
- Modify: `src-tauri/src/commands/message.rs:55-57`

- [ ] **Step 1: Remove preview write in `send_user_message`**

Delete lines 55-57:

```rust
    // 更新会话最后消息预览（按字符截断，防止 UTF-8 切片 panic）
    let preview = crate::scheduler::truncate_preview(&req.content, 100);
    let _ = session_repo::update_session_last_message(&conn, &req.session_id, &preview);
```

User messages will be picked up by the `new_message` event flow from `handle_agent_response` when agents reply. The session list will show the agent's reply as preview, which is more useful than showing the user's own message. For the brief moment before the agent replies, the preview stays at its previous value (acceptable UX).

- [ ] **Step 2: Verify cargo check passes**

```bash
cd src-tauri; cargo check 2>&1
```

Expected: 0 errors. No remaining references to `update_session_last_message` or `truncate_preview`.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/message.rs
git commit -m "refactor: remove last_message_preview write from send_user_message"
```

---

### Task 9: Frontend — taskbar focus detection via `onFocusChanged`

**Files:**
- Modify: `src/App.svelte:28-42`

- [ ] **Step 1: Replace `window blur/focus` listeners with `onFocusChanged`**

Replace lines 28-42:

```ts
        // Track window focus via window events (more reliable than isFocused() in WebView2)
        let isWindowFocused = true;
        const onBlur = () => { isWindowFocused = false; };
        const onFocus = () => {
            isWindowFocused = true;
            // Cancel notification state when window gains focus
            if (flashTimeout) {
                clearTimeout(flashTimeout);
                flashTimeout = null;
            }
            win.requestUserAttention(null).catch(() => {});
            win.setProgressBar({ status: ProgressBarStatus.None }).catch(() => {});
        };
        window.addEventListener('blur', onBlur);
        window.addEventListener('focus', onFocus);
```

With:

```ts
        // Track OS-level window focus via Tauri onFocusChanged
        let isWindowFocused = true;
        const unlistenFocus = await win.onFocusChanged((event) => {
            const focused = event.payload;
            isWindowFocused = focused;
            logger.debug('[DEBUG App focusChanged]', { focused });
            if (focused) {
                if (flashTimeout) {
                    clearTimeout(flashTimeout);
                    flashTimeout = null;
                }
                win.requestUserAttention(null).catch(() => {});
                win.setProgressBar({ status: ProgressBarStatus.None }).catch(() => {});
            }
        });
```

- [ ] **Step 2: Update cleanup return**

Replace lines 111-116:

```ts
        return () => {
            window.removeEventListener('blur', onBlur);
            window.removeEventListener('focus', onFocus);
            if (flashTimeout) clearTimeout(flashTimeout);
            unlistenFns.forEach((fn) => fn());
        };
```

With:

```ts
        return () => {
            unlistenFocus();
            if (flashTimeout) clearTimeout(flashTimeout);
            unlistenFns.forEach((fn) => fn());
        };
```

- [ ] **Step 3: Add fallback — poll `isFocused` on each new_message**

In the `new_message` listener (lines 71-84), add a secondary check as a safety net if `onFocusChanged` doesn't fire in some edge cases. Add before the `if (!isWindowFocused || !isCurrentSession)` check:

```ts
            // Fallback: poll isFocused in case onFocusChanged didn't fire reliably
            try {
                const realFocus = await win.isFocused();
                if (realFocus !== isWindowFocused) {
                    logger.debug('[DEBUG App focus mismatch]', { tracked: isWindowFocused, actual: realFocus });
                    isWindowFocused = realFocus;
                }
            } catch { /* ignore */ }
```

- [ ] **Step 4: Verify svelte-check passes**

```bash
npx svelte-check --tsconfig ./tsconfig.json 2>&1 | Select-Object -Last 5
```

Expected: 0 errors, existing warnings acceptable.

- [ ] **Step 5: Commit**

```bash
git add src/App.svelte
git commit -m "fix: use onFocusChanged for taskbar flash detection, with isFocused fallback"
```

---

### Task 10: Frontend — preserve `last_message_preview` in `loadSessions()`

**Files:**
- Modify: `src/lib/stores/sessionStore.svelte.ts:12-18`
- Modify: `src/lib/components/ChatView.svelte` — add preview update after `loadMessages`

- [ ] **Step 1: Add preview preservation to `loadSessions()`**

In `sessionStore.svelte.ts`, replace lines 12-18:

```ts
            // 保留已有的 unread_count，因为后端不维护实时未读计数
            const existingUnread = new Map(this.sessions.map(s => [s.id, s.unread_count]));
            this.sessions = fresh
                .map(s => ({
                    ...s,
                    unread_count: existingUnread.get(s.id) ?? s.unread_count,
                }))
```

With:

```ts
            // 保留已有的 unread_count 和 last_message_preview（后端已不再维护预览）
            const existingUnread = new Map(this.sessions.map(s => [s.id, s.unread_count]));
            const existingPreview = new Map(
                this.sessions
                    .filter(s => s.last_message_preview)
                    .map(s => [s.id, { preview: s.last_message_preview, time: s.last_message_at }])
            );
            this.sessions = fresh
                .map(s => {
                    const cached = existingPreview.get(s.id);
                    return {
                        ...s,
                        unread_count: existingUnread.get(s.id) ?? s.unread_count,
                        last_message_preview: cached?.preview ?? s.last_message_preview ?? null,
                        last_message_at: cached?.time ?? s.last_message_at ?? null,
                    };
                })
```

- [ ] **Step 2: Add preview update in ChatView after `loadMessages`**

In `ChatView.svelte`, find the `handleSend` function and locate where `messageStore.loadMessages()` is called in chat mode (around line 341). Also in `onMount` (around line 156). After the `await messageStore.loadMessages(...)` call completes, add:

For chat mode `handleSend` (after line ~341):
```ts
                    await messageStore.loadMessages(sessionId, pageIdx);
                    // Update session list preview from last loaded message
                    const msgs = messageStore.messages;
                    if (msgs.length > 0) {
                        const last = msgs[msgs.length - 1];
                        sessionStore.updateSessionPreview(sessionId, last.content, last.created_at);
                    }
```

For `onMount` navigation (after line ~156):
```ts
                messageStore.loadMessages(id, pageIdx);
                // Preview will be updated by new_message events; also derive from loaded messages
                const msgs = await messageStore.loadMessages(id, pageIdx);
                if (msgs && msgs.length > 0) {
                    const last = msgs[msgs.length - 1];
                    sessionStore.updateSessionPreview(id, last.content, last.created_at);
                }
```

Wait — the `onMount` line 156 is in a reactive context and might already handle this via events. Let's keep it simple: only add to `handleSend` chat mode branch, since that's where the user sends a message and expects immediate preview update.

Actually, let me check the exact ChatView code to place this correctly.

- [ ] **Step 3: Read ChatView.svelte around lines 330-345 and 150-160 to confirm exact insertion points**

Read the actual code to determine the correct insertion locations before making edits.

- [ ] **Step 4: Make the ChatView edits**

Based on actual code read in Step 3, add preview updates after `loadMessages` calls.

- [ ] **Step 5: Verify svelte-check passes**

```bash
npx svelte-check --tsconfig ./tsconfig.json 2>&1 | Select-Object -Last 5
```

Expected: 0 errors.

- [ ] **Step 6: Commit**

```bash
git add src/lib/stores/sessionStore.svelte.ts src/lib/components/ChatView.svelte
git commit -m "feat: frontend maintains last_message_preview via events + memory preservation"
```

---

## Verification Checklist

After all tasks are complete:

1. `cd src-tauri; cargo check 2>&1` — 0 errors
2. `npx svelte-check --tsconfig ./tsconfig.json` — 0 errors
3. `cd src-tauri; cargo test --no-run` — tests compile
4. Manual: switch focus away from app → trigger proactive message → verify taskbar flashes green
5. Manual: proactive message → verify other agents in same session respond
6. Manual: click session after proactive message → verify `last_message_preview` stays correct
