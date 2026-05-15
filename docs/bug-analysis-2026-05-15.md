# Bug Root Cause Analysis Report

**Date:** 2026-05-15
**Scope:** History Session Feature (Page-Level Isolation)
**Method:** Code Review + User Report

---

## Bug 1: Message List Briefly Goes Blank After User Sends Message in History Group Chat

### Phenomenon

In History view (`mode='history'`), when a user sends a message in a **group chat's old page**:
1. Before sending: Message list correctly displays historical messages of that page.
2. **Immediately after sending**: The message list briefly shows **no messages at all** (completely blank).
3. After the agent replies: All messages pop up at once — historical messages + the user's new message + the agent's reply.

### Root Cause Analysis

The current History mode reuses the same message loading and event handling pipeline as the Chat mode, but with ad-hoc `page_index` parameters. This creates a **race condition between `loadMessages` and `new_message` events**:

1. **User sends message** → `handleSend` calls `send_user_message` with `page_index`.
2. **Backend** inserts the message into the old page and calls `scheduler.on_new_message()`.
3. **Frontend** immediately calls `messageStore.loadMessages(sessionId, pageIdx)` after `send_user_message` returns.
4. **Scheduler triggers agents** in the background. Agents may reply quickly.
5. **Agent reply emits `new_message`** event. The event handler in History mode checks:
   ```typescript
   if (msg.session_id === historyStore.selectedSessionId && 
       msg.page_index === historyStore.selectedPageIndex) { ... }
   ```
   If `msg.page_index` is missing, mismatched, or arrives before `loadMessages` completes, the incremental update (`addMessage`) and the full refresh (`loadMessages`) race against each other.

6. **The critical issue**: The `ChatView.svelte` `$effect` that watches `historyStore.selectedPageIndex` can re-run if any reactive dependency changes. Combined with the fact that `loadMessages` sets `messages = result.reverse()` (a full replacement), a mistimed `loadMessages` call — perhaps querying `current_chat_page` instead of the historical `page_index` due to a transient null in `selectedPageIndex` — results in an empty array being rendered.

7. **Why it recovers on agent reply**: The agent's `new_message` event (or a subsequent `loadMessages` trigger) finally loads the correct page, restoring the full message list.

**Fundamental problem**: History mode and Chat mode share the same message dispatch pipeline (`send_user_message` → `scheduler.on_new_message` → `distribute_message` → `try_trigger_agent` → LLM → `new_message` event). The `page_index` parameter is threaded through as an optional add-on rather than a first-class separation of concerns.

### Proposed Fix (Requirement Change)

**Separate History session messaging from Chat session messaging entirely**:

1. **New backend command**: `send_history_message` — dedicated for History mode.
   - Only reads messages from the **current session + specified page**.
   - Does NOT trigger the global Scheduler pipeline.
   - Assembles prompt using only this session's historical messages + the new user message.
   - Sends to LLM, stores the reply in the same session + page.
   - Returns the reply directly to frontend (no `new_message` broadcast).

2. **Frontend History mode**: Uses `send_history_message` instead of `send_user_message`.
   - No reliance on `new_message` events.
   - After sending, frontend simply appends the user message optimistically, awaits the response, and appends the agent reply.
   - No Scheduler involvement = no race conditions, no blank states.

3. **Chat mode remains unchanged**: Continues to use `send_user_message` + Scheduler + `new_message` events.

This architectural separation eliminates the complexity of making the Scheduler page-aware and prevents History-mode messages from interfering with live Chat-mode sessions.

---

## Bug 2: Deleted Group Chat Messages Still Appear in Agent Prompts

### Phenomenon

After a group chat is deleted (soft-deleted via `disband_group` → `sessions.is_deleted = 1`), agents in other sessions still receive messages from that deleted group chat in their `PromptAssembler` Layer 2 context.

### Root Cause Analysis

`PromptAssembler::assemble` Layer 2 queries "other sessions' recent messages" using a JOIN that does **not filter out deleted sessions**:

```sql
-- Current Layer 2 query (simplified)
SELECT m.*, s.session_type
FROM messages m
JOIN sessions s ON m.session_id = s.session_id
WHERE m.session_id != ?1 AND m.sender_id != ?2
  AND m.is_deleted = 0
ORDER BY m.created_at DESC
LIMIT 10
```

The `WHERE` clause filters `messages.is_deleted = 0` but **does NOT check `sessions.is_deleted = 0`**. When a group chat is disbanded:
- `sessions.is_deleted` is set to `1` (soft delete).
- `group_sessions` row may still exist.
- `messages` in that session are NOT deleted (intentionally, to preserve history).
- Therefore, Layer 2 still picks up these messages.

This is a data leakage bug: deleted sessions should not influence live agent contexts.

### Proposed Fix

Add `sessions.is_deleted = 0` filter to all `PromptAssembler` layers that join with `sessions`:

```sql
-- Fixed Layer 2 query
SELECT m.*, s.session_type
FROM messages m
JOIN sessions s ON m.session_id = s.session_id
WHERE m.session_id != ?1 
  AND m.sender_id != ?2
  AND m.is_deleted = 0
  AND s.is_deleted = 0  -- <-- ADD THIS
ORDER BY m.created_at DESC
LIMIT 10
```

Similarly, verify Layer 3 (agent's own session messages) and Layer 4 (all session messages) also filter `s.is_deleted = 0` where applicable.

---

## Additional Requirement: Log Full Prompt Content in Backend

### Description

Currently, backend logs show individual stages of `PromptAssembler` (Layer 1, Layer 2, etc.) but do **not** log the final, complete prompt string that is actually sent to the LLM API. This makes debugging prompt-related issues extremely difficult.

### Proposed Implementation

In `PromptAssembler::assemble`, after all layers are concatenated into `final_prompt`, log the complete prompt at `INFO` level:

```rust
let final_prompt = format!("{}{}{}{}{}", layer1, layer2, layer3, layer4, layer5);

// Add this:
log::info!(
    "[PromptAssembler] Final prompt for agent {} (session={:?}, page={:?}):\n---PROMPT START---\n{}\n---PROMPT END---",
    agent_id, trigger_session_id, trigger_page_index, final_prompt
);
```

**Guidelines**:
- Log at `INFO` level so it is visible without enabling `TRACE`.
- Wrap with clear markers (`---PROMPT START---` / `---PROMPT END---`) for easy extraction.
- Include `agent_id`, `trigger_session_id`, and `trigger_page_index` in the log prefix for correlation.
- Ensure no API keys or other secrets are logged (the prompt content itself is safe; it contains only message text and system instructions).

---

## Summary

| # | Bug/Requirement | Severity | Proposed Action |
|---|-----------------|----------|-----------------|
| 1 | History group chat blank after send | **High** | Architectural separation: create `send_history_message` command, bypass Scheduler for History mode |
| 2 | Deleted group messages in prompts | **High** | Add `sessions.is_deleted = 0` filter to PromptAssembler SQL layers |
| 3 | Log full LLM prompt | **Medium** | Add `log::info!` in `PromptAssembler::assemble` after final concatenation |

---

*Next Step: Update the History Session Design Doc (Section 9) to incorporate the architectural separation requirement and the deleted-session filter.*
