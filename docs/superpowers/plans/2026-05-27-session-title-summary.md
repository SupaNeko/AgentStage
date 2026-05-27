# Session Page Title Summary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reset session时异步调用LLM生成旧chat_page的标题，支持配置总结模型和前端手动修改标题。

**Architecture:** 独立后台任务`run_generate_page_title`在reset时spawn；模型配置优先读取`app_settings.summary_model_config_id`，未配置则fallback到第一个有api_key的model_config；标题存到`chat_pages.name`；前端在历史页面选择器旁提供编辑UI。

**Tech Stack:** Rust (Tauri v2, rusqlite, tokio), Svelte 5, TypeScript, Tailwind v4

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src-tauri/src/db/schema.rs` | Modify | Add `MIGRATION_V20` |
| `src-tauri/src/db/migration.rs` | Modify | Register V20 in `MIGRATIONS` array |
| `src-tauri/src/models/settings.rs` | Modify | Add `summary_model_config_id` to AppSettings/SettingsResponse/UpdateAppSettingsRequest |
| `src-tauri/src/models/chat_page.rs` | Modify | Add `UpdateChatPageNameRequest` DTO |
| `src-tauri/src/db/model_config.rs` | Modify | Add `resolve_summary_model_config` helper |
| `src-tauri/src/db/chat_page.rs` | Modify | Add `update_name` function |
| `src-tauri/src/db/settings.rs` | Modify | Update SQL to include `summary_model_config_id` |
| `src-tauri/src/llm/prompt_templates.rs` | Modify | Add `PAGE_TITLE_SUMMARY_PROMPT` |
| `src-tauri/src/scheduler/mod.rs` | Modify | Add `spawn_generate_page_title` + `run_generate_page_title` |
| `src-tauri/src/commands/chat_page.rs` | Create | `update_chat_page_name` command |
| `src-tauri/src/commands/session.rs` | Modify | Append `spawn_generate_page_title` call in `reset_session` |
| `src-tauri/src/lib.rs` | Modify | Import + register `update_chat_page_name` |
| `src/lib/types.ts` | Modify | Add fields to `AppSettings`, add `UpdateChatPageNameRequest` |
| `src/lib/stores/settingsStore.svelte.ts` | Modify | Handle `summary_model_config_id` in load/update |
| `src/lib/components/SettingsPanel.svelte` | Modify | Add summary model selector in models tab |
| `src/lib/components/ChatView.svelte` | Modify | Make history page names editable |

---

## Task 1: Migration V20

**Files:**
- Modify: `src-tauri/src/db/schema.rs`
- Modify: `src-tauri/src/db/migration.rs`

- [ ] **Step 1: Add MIGRATION_V20 to schema.rs**

Add at the end of `src-tauri/src/db/schema.rs` (after `MIGRATION_V19`):

```rust
pub const MIGRATION_V20: &str = r#"
-- V20: Session page title summary model config
ALTER TABLE app_settings ADD COLUMN summary_model_config_id TEXT;
"#;
```

- [ ] **Step 2: Register V20 in migration.rs**

Add to `src-tauri/src/db/migration.rs` in the `MIGRATIONS` array, after version 19:

```rust
    Migration {
        version: 20,
        name: "session_page_title_summary",
        sql: super::schema::MIGRATION_V20,
    },
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/db/schema.rs src-tauri/src/db/migration.rs
git commit -m "feat(migration): V20 add summary_model_config_id to app_settings"
```

---

## Task 2: Backend Model Updates

**Files:**
- Modify: `src-tauri/src/models/settings.rs`
- Modify: `src-tauri/src/models/chat_page.rs`

- [ ] **Step 1: Update settings models**

Modify `src-tauri/src/models/settings.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {
    pub id: i32,
    pub global_min_trigger_interval: i32,
    pub private_message_limit_default: i32,
    pub group_message_limit_default: i32,
    pub private_limit_enabled_default: bool,
    pub group_limit_enabled_default: bool,
    pub theme: String,
    pub font_size: String,
    pub language: String,
    pub enter_to_send: bool,
    pub launch_on_startup: bool,
    pub minimize_to_tray: bool,
    pub active_persona_id: Option<String>,
    pub default_avatar_path: Option<String>,
    pub quiet_hours_start: i32,
    pub quiet_hours_end: i32,
    pub summary_model_config_id: Option<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsResponse {
    pub global_min_trigger_interval: i32,
    pub private_message_limit_default: i32,
    pub group_message_limit_default: i32,
    pub private_limit_enabled_default: bool,
    pub group_limit_enabled_default: bool,
    pub theme: String,
    pub font_size: String,
    pub language: String,
    pub enter_to_send: bool,
    pub launch_on_startup: bool,
    pub minimize_to_tray: bool,
    pub active_persona_id: Option<String>,
    pub default_avatar_path: Option<String>,
    pub quiet_hours_start: i32,
    pub quiet_hours_end: i32,
    pub summary_model_config_id: Option<String>,
}

impl From<AppSettings> for SettingsResponse {
    fn from(s: AppSettings) -> Self {
        Self {
            global_min_trigger_interval: s.global_min_trigger_interval,
            private_message_limit_default: s.private_message_limit_default,
            group_message_limit_default: s.group_message_limit_default,
            private_limit_enabled_default: s.private_limit_enabled_default,
            group_limit_enabled_default: s.group_limit_enabled_default,
            theme: s.theme,
            font_size: s.font_size,
            language: s.language,
            enter_to_send: s.enter_to_send,
            launch_on_startup: s.launch_on_startup,
            minimize_to_tray: s.minimize_to_tray,
            active_persona_id: s.active_persona_id,
            default_avatar_path: s.default_avatar_path,
            quiet_hours_start: s.quiet_hours_start,
            quiet_hours_end: s.quiet_hours_end,
            summary_model_config_id: s.summary_model_config_id,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateAppSettingsRequest {
    pub global_min_trigger_interval: Option<i32>,
    pub private_message_limit_default: Option<i32>,
    pub group_message_limit_default: Option<i32>,
    pub private_limit_enabled_default: Option<bool>,
    pub group_limit_enabled_default: Option<bool>,
    pub theme: Option<String>,
    pub font_size: Option<String>,
    pub language: Option<String>,
    pub enter_to_send: Option<bool>,
    pub launch_on_startup: Option<bool>,
    pub minimize_to_tray: Option<bool>,
    pub active_persona_id: Option<String>,
    pub default_avatar_path: Option<String>,
    pub summary_model_config_id: Option<String>,
}
```

- [ ] **Step 2: Add UpdateChatPageNameRequest**

Modify `src-tauri/src/models/chat_page.rs`:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateChatPageNameRequest {
    pub session_id: String,
    pub page_index: i32,
    pub name: String,
}
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/models/settings.rs src-tauri/src/models/chat_page.rs
git commit -m "feat(models): add summary_model_config_id to settings and UpdateChatPageNameRequest"
```

---

## Task 3: DB Helpers

**Files:**
- Modify: `src-tauri/src/db/model_config.rs`
- Modify: `src-tauri/src/db/chat_page.rs`
- Modify: `src-tauri/src/db/settings.rs`

- [ ] **Step 1: Add resolve_summary_model_config**

Add to `src-tauri/src/db/model_config.rs` (after `count_referencing_agents`):

```rust
pub fn resolve_summary_model_config(conn: &Connection, settings: &crate::models::settings::AppSettings) -> Result<Option<ModelConfig>, String> {
    // 1. Try the configured summary model first
    if let Some(ref config_id) = settings.summary_model_config_id {
        if let Ok(Some(config)) = get_by_id(conn, config_id) {
            if config.api_key_encrypted.is_some() {
                return Ok(Some(config));
            }
        }
    }

    // 2. Fallback: first model_config with api_key_encrypted present
    let mut stmt = conn.prepare(
        &format!("SELECT {} FROM model_configs WHERE api_key_encrypted IS NOT NULL ORDER BY created_at DESC LIMIT 1", SELECT_COLUMNS)
    ).map_err(|e| e.to_string())?;
    let mut rows = stmt.query_map([], row_to_model_config).map_err(|e| e.to_string())?;
    if let Some(Ok(config)) = rows.next() {
        return Ok(Some(config));
    }

    Ok(None)
}
```

- [ ] **Step 2: Add update_name to chat_page.rs**

Modify `src-tauri/src/db/chat_page.rs`:

```rust
pub fn update_name(conn: &Connection, session_id: &str, page_index: i32, name: &str) -> Result<()> {
    conn.execute(
        "UPDATE chat_pages SET name = ?1 WHERE session_id = ?2 AND page_index = ?3",
        rusqlite::params![name, session_id, page_index],
    )?;
    Ok(())
}
```

- [ ] **Step 3: Update settings.rs SQL**

Modify `src-tauri/src/db/settings.rs`:

In `get_or_create_settings`, update the SELECT and row mapping:

```rust
    let result = conn.query_row(
        "SELECT id, global_min_trigger_interval, private_message_limit_default, \
                group_message_limit_default, private_limit_enabled_default, \
                group_limit_enabled_default, theme, font_size, language, \
                enter_to_send, launch_on_startup, minimize_to_tray, \
                active_persona_id, default_avatar_path, quiet_hours_start, quiet_hours_end, summary_model_config_id, updated_at \
         FROM app_settings WHERE id = 1",
        [],
        |row| {
            Ok(AppSettings {
                id: row.get(0)?,
                global_min_trigger_interval: row.get(1)?,
                private_message_limit_default: row.get(2)?,
                group_message_limit_default: row.get(3)?,
                private_limit_enabled_default: row.get::<_, i32>(4)? != 0,
                group_limit_enabled_default: row.get::<_, i32>(5)? != 0,
                theme: row.get(6)?,
                font_size: row.get(7)?,
                language: row.get(8)?,
                enter_to_send: row.get::<_, i32>(9)? != 0,
                launch_on_startup: row.get::<_, i32>(10)? != 0,
                minimize_to_tray: row.get::<_, i32>(11)? != 0,
                active_persona_id: row.get(12).ok(),
                default_avatar_path: row.get(13).ok(),
                quiet_hours_start: row.get(14)?,
                quiet_hours_end: row.get(15)?,
                summary_model_config_id: row.get(16).ok(),
                updated_at: row.get(17)?,
            })
        },
    );
```

In `update_settings`, update the SQL and params:

```rust
    conn.execute(
        "UPDATE app_settings SET 
            global_min_trigger_interval = ?1, private_message_limit_default = ?2,
            group_message_limit_default = ?3, private_limit_enabled_default = ?4,
            group_limit_enabled_default = ?5, theme = ?6, font_size = ?7,
            language = ?8, enter_to_send = ?9, launch_on_startup = ?10,
            minimize_to_tray = ?11, active_persona_id = ?12,
            default_avatar_path = ?13, quiet_hours_start = ?14, quiet_hours_end = ?15, summary_model_config_id = ?16, updated_at = ?17 WHERE id = 1",
        rusqlite::params![
            req.global_min_trigger_interval.unwrap_or(current.global_min_trigger_interval),
            req.private_message_limit_default.unwrap_or(current.private_message_limit_default),
            req.group_message_limit_default.unwrap_or(current.group_message_limit_default),
            req.private_limit_enabled_default.unwrap_or(current.private_limit_enabled_default) as i32,
            req.group_limit_enabled_default.unwrap_or(current.group_limit_enabled_default) as i32,
            req.theme.as_deref().unwrap_or(&current.theme),
            req.font_size.as_deref().unwrap_or(&current.font_size),
            req.language.as_deref().unwrap_or(&current.language),
            req.enter_to_send.unwrap_or(current.enter_to_send) as i32,
            req.launch_on_startup.unwrap_or(current.launch_on_startup) as i32,
            req.minimize_to_tray.unwrap_or(current.minimize_to_tray) as i32,
            req.active_persona_id.as_deref().or(current.active_persona_id.as_deref()),
            req.default_avatar_path.as_deref().or(current.default_avatar_path.as_deref()),
            current.quiet_hours_start,
            current.quiet_hours_end,
            req.summary_model_config_id.as_deref().or(current.summary_model_config_id.as_deref()),
            now,
        ],
    )?;
```

- [ ] **Step 4: Update settings.rs tests**

In `src-tauri/src/db/settings.rs` tests, update `init_test_db` to include V20:

```rust
    fn init_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = OFF;", []).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V1).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V2).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V3).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V4).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V5).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V6).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V7).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V8).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V9).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V11).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V12).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V13).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V14).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V15).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V16).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V17).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V18).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V19).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V20).unwrap();
        conn
    }
```

Also update `test_update_settings_preserve_untouched_fields` to include the new field:

```rust
        let req = UpdateAppSettingsRequest {
            global_min_trigger_interval: Some(60),
            private_message_limit_default: None,
            group_message_limit_default: None,
            private_limit_enabled_default: None,
            group_limit_enabled_default: None,
            theme: None,
            font_size: None,
            language: None,
            enter_to_send: None,
            launch_on_startup: None,
            minimize_to_tray: None,
            active_persona_id: None,
            default_avatar_path: None,
            summary_model_config_id: None,
        };
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db/model_config.rs src-tauri/src/db/chat_page.rs src-tauri/src/db/settings.rs
git commit -m "feat(db): add resolve_summary_model_config, update_chat_page_name, settings summary_model_config_id"
```

---

## Task 4: Prompt Template

**Files:**
- Modify: `src-tauri/src/llm/prompt_templates.rs`

- [ ] **Step 1: Add PAGE_TITLE_SUMMARY_PROMPT**

Add at the end of `src-tauri/src/llm/prompt_templates.rs`:

```rust
pub const PAGE_TITLE_SUMMARY_PROMPT: &str = r#"请根据以下聊天记录，生成一个简短的中文标题（10-20字），概括本次对话的核心主题。

聊天记录：
{session_messages}

要求：
- 只输出标题文本，不要加引号、不要解释、不要输出任何其他内容
- 标题应简洁、准确，便于后续识别
- 如果聊天内容很简短或没有实质内容，输出"闲聊"即可
"#;
```

- [ ] **Step 2: Commit**

```bash
git add src-tauri/src/llm/prompt_templates.rs
git commit -m "feat(prompt): add PAGE_TITLE_SUMMARY_PROMPT for session title generation"
```

---

## Task 5: Scheduler Background Task

**Files:**
- Modify: `src-tauri/src/scheduler/mod.rs`

- [ ] **Step 1: Add spawn_generate_page_title and run_generate_page_title**

Add these methods to the `impl Scheduler` block in `src-tauri/src/scheduler/mod.rs`. Place them near the existing `spawn_session_summary` method.

```rust
    pub fn spawn_generate_page_title(&self, session_id: String, page_index: i32) {
        let scheduler = self.clone();
        tokio::spawn(async move {
            if let Err(e) = scheduler.run_generate_page_title(&session_id, page_index).await {
                crate::logger::error(&format!("[PageTitle] failed for session={} page={}: {}", session_id, page_index, e));
            }
        });
    }

    async fn run_generate_page_title(&self, session_id: &str, page_index: i32) -> Result<(), String> {
        crate::logger::debug(&format!("[PageTitle] start session={} page={}", session_id, page_index));

        let conn = self.db_state.0.lock().await;

        // 1. Get messages for this page (up to 50, chronological)
        let messages: Vec<crate::models::message::Message> = {
            let mut stmt = conn.prepare(
                "SELECT m.id, m.session_id, m.sender_type, m.sender_id, m.content, m.created_at,
                        m.message_type, m.tool_call_data, m.generation_info, m.is_deleted,
                        COALESCE(a.name, up.name, CASE WHEN m.sender_type = 'user' THEN '用户' ELSE '未知' END) as sender_name,
                        m.page_index
                 FROM messages m
                 LEFT JOIN agents a ON m.sender_type = 'agent' AND m.sender_id = a.id AND a.is_deleted = 0
                 LEFT JOIN user_personas up ON m.sender_type = 'user' AND m.sender_id = up.id
                 WHERE m.session_id = ?1 AND m.page_index = ?2 AND m.is_deleted = 0
                 ORDER BY m.created_at ASC
                 LIMIT 50"
            ).map_err(|e| e.to_string())?;

            let rows = stmt.query_map(
                rusqlite::params![session_id, page_index],
                |row| {
                    Ok(crate::models::message::Message {
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
                        sender_avatar: None,
                        page_index: row.get(11)?,
                    })
                }
            ).map_err(|e| e.to_string())?;

            rows.filter_map(|r| r.ok()).collect()
        };

        // 2. Skip if no meaningful messages (empty or only system messages)
        let meaningful_messages: Vec<_> = messages.iter()
            .filter(|m| m.sender_type != "system")
            .collect();
        if meaningful_messages.is_empty() {
            crate::logger::debug(&format!("[PageTitle] no meaningful messages, skipping"));
            return Ok(());
        }

        // 3. Get settings to resolve summary model
        let settings = match crate::db::settings::get_or_create_settings(&conn) {
            Ok(s) => s,
            Err(e) => {
                crate::logger::warn(&format!("[PageTitle] failed to get settings: {}", e));
                return Ok(());
            }
        };

        let model_config = match crate::db::model_config::resolve_summary_model_config(&conn, &settings) {
            Ok(Some(c)) => c,
            Ok(None) => {
                crate::logger::debug(&format!("[PageTitle] no available model config, skipping"));
                return Ok(());
            }
            Err(e) => {
                crate::logger::warn(&format!("[PageTitle] failed to resolve model config: {}", e));
                return Ok(());
            }
        };

        drop(conn);

        // 4. Build session messages text
        let mut session_messages_text = String::new();
        for msg in &messages {
            let time = crate::llm::prompt::PromptAssembler::format_time(msg.created_at);
            session_messages_text.push_str(&format!("[{}] {}: {}\n", time, msg.sender_name, msg.content));
        }

        let system_prompt = crate::llm::prompt_templates::PAGE_TITLE_SUMMARY_PROMPT
            .replace("{session_messages}", &session_messages_text);

        // 5. Decrypt api key
        let api_key = match model_config.api_key_encrypted {
            Some(ref encrypted) => match crate::crypto::decrypt(encrypted) {
                Ok(k) => k,
                Err(e) => {
                    crate::logger::error(&format!("[PageTitle] failed to decrypt api key: {}", e));
                    return Ok(());
                }
            },
            None => {
                crate::logger::warn(&format!("[PageTitle] model config has no api key"));
                return Ok(());
            }
        };

        // 6. Call LLM
        let provider = crate::llm::openai::OpenAiCompatibleProvider::new(
            api_key,
            model_config.base_url,
            model_config.model_name,
            None, // temperature: use default
            Some(100), // max_tokens: title should be short
        );

        let messages_json = vec![serde_json::json!({
            "role": "user",
            "content": system_prompt
        })];

        crate::logger::debug(&format!("[PageTitle] calling LLM with model={}", model_config.model_name));

        let response = match provider.chat("", messages_json, vec![]).await {
            Ok(resp) => resp,
            Err(e) => {
                crate::logger::error(&format!("[PageTitle] LLM call failed: {}", e));
                return Ok(());
            }
        };

        // 7. Clean response
        let mut title = response.content.trim().to_string();
        // Remove <think>...</think> tags and their content
        if let Some(start) = title.find("<think>") {
            if let Some(end) = title.find("</think>") {
                title = title[..start].to_string() + &title[end + 8..];
            }
        }
        title = title.trim().to_string();
        // Truncate to 30 chars
        if title.chars().count() > 30 {
            title = title.chars().take(30).collect();
        }

        if title.is_empty() {
            crate::logger::debug(&format!("[PageTitle] empty title after cleaning, skipping"));
            return Ok(());
        }

        // 8. Update chat_pages name
        let conn = self.db_state.0.lock().await;
        if let Err(e) = crate::db::chat_page::update_name(&conn, session_id, page_index, &title) {
            crate::logger::error(&format!("[PageTitle] failed to update name: {}", e));
        } else {
            crate::logger::debug(&format!("[PageTitle] updated name to '{}' for session={} page={}", title, session_id, page_index));
        }

        crate::logger::debug(&format!("[PageTitle] complete session={} page={}", session_id, page_index));
        Ok(())
    }
```

- [ ] **Step 2: Commit**

```bash
git add src-tauri/src/scheduler/mod.rs
git commit -m "feat(scheduler): add generate_page_title background task"
```

---

## Task 6: Commands and Registration

**Files:**
- Create: `src-tauri/src/commands/chat_page.rs`
- Modify: `src-tauri/src/commands/session.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create update_chat_page_name command**

Create `src-tauri/src/commands/chat_page.rs`:

```rust
use tauri::State;
use crate::db::connection::{get_db, DbState};
use crate::models::chat_page::UpdateChatPageNameRequest;

#[tauri::command]
pub async fn update_chat_page_name(
    state: State<'_, DbState>,
    req: UpdateChatPageNameRequest,
) -> Result<(), String> {
    let conn = get_db(&state).await?;
    crate::db::chat_page::update_name(&conn, &req.session_id, req.page_index, &req.name)
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Modify reset_session to trigger title generation**

In `src-tauri/src/commands/session.rs`, modify the `reset_session` command:

```rust
#[tauri::command]
pub async fn reset_session(
    state: State<'_, DbState>,
    scheduler: State<'_, Scheduler>,
    req: crate::models::session::ResetSessionRequest,
) -> Result<String, String> {
    let conn = get_db(&state).await?;
    let (page_id, new_page_index) = session_repo::reset_session(&conn, &req.session_id)
        .map_err(|e| e.to_string())?;
    scheduler.cancel_session(&req.session_id).await;

    // Spawn background AI summary task
    if new_page_index > 0 {
        let old_page_index = new_page_index - 1;
        scheduler.spawn_session_summary(req.session_id.clone(), old_page_index);
        scheduler.spawn_generate_page_title(req.session_id.clone(), old_page_index);
    }

    Ok(page_id)
}
```

- [ ] **Step 3: Register command in lib.rs**

In `src-tauri/src/lib.rs`:

Add import:
```rust
use commands::chat_page::update_chat_page_name;
```

Add to `invoke_handler!` macro:
```rust
            update_chat_page_name,
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/chat_page.rs src-tauri/src/commands/session.rs src-tauri/src/lib.rs
git commit -m "feat(commands): add update_chat_page_name and trigger title generation on reset"
```

---

## Task 7: Frontend Types

**Files:**
- Modify: `src/lib/types.ts`

- [ ] **Step 1: Update AppSettings and add UpdateChatPageNameRequest**

Modify `src/lib/types.ts`:

```typescript
export interface AppSettings {
    global_min_trigger_interval: number;
    private_message_limit_default: number;
    group_message_limit_default: number;
    private_limit_enabled_default: boolean;
    group_limit_enabled_default: boolean;
    enter_to_send: boolean;
    theme: string;
    font_size: string;
    language: string;
    launch_on_startup?: boolean;
    minimize_to_tray?: boolean;
    active_persona_id?: string | null;
    default_avatar_path?: string | null;
    quiet_hours_start?: number;
    quiet_hours_end?: number;
    summary_model_config_id: string | null;
}

export interface UpdateChatPageNameRequest {
    session_id: string;
    page_index: number;
    name: string;
}
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/types.ts
git commit -m "feat(types): add summary_model_config_id and UpdateChatPageNameRequest"
```

---

## Task 8: Frontend Settings Store

**Files:**
- Modify: `src/lib/stores/settingsStore.svelte.ts`

- [ ] **Step 1: Handle summary_model_config_id in load and update**

Modify `src/lib/stores/settingsStore.svelte.ts`:

```typescript
import { invoke } from '@tauri-apps/api/core';

export interface AppSettings {
    global_min_trigger_interval: number;
    private_message_limit_default: number;
    group_message_limit_default: number;
    private_limit_enabled_default: boolean;
    group_limit_enabled_default: boolean;
    enter_to_send: boolean;
    theme: string;
    font_size: string;
    language: string;
    launch_on_startup?: boolean;
    minimize_to_tray?: boolean;
    active_persona_id?: string | null;
    default_avatar_path?: string | null;
    quiet_hours_start?: number;
    quiet_hours_end?: number;
    summary_model_config_id: string | null;
}

class SettingsStore {
    settings = $state<AppSettings | null>(null);
    loading = $state(false);

    async load() {
        this.loading = true;
        try {
            this.settings = await invoke<AppSettings>('get_settings');
        } finally {
            this.loading = false;
        }
    }

    async update(partial: Partial<AppSettings>) {
        const req = {
            global_min_trigger_interval: partial.global_min_trigger_interval,
            private_message_limit_default: partial.private_message_limit_default,
            group_message_limit_default: partial.group_message_limit_default,
            private_limit_enabled_default: partial.private_limit_enabled_default,
            group_limit_enabled_default: partial.group_limit_enabled_default,
            enter_to_send: partial.enter_to_send,
            theme: partial.theme,
            font_size: partial.font_size,
            language: partial.language,
            launch_on_startup: partial.launch_on_startup,
            minimize_to_tray: partial.minimize_to_tray,
            active_persona_id: partial.active_persona_id,
            default_avatar_path: partial.default_avatar_path,
            summary_model_config_id: partial.summary_model_config_id,
        };
        const updated = await invoke<AppSettings>('update_settings', { req });
        this.settings = updated;
        return updated;
    }
}

export const settingsStore = new SettingsStore();
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/stores/settingsStore.svelte.ts
git commit -m "feat(settingsStore): handle summary_model_config_id"
```

---

## Task 9: Frontend SettingsPanel

**Files:**
- Modify: `src/lib/components/SettingsPanel.svelte`

- [ ] **Step 1: Add summary model selector to models tab**

In `src/lib/components/SettingsPanel.svelte`, modify the models tab section. The current code is:

```svelte
            {:else if activeTab === 'models'}
                <ModelConfigPanel />
```

Replace with:

```svelte
            {:else if activeTab === 'models'}
                <ModelConfigPanel />
                <div class="px-6 pb-6">
                    <div class="mt-6 pt-6 border-t border-border">
                        <h4 class="font-medium mb-2">标题总结模型</h4>
                        <select
                            value={draft.summary_model_config_id ?? ''}
                            onchange={(e) => draft.summary_model_config_id = e.currentTarget.value || null}
                            class="w-full px-3 py-2 bg-bg border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 input-field"
                        >
                            <option value="">自动选择（第一个可用模型）</option>
                            {#each modelConfigStore.configs as cfg}
                                <option value={cfg.id}>{cfg.name} ({cfg.model_name})</option>
                            {/each}
                        </select>
                        <p class="text-xs text-text-secondary mt-1">
                            重置会话时，用于总结聊天记录生成历史页面标题。不选则自动使用第一个配置了 API Key 的模型。
                        </p>
                    </div>
                </div>
```

Also update the `$effect` that initializes `draft` from settings to include `summary_model_config_id`:

```typescript
    $effect(() => {
        if (settingsStore.settings) {
            draft = {
                global_min_trigger_interval: settingsStore.settings.global_min_trigger_interval,
                summary_model_config_id: settingsStore.settings.summary_model_config_id,
            };
            quietHoursEnabled = (settingsStore.settings.quiet_hours_start ?? -1) >= 0;
            quietStart = minutesToTime(settingsStore.settings.quiet_hours_start ?? 0);
            quietEnd = minutesToTime(settingsStore.settings.quiet_hours_end ?? 480);
        }
    });
```

And update `handleSave` to include it:

```typescript
    async function handleSave() {
        saving = true;
        try {
            await settingsStore.update({
                global_min_trigger_interval: draft.global_min_trigger_interval,
                summary_model_config_id: draft.summary_model_config_id,
            });
            // ... rest of handleSave unchanged
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/components/SettingsPanel.svelte
git commit -m "feat(SettingsPanel): add summary model selector in models tab"
```

---

## Task 10: Frontend ChatView Title Editing

**Files:**
- Modify: `src/lib/components/ChatView.svelte`

- [ ] **Step 1: Add state and handler for editing page names**

In `<script>` section of `src/lib/components/ChatView.svelte`, add:

```typescript
    import { Pencil } from 'lucide-svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';

    let editingPageIndex = $state<number | null>(null);
    let editingName = $state('');

    function startEditPage(page: ChatPage) {
        editingPageIndex = page.page_index;
        editingName = page.name;
    }

    async function savePageName(pageIndex: number) {
        const trimmed = editingName.trim();
        const newName = trimmed || '未命名对话';
        if (!historyStore.selectedSessionId) return;
        try {
            await invoke('update_chat_page_name', {
                req: {
                    session_id: historyStore.selectedSessionId,
                    page_index: pageIndex,
                    name: newName,
                }
            });
            const idx = historyStore.chatPages.findIndex(p => p.page_index === pageIndex);
            if (idx !== -1) {
                historyStore.chatPages[idx].name = newName;
                historyStore.chatPages = [...historyStore.chatPages];
            }
        } catch (err) {
            toastStore.show('保存失败: ' + String(err), 'error', 3000);
        } finally {
            editingPageIndex = null;
        }
    }

    function cancelEditPage() {
        editingPageIndex = null;
    }

    function handleEditKey(e: KeyboardEvent, pageIndex: number) {
        if (e.key === 'Enter') {
            savePageName(pageIndex);
        } else if (e.key === 'Escape') {
            cancelEditPage();
        }
    }
```

Make sure `ChatPage` is imported in the types. If it's not already imported from `$lib/types`, add it:

```typescript
    import type { ChatPage } from '$lib/types';
```

- [ ] **Step 2: Replace the history page selector**

Replace the existing history page selector (around line 543-562) with:

```svelte
                {#if mode === 'history' && historyStore.chatPages.length > 0}
                    <div class="absolute left-1/2 -translate-x-1/2 flex items-center gap-2">
                        <div class="flex items-center gap-1 bg-bg border border-border rounded-lg px-2 py-1">
                            {#each historyStore.chatPages as page (page.page_index)}
                                {#if editingPageIndex === page.page_index}
                                    <input
                                        bind:value={editingName}
                                        onkeydown={(e) => handleEditKey(e, page.page_index)}
                                        onblur={() => savePageName(page.page_index)}
                                        class="text-sm px-1 py-0.5 bg-bg border border-primary rounded w-32 focus:outline-none"
                                        autofocus
                                    />
                                {:else}
                                    <button
                                        class="text-sm px-2 py-0.5 rounded transition-colors {historyStore.selectedPageIndex === page.page_index ? 'bg-primary/10 text-primary font-medium' : 'hover:bg-gray-100'}"
                                        onclick={() => {
                                            historyStore.selectPage(page.page_index);
                                            if (historyStore.selectedSessionId) {
                                                messageStore.loadMessages(historyStore.selectedSessionId, page.page_index);
                                            }
                                        }}
                                    >
                                        {page.name}
                                    </button>
                                    <button
                                        onclick={() => startEditPage(page)}
                                        class="p-0.5 text-text-secondary hover:text-text transition-colors"
                                        title="编辑标题"
                                    >
                                        <Pencil size={12} />
                                    </button>
                                {/if}
                            {/each}
                        </div>
                    </div>
                {/if}
```

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/ChatView.svelte
git commit -m "feat(ChatView): editable history page titles"
```

---

## Task 11: Verification

- [ ] **Step 1: Run cargo check**

```bash
cd src-tauri && cargo check 2>&1
```

Expected: 0 errors.

- [ ] **Step 2: Run svelte-check**

```bash
npx svelte-check --tsconfig ./tsconfig.json 2>&1
```

Expected: 0 errors (warnings OK).

- [ ] **Step 3: Run Rust tests**

```bash
cd src-tauri && cargo test 2>&1
```

Expected: All tests pass.

- [ ] **Step 4: Final commit (if any fixes were needed)**

```bash
git add -A
git commit -m "fix: address cargo check and svelte-check issues"
```

---

## Spec Coverage Self-Review

| Spec Requirement | Task |
|-----------------|------|
| V20 Migration | Task 1 |
| `summary_model_config_id` in settings models | Task 2 |
| `UpdateChatPageNameRequest` DTO | Task 2 |
| `resolve_summary_model_config` helper | Task 3 |
| `update_name` in chat_page repo | Task 3 |
| settings SQL updated | Task 3 |
| `PAGE_TITLE_SUMMARY_PROMPT` | Task 4 |
| `spawn/run_generate_page_title` | Task 5 |
| `update_chat_page_name` command | Task 6 |
| reset_session triggers title generation | Task 6 |
| Command registered in lib.rs | Task 6 |
| Frontend types updated | Task 7 |
| settingsStore handles new field | Task 8 |
| SettingsPanel model selector | Task 9 |
| ChatView editable titles | Task 10 |
| cargo check + svelte-check | Task 11 |

**No gaps found.**
