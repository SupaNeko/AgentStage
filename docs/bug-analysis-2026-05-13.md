# Bug Root Cause Analysis Report

**Date:** 2026-05-13
**Scope:** Session Inbox Architecture Refactor + Message Limit Reset Feature
**Method:** Systematic Debugging (Phase 1-3)

---

## Bug 1: Resetting Group Chat While Agents Are Responding Causes Continued Output and Triggering

### Phase 1: Root Cause Investigation

#### Reproduction Steps
1. User sends a message in a group chat.
2. Agent A begins responding (LLM call in progress, `is_triggering=1`).
3. User clicks "Reset Session" (`reset_session` command invoked).
4. `reset_session` creates a new `chat_page` (increments `page_index`), updates `current_chat_page`, clears `agent_unread_queue`, and removes `session_frozen_states`.
5. Agent A's in-flight LLM call completes.
6. Agent A's response is inserted into the **new** page (because `current_chat_page` has been updated).
7. `distribute_message` pushes Agent A's message into other agents' unread queues.
8. Other agents are triggered, continuing the conversation chain on the **new** page.

#### Data Flow Analysis
```
User Message
  → send_user_message()
    → insert_message(page_index=OLD) ✓
    → scheduler.on_new_message()
      → distribute_message() → unread queue ✓
      → try_trigger_agent("A")
        → trigger_agent("A")
          → is_triggering=1 ✓
          → emit("agent_typing")
          → trigger_agent_inner() [LLM CALL STARTS]
            
            [USER CLICKS RESET SESSION]
            → reset_session()
              → INSERT new chat_page (page_index=NEW)
              → UPDATE current_chat_page=NEW
              → DELETE FROM agent_unread_queue
              → DELETE FROM session_frozen_states
              → [DOES NOT cancel in-flight LLM]
              → [DOES NOT clear scheduler memory state]
            
            [LLM CALL COMPLETES]
            → ToolExecutor.execute_send_message()
              → insert_message(page_index=NEW) ← WRONG PAGE!
            → trigger_agent_inner() Stage 7
              → distribute_message() → pushes to unread queues
              → try_trigger_agent("B", "C") → triggers continue
          → emit("agent_completed")
```

#### Key Evidence
- `reset_session` in `src-tauri/src/db/session.rs` (lines 286-331) only manipulates DB state. It does **not**:
  - Cancel in-flight LLM calls.
  - Clear `Scheduler.unread_messages` (in-memory HashMap).
  - Clear `Scheduler.agent_notifications` (in-memory HashMap).
- `insert_message` in `src-tauri/src/db/message.rs` (lines 24-53) always reads `current_chat_page` from the DB at call time. An in-flight LLM call will therefore use the **new** page index after reset.
- `trigger_agent_inner` Stage 7 in `src-tauri/src/scheduler/mod.rs` (lines 649-652) calls `distribute_message` for each generated message, which can re-trigger the conversation chain on the new page.

### Phase 2: Pattern Analysis

| Expected Behavior | Actual Behavior |
|-------------------|-----------------|
| Reset creates a completely isolated new page. Old in-flight responses should be discarded or directed to the old page history. No further triggering should occur on the new page from old LLM calls. | Old LLM calls complete and insert messages into the **new** page. `distribute_message` then pushes these messages into unread queues, causing a trigger chain on the new page. |

The expected pattern is "snapshot isolation": a reset should snapshot the old conversation, cancel pending work, and start a fresh state. The actual pattern violates isolation because pending async work (LLM calls) is not bound to a specific page version.

### Phase 3: Hypothesis

**Root Cause:** `reset_session` lacks concurrency control for in-flight LLM calls and does not clean up the Scheduler's in-memory state (`unread_messages`, `agent_notifications`).

Specifically:
1. **No cancellation mechanism:** There is no way to abort an ongoing `trigger_agent_inner` LLM call when reset occurs.
2. **No memory state cleanup:** `reset_session` clears DB tables (`agent_unread_queue`, `session_frozen_states`) but leaves `Scheduler.unread_messages` and `Scheduler.agent_notifications` untouched. Old unread messages and notifications remain in memory.
3. **Page index race condition:** `insert_message` reads `current_chat_page` at execution time. An LLM call started before reset will use the post-reset page index, causing old responses to leak into the new page.

