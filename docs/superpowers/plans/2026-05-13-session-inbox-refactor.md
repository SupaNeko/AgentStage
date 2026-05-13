# Session Inbox Refactor Implementation Plan

> **For agentic workers:** Use TDD (test-first) for all tasks. Parallelize non-conflicting work across 2-3 subagents where possible.

**Goal:** Replace `pending_queue` with a per-session-per-agent unread message layer (Session Inbox), implement persistent message-limit freezing, simplify PromptAssembler to a single chronological layer, and extract all prompt templates to a dedicated file.

**Architecture:** 
- Runtime: `unread_messages: HashMap<session_id, HashMap<agent_id, Vec<PendingMessage>>>` + `agent_notifications: HashMap<agent_id, HashSet<session_id>>>` + `frozen_sessions: HashSet<String>`
- Persistence: `session_frozen_states` table + `agent_unread_queue` table (stores only `message_id`, JOINs `messages` on recovery)
- Prompt: Remove Layer 5 (`pending_messages`), merge all messages into a single chronological Layer 4 with a footer note. Extract all templates to `prompt_templates.rs`.

**Tech Stack:** Tauri v2 + Rust + SQLite (rusqlite), Svelte 5

---

## Phase 1: Database Migration V6

**Files:**
- Modify: `src-tauri/src/db/schema.rs`
- Modify: `src-tauri/src/db/migration.rs`

**TDD Steps:**
1. Write failing Rust test: `test_v6_migration_creates_frozen_states_and_unread_queue` — asserts tables exist and have correct schema
2. Run `cargo test` — confirm failure (tables missing)
3. Add `MIGRATION_V6` with `session_frozen_states` and `agent_unread_queue` tables + indexes
4. Register V6 in `migration.rs`
5. Run `cargo test` — confirm pass
6. Commit

---

## Phase 2: Backend Repository Layer

**Files:**
- Create: `src-tauri/src/db/frozen_state.rs`
- Create: `src-tauri/src/db/agent_unread.rs`
- Modify: `src-tauri/src/db/mod.rs`

**TDD Steps:**
1. Write failing Rust tests for both repos (insert/get/delete/clear operations)
2. Run `cargo test` — confirm failures
3. Implement minimal repo functions
4. Run `cargo test` — confirm pass
5. Commit

---

## Phase 3: Scheduler Core Refactor

**File:** `src-tauri/src/scheduler/mod.rs`

**Tasks (sequential, must follow TDD):**

### 3.1 Data structure replacement
- Replace `pending_queue` with `unread_messages` + `agent_notifications` + `frozen_sessions`
- Add startup recovery logic (load from DB into memory)

### 3.2 `distribute_message` (unified entry)
- All messages (user + agent) go through this method
- Inserts into `unread_messages` (memory + DB)
- Updates `agent_notifications`
- Does NOT check freeze (freeze is checked at read time)

### 3.3 `on_new_message` refactor
- User messages: reset counter + unfreeze + distribute + trigger agents
- Agent messages: distribute only (trigger happens via stage7)

### 3.4 `trigger_agent_inner` stage6/7 refactor
- Stage6 after counter increment: check limit, if reached → update `session_frozen_states` + `frozen_sessions` + emit notice
- Stage7: replace direct queue push with `distribute_message`

### 3.5 `trigger_agent` read refactor
- Read from `unread_messages` instead of `pending_queue`
- Skip frozen sessions
- Sort by `created_at`
- Delete from DB after reading

### 3.6 `start_background_scan` refactor
- Scan `agent_notifications` instead of `pending_queue`

### 3.7 `reset_message_count` enhancement (in `commands/session.rs`)
- Add `State<'_, Scheduler>` parameter
- After DB reset: unfreeze session + trigger agents with unread messages

**TDD Steps:**
1. Write failing Rust tests for each behavior above
2. Implement minimal changes to make tests pass
3. Run `cargo test` — confirm all pass
4. Commit

---

## Phase 4: reset_session Cleanup

**File:** `src-tauri/src/db/session.rs`

**TDD Steps:**
1. Write failing Rust test: `test_reset_session_clears_unread_and_frozen_state`
2. Add `DELETE FROM agent_unread_queue WHERE session_id = ?` and `DELETE FROM session_frozen_states WHERE session_id = ?` to `reset_session`
3. Run `cargo test` — confirm pass
4. Commit

---

## Phase 5: PromptAssembler Simplification

**Files:**
- Create: `src-tauri/src/llm/prompt_templates.rs`
- Modify: `src-tauri/src/llm/prompt.rs`
- Modify: `src-tauri/src/llm/mod.rs`

**Tasks:**
1. Extract all hardcoded strings to `prompt_templates.rs` as `pub const` items
2. Remove Layer 5 (`pending_messages` usage)
3. Merge all messages into Layer 4, sort chronologically
4. Add footer note constant

**TDD Steps:**
1. Write failing Rust test: `test_prompt_assemble_single_layer_sorted` — verifies no Layer 5 header exists and messages are sorted
2. Implement changes
3. Run `cargo test` — confirm pass
4. Commit

---

## Phase 6: Frontend Verification

**Files:** No frontend changes required for this refactor (backend-only).

**Verification:**
- `cargo test` — all Rust tests pass
- `cargo check` — 0 errors
- `pnpm test` — all Vitest tests pass (existing tests should not break)
- `npx svelte-check` — 0 errors

---

## File Change Summary

| File | Action | Phase |
|------|--------|-------|
| `src-tauri/src/db/schema.rs` | Modify | 1 |
| `src-tauri/src/db/migration.rs` | Modify | 1 |
| `src-tauri/src/db/frozen_state.rs` | Create | 2 |
| `src-tauri/src/db/agent_unread.rs` | Create | 2 |
| `src-tauri/src/db/mod.rs` | Modify | 2 |
| `src-tauri/src/db/session.rs` | Modify | 4 |
| `src-tauri/src/scheduler/mod.rs` | Modify | 3 |
| `src-tauri/src/commands/session.rs` | Modify | 3.7 |
| `src-tauri/src/llm/prompt.rs` | Modify | 5 |
| `src-tauri/src/llm/prompt_templates.rs` | Create | 5 |
| `src-tauri/src/llm/mod.rs` | Modify | 5 |

---

*Plan version: 1.0*  
*Date: 2026-05-13*
