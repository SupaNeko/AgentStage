# Unified Message Processing — Design Doc

**Date:** 2026-05-24  
**Status:** DRAFT  
**Problem:** 3 bugs rooted in fragmented message post-processing across two execution paths (`trigger_agent_inner` vs `trigger_special`), plus `last_message_preview` maintained as a backend-authoritative field despite the frontend having live message data.

---

## 1. Goals

1. **Bug 1 — Taskbar flashing**: Window focus detection uses `onFocusChanged` (Tauri OS-level event) instead of `window blur/focus` (DOM-level, unreliable in WebView2).
2. **Bug 2 — Wrong `last_message_preview` in session list**: Remove the DB column entirely. Frontend becomes the single source of truth — `new_message` events update memory; `loadSessions()` preserves memory values over DB values.
3. **Bug 3 — Proactive/timer messages don't trigger other agents**: Extract a shared `handle_agent_response()` method that both `trigger_agent_inner` and `trigger_special` call, ensuring all agent messages go through the same post-processing pipeline (emit + distribute + freeze check + counter update).

---

## 2. Architecture: Before vs After

### Before (fragmented)

```
trigger_agent_inner ──► [emit new_message] [distribute] [freeze check] [counter] [preview DB write] [agent_completed]
trigger_special     ──► [emit new_message]  ←── incomplete!
```

### After (unified)

```
handle_agent_response(agent_id, messages) {
    for each msg:
        increment agent_message_count
        emit new_message
        distribute_message → triggers other agents
    for each unique session:
        check_and_freeze_if_needed
    emit agent_completed
}

trigger_agent_inner ──► handle_agent_response()
trigger_special     ──► handle_agent_response()
```

---

## 3. Backend Changes

### 3.1 New shared method: `handle_agent_response`

**File:** `src-tauri/src/scheduler/mod.rs`  
**Location:** Insert after `trigger_agent_inner`, before `trigger_special`

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
        self.emit("agent_completed", serde_json::json!({"agent_id": agent_id}));
        return Ok(());
    }

    // 1. Update agent_message_count for each message's session
    let mut session_ids: HashSet<String> = HashSet::new();
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
        // Verify session still exists
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

### 3.2 Refactor `trigger_agent_inner`

**File:** `src-tauri/src/scheduler/mod.rs`  
**Changes:** Replace lines 905-1001 (stage 6 + stage 7) with a single call:

```rust
// === Stage 6+7: Unified post-processing ===
self.handle_agent_response(agent_id, &agent_messages).await?;
```

Remove:
- `agent_message_count` update loop (lines 910-921)
- `last_message_preview` DB write (lines 923-925)
- `check_and_freeze_if_needed` loop (lines 932-938)
- `new_message` + `distribute_message` loop (lines 958-986)
- `agent_completed` emit (lines 995-998)

### 3.3 Refactor `trigger_special`

**File:** `src-tauri/src/scheduler/mod.rs`  
**Changes:** Replace lines 1244-1247 (the `new_message` emit loop) with:

```rust
// 6. Unified post-processing (emit, distribute, freeze check, counter)
if let Err(e) = scheduler.handle_agent_response(agent_id, &result.messages).await {
    crate::logger::error(&format!(
        "[trigger_special] handle_agent_response failed for agent_id={}: {}", agent_id, e
    ));
}
```

This adds: `distribute_message`, `agent_message_count` update, `check_and_freeze_if_needed`, `agent_completed` emit — all previously missing.

### 3.4 Remove `last_message_preview` entirely (DB + all interfaces)

**Rationale**: The field was a backend "authority" for a purely frontend concern. With the frontend receiving live `new_message` events and having access to full message data, the backend has no business maintaining this value. Removing it eliminates the multi-source-of-truth problem at its root.

**Schema migration (new function in `migration.rs`):**

```sql
ALTER TABLE sessions DROP COLUMN last_message_preview
```

If `DROP COLUMN` is unsupported by the SQLite build, fall back to recreate-table.

**Remove from `SessionResponse` model (`models/session.rs`):**

Delete the `last_message_preview` field from the struct.

**Remove from `list_sessions` (`db/session.rs`):**

- SQL query (line 124): Remove `s.last_message_preview` from SELECT list. All subsequent column indices shift up by 1.
- `build_session_response_from_row` (line 89): Remove `last_message_preview: row.get(3)?`. Column indices shift accordingly.

**Remove `update_session_last_message` function (`db/session.rs` lines 318-325):** Delete entirely.

**Remove all call sites:**
- `commands/message.rs` lines 56-57 (`send_user_message`)
- `scheduler/mod.rs` lines 924-925 (`trigger_agent_inner` — already removed by refactor)
- `db/session.rs` lines 561-564 (`reset_session`)

**Remove `truncate_preview` function (`scheduler/mod.rs` lines 1903-1910):** No longer used anywhere.