---

## Bug 2: Clicking "Reset Limit" After Message Limit Reached Does NOT Continue Triggering Conversation

### Phase 1: Root Cause Investigation

#### Reproduction Steps
1. Group chat reaches message limit.
2. `check_and_freeze_if_needed` freezes the session (`frozen_sessions` + DB `session_frozen_states`).
3. User clicks "Reset Limit" (`reset_message_count` command invoked).
4. `reset_message_count` resets DB counter, unfreezes session.
5. `get_agents_with_unread` queries `agent_unread_queue` for agents with unread messages in this session.
6. `try_trigger_agent` is called for each agent found.
7. **Result:** No agents are triggered. Conversation does not continue.

#### Data Flow Analysis

**Path A: How messages normally flow after an agent responds**
```
Agent A responds (Stage 6)
  → agent_message_count += 1
  → check_and_freeze_if_needed() → FREEZES session if limit reached
  
Agent A Stage 7:
  → distribute_message(session_id, msg, agent_id)
    → if frozen_sessions.contains(session_id): return Ok(()) ← BLOCKED!
    → [Messages are NOT pushed to other agents' unread queues]
```

**Path B: What reset_message_count does**
```
reset_message_count(session_id)
  → reset DB counter
  → unfreeze session (DB + memory)
  → agents = get_agents_with_unread(session_id)
    → SELECT DISTINCT agent_id FROM agent_unread_queue WHERE session_id = ?
    → Returns [] (because Path A blocked distribution)
  → for agent in agents: try_trigger_agent(agent)
    → Nothing to trigger
```

#### Key Evidence
- `trigger_agent_inner` Stage 6 (`src-tauri/src/scheduler/mod.rs`, lines 599-639) increments `agent_message_count` and calls `check_and_freeze_if_needed` **before** Stage 7 distribution.
- `check_and_freeze_if_needed` (lines 222-254) adds the session to `frozen_sessions` **immediately** when the limit is reached.
- `distribute_message` (lines 186-220) checks `frozen_sessions` at the very beginning and returns early if frozen.
- Therefore, the message that causes the limit to be reached is **never distributed** to other agents' unread queues.
- When `reset_message_count` later queries `agent_unread_queue`, it finds nothing for that session.

### Phase 2: Pattern Analysis

| Expected Behavior | Actual Behavior |
|-------------------|-----------------|
| When limit is reached, the triggering agent's final message should still be distributed to other agents' unread queues. After reset, those queued messages should trigger the next agents in the chain. | The final message that hits the limit is blocked by `distribute_message` because the session is already frozen by Stage 6. Other agents never receive it in their unread queues. Reset finds nothing to trigger. |

The expected pattern is "distribute-then-freeze": the message that triggers the limit should still complete its lifecycle (distribution) before the session is frozen for *future* messages. The actual pattern is "freeze-then-distribute," which blocks the final message's distribution.

### Phase 3: Hypothesis

**Root Cause:** `check_and_freeze_if_needed` is called in Stage 6 (before distribution in Stage 7), and `distribute_message` checks the frozen state at its entry point. This creates a race where the session is frozen **before** the triggering message can be distributed to other agents.

Specifically:
1. **Freeze happens too early:** Stage 6 increments the counter and freezes the session **before** Stage 7 distributes the message.
2. **Distribution blocked by frozen check:** `distribute_message` sees the session is already frozen and returns early, preventing the message from entering other agents' unread queues.
3. **Reset has nothing to work with:** `reset_message_count` relies on `agent_unread_queue` to find agents to trigger. Since the queue is empty for this session, no agents are triggered after reset.

**Correct pattern:** Distribution should not be blocked by the frozen state. Freezing should only prevent **new** messages from entering the queue, not block messages that are already being processed. Alternatively, the freeze check should happen **after** distribution (i.e., in Stage 7 or via a post-distribution hook), not before.

---

## Bug 3: Private Chat Shows Stuck Typing Indicator + Group Chat Shows Phantom "Agent" Typing Indicator

### Phase 1: Root Cause Investigation

#### Reproduction Steps
1. Group chat previously reached limit and was frozen.
2. User clicks "Reset Limit" (`reset_message_count`).
3. User sends a message in a **private** chat to Agent A.
4. Agent A responds normally in the private chat.
5. Private chat shows "正在输入中..." and gets **stuck**.
6. Simultaneously, group chat shows a typing indicator from a sender named **"Agent"** and also gets **stuck**.

#### Data Flow Analysis

**Typing Indicator State Machine (Frontend)**
```
ChatView.svelte:
  isAgentTyping = $state(false)
  currentAgentId = $state<string | undefined>(undefined)

  listen('agent_typing', (event) => {
      if (currentAgentId === payload.agent_id) {
          isAgentTyping = true;  // SET
      }
  });

  listen('agent_completed', (event) => {
      if (currentAgentId === payload.agent_id) {
          isAgentTyping = false; // CLEAR
      }
  });
```

**Scenario: Cross-session trigger + session switch**
```
Step 1: reset_message_count(group_session_id)
  → unfreeze group session
  → agents_with_unread = get_agents_with_unread(group_session_id)
    → If Agent A has unread messages in the group chat,
       try_trigger_agent("A") is called.
  → emit("agent_typing", { agent_id: "A" })
    → Private chat: currentAgentId === "A" → isAgentTyping = true
    → Group chat: currentAgentId === undefined → no effect (yet)

Step 2: User sends message in private chat
  → on_new_message(private_session_id, msg)
  → try_trigger_agent("A")
    → A is already is_triggering=1 (from Step 1) → SKIP
    → OR: A is triggered if is_triggering=0

Step 3: Agent A completes its response
  → emit("agent_completed", { agent_id: "A" })
    → If user is STILL in private chat: isAgentTyping = false ✓
    → BUT if user SWITCHED to group chat:
         currentAgentId is now undefined
         currentAgentId (undefined) !== "A"
         → isAgentTyping is NOT cleared! ← STUCK

Step 4: User is now in group chat
  → isAgentTyping is still true (never cleared)
  → selectedSession.agent_name is undefined (group chat)
  → UI shows: {selectedSession.agent_name || 'Agent'} = "Agent"
  → Typing indicator displays "Agent" and is stuck
```

#### Key Evidence
- `ChatView.svelte` (`src/lib/components/ChatView.svelte`):
  - `isAgentTyping` is a component-level `$state` that is **never reset** when `selectedSessionId` changes (lines 15, 179-193, 288-306).
  - `agent_completed` only clears typing if `currentAgentId === payload.agent_id` (line 190). If the user has switched to a different session, `currentAgentId` no longer matches, and the typing state is orphaned.
  - The typing indicator UI uses `selectedSession.agent_name || 'Agent'` (line 299). In a group chat, `agent_name` is `undefined`, so it falls back to the hardcoded string `"Agent"`.
- `reset_message_count` in `src-tauri/src/commands/session.rs` (lines 112-139) calls `try_trigger_agent` for agents with unread messages in the **group** session. This can trigger Agent A for the group chat even while the user is interacting with Agent A in a private chat.
- `try_trigger_agent` in `src-tauri/src/scheduler/mod.rs` (lines 296-358) checks `is_triggering` but does **not** scope the check to a specific session. Agent A can be triggered for the group chat while already processing the private chat (if `is_triggering` was cleared between the two calls, or if the two triggers happen concurrently before `is_triggering` is set).

### Phase 2: Pattern Analysis

| Expected Behavior | Actual Behavior |
|-------------------|-----------------|
| Typing indicator should be session-scoped. Switching sessions should reset the typing state. The indicator should only show for the current session. `agent_completed` should reliably clear the indicator regardless of which session is active when it arrives. | Typing indicator is global to the `ChatView` component. Switching sessions does not reset it. `agent_completed` only clears if `currentAgentId` matches at arrival time, which fails if the user switched. Group chat shows fallback name "Agent" because `agent_name` is undefined. |

The expected pattern is "session-local state": each session maintains its own typing indicator state. The actual pattern is "global component state": one `isAgentTyping` flag shared across all sessions.