---

## 4. Frontend Changes

### 4.1 Taskbar flashing — use `onFocusChanged`

**File:** `src/App.svelte`  
**Lines 28-42:** Replace `window.addEventListener('blur'/'focus')` with:

```ts
let isWindowFocused = true;
const unlistenFocus = await win.onFocusChanged(({ payload: focused }) => {
    isWindowFocused = focused;
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

Register `unlistenFocus` in the cleanup return.

### 4.2 `loadSessions()` — preserve `last_message_preview` from memory

**File:** `src/lib/stores/sessionStore.svelte.ts`  
**Lines 12-18:** Since the API no longer returns `last_message_preview`, `fresh` sessions come with `last_message_preview` as undefined/null. The store must preserve the in-memory value from the previous state:

```ts
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
    .filter(...);
```

Cold start behavior: on first load, all `last_message_preview` values are `null`. Previews populate as `new_message` events arrive (handled by `App.svelte` listener), or when ChatView loads messages for a session.

### 4.3 ChatView — update session preview on message load

**File:** `src/lib/components/ChatView.svelte`

After `messageStore.loadMessages()` completes, update the session preview from the last message in the loaded page:

```ts
const msgs = await messageStore.loadMessages(sessionId, pageIdx);
if (msgs.length > 0) {
    const last = msgs[msgs.length - 1];
    sessionStore.updateSessionPreview(sessionId, last.content, last.created_at);
}
```

This ensures that clicking a session always shows the correct preview, even on cold start (before any `new_message` events arrive).

### 4.4 `App.svelte` `new_message` listener — no changes needed

Lines 44-68 already correctly update `last_message_preview` from events. With `loadSessions()` preserving memory values, event-driven updates are safe from being overwritten.

---

## 5. Data Flow After Changes

```
           ┌────────────────────────────────────────────────────┐
           │             handle_agent_response()               │
           │                                                   │
Agent LLM  │  agent_msg_count++   emit new_message ──────────►│──► App.svelte → session list preview
  produces │  freeze check        distribute_message ─────────►│──► other agents' unread queues
 messages  │  agent_completed                                │
           └────────────────────────────────────────────────────┘
                    ▲                          ▲
                    │                          │
           trigger_agent_inner         trigger_special
           (user/chain-triggered)      (proactive/timer)

Frontend last_message_preview (backend no longer involved):
  Cold start:  null (list_sessions no longer returns preview)
  Click session: ChatView loads messages → updates preview from last message
  Live updates: new_message events → App.svelte listener updates preview
  Reloads:     loadSessions() preserves in-memory previews, DB has none
```

---

## 6. Affected Files Summary

| File | Change |
|------|--------|
| `src-tauri/src/scheduler/mod.rs` | Add `handle_agent_response()`; refactor `trigger_agent_inner` stage 6+7; refactor `trigger_special` post-LLM; remove `truncate_preview` |
| `src-tauri/src/db/schema.rs` | Update DDL: drop `last_message_preview` column |
| `src-tauri/src/db/migration.rs` | Add migration to drop column |
| `src-tauri/src/models/session.rs` | Remove `last_message_preview` from `SessionResponse` struct |
| `src-tauri/src/db/session.rs` | Remove `s.last_message_preview` from `list_sessions` SQL + `build_session_response_from_row`; delete `update_session_last_message`; remove preview clear in `reset_session` |
| `src-tauri/src/commands/message.rs` | Remove `update_session_last_message` call in `send_user_message` |
| `src/App.svelte` | Replace `window blur/focus` with `onFocusChanged` |
| `src/lib/stores/sessionStore.svelte.ts` | Preserve `last_message_preview` in `loadSessions()`; add preview update from loaded messages |
| `src/lib/components/ChatView.svelte` | Update session preview after `loadMessages()` completes |

---

## 7. Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| SQLite `DROP COLUMN` not supported on old versions | Use recreate-table approach; rusqlite bundles recent SQLite |
| `handle_agent_response` may get duplicate messages if both paths overlap | Each path's messages are distinct: `trigger_agent_inner` handles its own `agent_messages`, `trigger_special` handles `result.messages` |
| `onFocusChanged` may not fire in all WebView2 scenarios | Fallback: also poll `win.isFocused()` on each `new_message` event as secondary check |
| Cold start: session list shows empty previews until first interaction | Acceptable — clicking a session loads messages and updates preview immediately; `new_message` events fill remaining sessions over time |

---

## 8. Verification

1. `cargo check` — 0 errors
2. `npx svelte-check` — 0 errors / no new warnings
3. Manual test: switch focus away from window → trigger proactive message → verify taskbar flashes + goes green
4. Manual test: trigger proactive message → verify other agents in same session respond (chain reaction)
5. Manual test: click session after proactive message → verify `last_message_preview` stays correct (doesn't revert to stale value)