### Phase 3: Hypothesis

**Root Cause:** The typing indicator is implemented as a single global boolean (`isAgentTyping`) rather than a session-scoped state. Combined with the fact that `agent_typing` / `agent_completed` events are global and `reset_message_count` can trigger agents across sessions, this causes orphaned typing states.

Specifically:
1. **No session switch reset:** When `selectedSessionId` changes, `isAgentTyping` is never reset to `false`. If it was `true` in the previous session, it remains `true` in the new session.
2. **`agent_completed` is arrival-time sensitive:** The clear condition `currentAgentId === payload.agent_id` fails if the user switches sessions between `agent_typing` and `agent_completed`. The event is lost for state cleanup purposes.
3. **Group chat typing indicator uses wrong name source:** `selectedSession.agent_name` is `undefined` for group chats. The fallback `"Agent"` is a generic placeholder, not the actual agent's name.
4. **Cross-session triggering via `reset_message_count`:** Resetting the group chat can trigger Agent A while the user is in a private chat with Agent A. The `agent_typing` event emitted for the group chat is incorrectly handled by the private chat UI (because `currentAgentId` matches).

---

## Summary Table

| Bug | Root Cause | Primary File(s) |
|-----|-----------|-----------------|
| **Bug 1:** Reset during active response leaks old LLM output to new page | `reset_session` does not cancel in-flight LLM calls or clear Scheduler memory state (`unread_messages`, `agent_notifications`). `insert_message` reads `current_chat_page` at execution time, causing a race. | `src-tauri/src/db/session.rs` (reset_session)<br>`src-tauri/src/scheduler/mod.rs` (memory state)<br>`src-tauri/src/db/message.rs` (insert_message) |
| **Bug 2:** Reset limit does not continue conversation | `check_and_freeze_if_needed` (Stage 6) freezes the session **before** `distribute_message` (Stage 7). The final message that hits the limit is blocked from entering other agents' unread queues. Reset finds empty queues. | `src-tauri/src/scheduler/mod.rs` (Stage 6/7 ordering)<br>`src-tauri/src/scheduler/mod.rs` (distribute_message frozen check) |
| **Bug 3:** Stuck typing indicator + phantom "Agent" | `isAgentTyping` is a global component state, not session-scoped. Session switches do not reset it. `agent_completed` clear condition fails if user switches sessions. Group chat typing UI uses `selectedSession.agent_name` which is undefined. | `src/lib/components/ChatView.svelte` (typing state)<br>`src-tauri/src/commands/session.rs` (reset_message_count cross-trigger) |

---

## Phase 4: Repair Strategy

> **Design principle:** Fixes must align with the existing Session Inbox architecture. Where a symptom-level patch would create future technical debt, an architectural change is proposed for discussion.

---

### Bug 1: Reset During Active Response

#### Recommended Fix (Lightweight — Minimal Intrusion)

Introduce a **session-scoped cancellation token** in the Scheduler. Rather than aborting the in-flight LLM call (which would require deep changes to the async stack), we let the call finish but intercept its output before it can leak into the new page.

**Step-by-step:**

1. **Scheduler state extension** (`src-tauri/src/scheduler/mod.rs`):
   ```rust
   canceled_sessions: Arc<Mutex<HashMap<String, i64>>>, // session_id → cancel_timestamp
   ```

2. **`Scheduler::cancel_session`** (new method):
   - Insert `session_id` into `canceled_sessions` with current timestamp.
   - Remove all entries for this session from `unread_messages`.
   - Remove this session from every agent's `agent_notifications` set (and clean up empty sets).
   - This ensures no stale memory state survives the reset.

3. **`commands/session.rs::reset_session`**:
   - Add `scheduler: State<'_, Scheduler>` parameter.
   - After `session_repo::reset_session()` succeeds, call `scheduler.cancel_session(&req.session_id).await`.

4. **Capture old page index in `trigger_agent`**:
   - Immediately after collecting `session_ids` from `agent_notifications`, read each session's `current_chat_page` from the DB into a `HashMap<String, i32>` called `session_pages`.
   - Pass `session_pages` into `trigger_agent_inner`.

5. **Interception in `trigger_agent_inner` Stage 7**:
   ```rust
   for msg in &agent_messages {
       if self.canceled_sessions.lock().await.contains_key(&msg.session_id) {
           // Move the message back to the old page so it lives in history, not the new page
           if let Some(&old_page) = session_pages.get(&msg.session_id) {
               let conn = self.db_state.0.lock().await;
               let _ = conn.execute(
                   "UPDATE messages SET page_index = ?1 WHERE id = ?2",
                   (old_page, &msg.id),
               );
           }
           continue; // Skip emit + distribute — no trigger chain, no UI pollution
       }
       self.emit("new_message", msg);
       self.distribute_message(&msg.session_id, msg, agent_id).await?;
   }
   ```

6. **Cleanup background task**:
   - In `start_background_scan`, every N ticks call `cleanup_canceled_sessions()` which removes entries older than 10 minutes.

**Why this works:**
- The LLM call is allowed to complete naturally (no risky async cancellation).
- The message is **archived to the old page** rather than deleted, satisfying the user's expectation that old output goes to old history.
- Because `emit` and `distribute_message` are skipped, the new page stays clean and no trigger chain continues.
- Memory state (`unread_messages`, `agent_notifications`) is purged, so the background scanner cannot resurrect stale triggers.

#### Confirmed Final Fix (User-Proposed: Bind Page Index + Scheduler Cleanup on Reset)

**Status:** ✅ Final architecture confirmed. Ready for TDD implementation.

After discussion, the team agreed on a **two-pronged fix**:

1. **Bind `page_index` to the trigger lifecycle** (replaces `canceled_sessions` token).
2. **On `reset_session`, explicitly purge the Scheduler's in-memory state** for the reset session (`unread_messages` + `agent_notifications`).

**Why both are needed:**
- Page-index binding ensures in-flight LLM responses land in the correct historical page and do not pollute the new page.
- Scheduler memory cleanup ensures the background scanner does not resurrect stale triggers after reset.

**Core idea:** When `trigger_agent` starts, it captures each session's current `page_index` (the "stage number"). This `page_index` is passed through the entire call chain (`trigger_agent_inner` → `ToolExecutor` → `insert_message`). The message is always written to that bound page. After the LLM call returns, Stage 7 checks: *"Does the message's page_index still match the session's current page?"* If not, the session was reset — the message stays in its original page history but is **not** shown to the frontend and **not** distributed to other agents.

**Why page-index binding is better than `canceled_sessions`:**
- No need for a separate cancellation HashMap and TTL cleanup.
- No need for a post-hoc `UPDATE messages SET page_index = ...` to move messages back.
- The message lands in the correct historical page **on the first write**.
- The check in Stage 7 is a simple integer comparison (`msg.page_index != current_page`).

**Step-by-step:**

1. **`models/message.rs`**: Add `page_index: i32` to `Message` struct.
2. **`db/message.rs`**:
   - `insert_message` gains an optional `page_index: Option<i32>` parameter.
   - If `Some(page_index)` is provided, use it directly; if `None`, fall back to reading `current_chat_page` from DB (preserving compatibility for direct calls like `send_user_message`).
   - `row_to_message` populates `page_index` from the query result.
3. **`llm/tool.rs`**:
   - `ToolExecutor` receives `session_pages: HashMap<String, i32>` (captured at trigger start).
   - `execute_send_message` looks up the bound `page_index` for `target_id` and passes it to `insert_message`.
4. **`scheduler/mod.rs`**:
   - `trigger_agent` reads each session's `current_chat_page` into `session_pages: HashMap<String, i32>` before entering `trigger_agent_inner`.
   - `trigger_agent_inner` accepts `session_pages` and passes it to `ToolExecutor`.
   - Stage 7 checks:
      ```rust
      let current_page: i32 = /* read current_chat_page from DB for msg.session_id */;
      if msg.page_index != current_page {
          // Session was reset. Skip emit + distribute.
          continue;
      }
      self.emit("new_message", msg);
      self.distribute_message(&msg.session_id, msg, agent_id).await?;
      ```
5. **`scheduler/mod.rs` — `cancel_session`** (new method):
   - Remove all entries for this session from `unread_messages`.
   - Remove this session from every agent's `agent_notifications` set (and clean up empty sets).
6. **`commands/session.rs::reset_session`**:
   - Add `scheduler: State<'_, Scheduler>` parameter.
   - After `session_repo::reset_session()` succeeds, call `scheduler.cancel_session(&req.session_id).await`.

---

#### Architectural Discussion: Can the Scheduler Be Pure DB?

**Current Scheduler responsibilities:**
| Component | Purpose | DB Equivalent |
|-----------|---------|---------------|
| `unread_messages` (memory) | Cache of pending messages per (session, agent) | `agent_unread_queue` table + `messages` join |
| `agent_notifications` (memory) | Which agents have which sessions pending | `SELECT DISTINCT session_id FROM agent_unread_queue WHERE agent_id = ?` |
| `frozen_sessions` (memory) | Sessions paused for limit | `session_frozen_states` table |
| `is_triggering` (DB) | Prevents concurrent triggers per agent | `trigger_states.is_triggering` |
| `start_background_scan` | 5-second poll for triggerable agents | `SELECT DISTINCT agent_id FROM agent_unread_queue` every 5s |

**Verdict:** Yes, it *can* be pure DB, but it is a **large refactor** with the following trade-offs:

| Aspect | Memory + DB (Current) | Pure DB |
|--------|----------------------|---------|
| Trigger latency | O(1) memory ops | O(N) DB query per agent |
| Background scan | O(1) memory key iteration | `SELECT DISTINCT agent_id` every 5s (acceptable for desktop) |
| Reset consistency | Requires explicit memory cleanup | Single-source-of-truth; no memory sync needed |
| Code complexity | Higher (two state layers) | Lower (one state layer) |
| Risk for this fix | Low (add `cancel_session`) | High (rewrite Scheduler core) |

**Recommendation for now:** Keep the hybrid architecture. The fix only requires:
1. Adding `page_index` binding (the user-proposed approach).
2. Adding a `scheduler.cancel_session()` call inside `reset_session` to purge memory caches.

A pure-DB Scheduler is a good **future refactor** if we continue to hit state-synchronization bugs, but it is overkill for resolving this specific Bug 1.

---

### Bug 2: Reset Limit Does Not Continue Conversation

**Status:** ✅ Repair strategy confirmed. Ready for implementation.

#### Recommended Fix (Semantic Clarification + One-line Change)

The fix is to **remove the frozen check from `distribute_message`** and rely on the frozen check in `trigger_agent` to pause the conversation.

**Step-by-step:**

1. **`src-tauri/src/scheduler/mod.rs::distribute_message`** (lines 186-220):
   - Remove or comment out the early-return frozen check at the top:
     ```rust
     // REMOVED: if self.frozen_sessions.lock().await.contains(session_id) { return Ok(()); }
     ```
    - `distribute_message` now always pushes messages into unread queues and writes to `agent_unread_queue`.

2. **Keep frozen guards in the right places**:
   - `trigger_agent` (lines 379-399): Already skips frozen sessions when draining unread queues. **Keep this.**
   - `on_new_message` (lines 261-294): Already auto-unfreezes on user messages. **Keep this.**

**Why this works:**
- `distribute_message`'s sole responsibility becomes "make this message available to other agents." Freezing is about **pausing automatic triggers**, not about censoring messages.
- When the limit is hit, Stage 6 still freezes the session. Stage 7 now successfully distributes the final message to other agents' queues.
- Those agents **do not trigger immediately** because `trigger_agent` skips frozen sessions.
- When the user clicks **Reset Limit**, `reset_message_count` unfreezes the session and calls `get_agents_with_unread`. The queues now contain the final message, so agents are found and triggered. The conversation chain resumes exactly where it left off.

**Risk assessment:**
- Could unread queues grow unbounded while frozen? Yes, but only if agents keep sending messages to a frozen session. In practice, once a session is frozen, `trigger_agent` prevents further agent triggers for that session, so the only new messages are the ones currently in flight (one round per agent). This is bounded and acceptable.
- User messages auto-unfreeze, so the user always has an escape hatch.

#### Architectural Discussion (Long-term)

This fix clarifies a subtle semantic ambiguity in the Session Inbox architecture:

| Layer | Responsibility | Frozen Meaning |
|-------|---------------|----------------|
| `distribute_message` | Message enqueue (inbox) | Should **not** be frozen. Messages always land in the inbox. |
| `trigger_agent` | Message dequeue + trigger | **Should** be frozen. Agents do not read from frozen sessions. |
| `on_new_message(user)` | User-initiated reset | **Overrides** freeze. User action always unfreezes. |

If we later add features like "mute session" (where messages are silently dropped), that should be a **separate** flag (e.g., `drop_incoming`), not an overload of `frozen`. Keeping these semantics distinct prevents the kind of cascade failure we see in Bug 2.

---

### Bug 3: Stuck Typing Indicator + Phantom "Agent"

**Status:** ✅ Repair strategy confirmed with updated group-chat UX design.

#### Recommended Fix (Frontend State Restructure + Member-List Typing Icons)

Replace the global `isAgentTyping` boolean with a **global set of typing agents**, and derive the per-session indicator from it.

**Core principle:** In **private chats**, typing is shown inside the message stream (1-to-1). In **group chats**, typing is shown as a small icon/badge on the member list (right sidebar) rather than inside the message stream. This avoids message-order misalignment: an agent may finish typing and its message arrives out of visual order with other agents' messages.

**Step-by-step:**

1. **`src/lib/components/ChatView.svelte`**:
   Replace:
   ```svelte
   let isAgentTyping = $state(false);
   ```
   With:
   ```svelte
   let typingAgents = $state<Set<string>>(new Set());
   let typingTimeouts = $state<Map<string, number>>(new Map());

   // Private chat: show in message stream
   let isAgentTyping = $derived(
       currentAgentId != null && typingAgents.has(currentAgentId)
   );

   // Group chat: derive which members are typing for sidebar icons
   let typingMemberIds = $derived(
       selectedSession?.session_type === 'group'
           ? members.filter(m => m.participant_type === 'agent' && typingAgents.has(m.participant_id))
           : []
   );
   ```

2. **Event listeners**:
   ```svelte
   listen('agent_typing', (event) => {
       const payload = event.payload as { agent_id?: string };
       if (payload.agent_id) {
           typingAgents = new Set(typingAgents).add(payload.agent_id);
           // Defense: 5-minute timeout in case agent_completed is lost
           const existing = typingTimeouts.get(payload.agent_id);
           if (existing) clearTimeout(existing);
           const t = setTimeout(() => {
               const next = new Set(typingAgents);
               next.delete(payload.agent_id);
               typingAgents = next;
               typingTimeouts.delete(payload.agent_id);
           }, 5 * 60 * 1000);
           typingTimeouts = new Map(typingTimeouts).set(payload.agent_id, t);
       }
   });

   listen('agent_completed', (event) => {
       const payload = event.payload as { agent_id?: string };
       if (payload.agent_id) {
           const next = new Set(typingAgents);
           next.delete(payload.agent_id);
           typingAgents = next;
           const t = typingTimeouts.get(payload.agent_id);
           if (t) { clearTimeout(t); typingTimeouts.delete(payload.agent_id); }
       }
   });

   listen('agent_error', (event) => {
       const payload = event.payload as { agent_id?: string };
       if (payload.agent_id) {
           const next = new Set(typingAgents);
           next.delete(payload.agent_id);
           typingAgents = next;
           const t = typingTimeouts.get(payload.agent_id);
           if (t) { clearTimeout(t); typingTimeouts.delete(payload.agent_id); }
       }
   });
   ```

3. **Member list typing indicator (Group Chat Sidebar)**:
   In the group member list (`<aside>`), add a small animated dot or "输入中..." label next to agents that are in `typingAgents`:
   ```svelte
   {#each members as member}
       <div class="flex items-center gap-2 p-2 rounded-lg hover:bg-bg">
           <div class="w-8 h-8 rounded-full bg-primary/10 flex items-center justify-center text-primary shrink-0 overflow-hidden relative">
               {#if member.avatar_path}
                   <img src={member.avatar_path} alt={member.name} class="w-full h-full object-cover" />
               {:else}
                   <User size={16} />
               {/if}
               {#if typingAgents.has(member.participant_id)}
                   <span class="absolute bottom-0 right-0 w-2.5 h-2.5 bg-green-500 rounded-full border-2 border-surface animate-pulse" />
               {/if}
           </div>
           <span class="text-sm truncate">{member.name}</span>
           {#if typingAgents.has(member.participant_id)}
               <span class="text-xs text-text-secondary ml-auto">输入中...</span>
           {/if}
       </div>
   {/each}
   ```

4. **Private chat message stream**: Keep the existing typing bubble, but bind it to `isAgentTyping` (which is now `$derived` from `currentAgentId`). Remove the fallback name `"Agent"` because `currentAgentId` being `null` already prevents rendering.

5. **Cleanup on unmount**:
   ```svelte
   onMount(() => {
       // ... existing listeners ...
       return () => {
           unlistenFns.forEach((fn) => fn());
           typingTimeouts.forEach((t) => clearTimeout(t));
       };
   });
   ```

**Why this works:**
- `typingAgents` is global (correctly reflecting that typing is an agent-level activity), but `isAgentTyping` is **session-local** by derivation.
- When the user switches from private chat (Agent A) to group chat (`currentAgentId = undefined`), `isAgentTyping` instantly becomes `false`. No phantom typing bubble appears in the message stream.
- In group chat, the member list shows who is typing without polluting the chronological message order. If Agent A finishes typing but Agent B's earlier message arrives first, the stream remains correctly ordered.
- If `agent_completed` is missed, the 5-minute frontend timeout guarantees the indicator does not stick forever.

#### Optional Backend Improvement

In `src-tauri/src/scheduler/mod.rs::try_trigger_agent`, the stale-trigger recovery branch (lines 318-327) currently resets `is_triggering` but does **not** emit `agent_completed`. This can leave the frontend indicator dangling if the backend panics or hangs.

**Suggested addition:**
```rust
if now - updated_at > 5 * 60 * 1000 {
    conn.execute("UPDATE trigger_states SET is_triggering = 0 ...");
    self.emit("agent_completed", serde_json::json!({"agent_id": agent_id}));
    // Continue to trigger below...
}
```

This makes the backend's self-healing behavior visible to the frontend, reducing reliance on the frontend timeout.

#### Architectural Discussion (Long-term)

The current event design (`agent_typing` / `agent_completed` scoped only to `agent_id`) assumes a 1-to-1 mapping between the visible session and the typing agent. This breaks down in group chats where multiple agents may type simultaneously.

**Proposed evolution:** Extend the event payload with `session_id`.

```rust
self.emit("agent_typing", serde_json::json!({"agent_id": agent_id, "session_id": session_id}));
```

This would allow the frontend to:
- Show multiple typing indicators in a group chat (one per agent).
- Scope typing state precisely to `(session_id, agent_id)` pairs.
- Eliminate any ambiguity when the same agent is active in multiple sessions.

This is **not required** for the current fix but should be considered if group-chat UX is deepened (e.g., per-agent typing bubbles).

---

## Fix Summary Table

| Bug | Fix Approach | Files to Modify | Architectural Change? |
|-----|-------------|-----------------|----------------------|
| **Bug 1** | Scheduler `canceled_sessions` + old-page redirection | `scheduler/mod.rs`, `commands/session.rs` | **Lightweight** (recommended). Long-term: Trigger Context with bound `page_index`. |
| **Bug 2** | Remove frozen check from `distribute_message`; keep it in `trigger_agent` | `scheduler/mod.rs` | **Semantic clarification** — no new state, just correct layer separation. |
| **Bug 3** | Global `typingAgents` Set + derived `isAgentTyping` + frontend timeout | `ChatView.svelte` | **Lightweight**. Long-term: add `session_id` to typing events for multi-agent group typing. |

## Supporting Logs / Screenshots
- `e2e/私聊卡住.png` — Private chat stuck with typing indicator
- `e2e/群聊卡住.png` — Group chat stuck with "Agent" typing indicator
- Backend logs: `data/logs/backend.log`
- Frontend logs: `data/logs/frontend.log`
