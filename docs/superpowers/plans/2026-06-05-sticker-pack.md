# Sticker Pack Import/Export and Chat Usage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build sticker pack management, import/export, agent assignment, prompt injection, message rendering, and chat-input sticker insertion while keeping messages stored as plain text with `<sticker>packName_stickerName</sticker>` tags.

**Architecture:** Store sticker pack metadata in SQLite and sticker image files under `data/stickers/`. The Rust backend owns validation, image processing, import/export, prompt data, and file access; the Svelte frontend owns management screens, selection UI, input insertion, and render-time sticker tag parsing.

**Tech Stack:** Tauri v2, Rust, rusqlite, base64, image crate for static PNG/JPEG resizing, Svelte 5 runes, Vitest, Tailwind v4 utilities.

---

## File Structure

- Modify: `src-tauri/Cargo.toml`
  - Add `image` dependency for static PNG/JPEG decoding/resizing. Do not enable GIF decoding in the `image` crate for the first version.
- Modify: `src-tauri/src/db/schema.rs`
  - Add sticker tables to `BASE_SCHEMA` and a new migration SQL constant.
- Modify: `src-tauri/src/db/migration.rs`
  - Register the new migration version after V22.
- Create: `src-tauri/src/models/sticker.rs`
  - Request/response DTOs and repository structs for sticker packs, stickers, resolution results, and bundle payloads.
- Modify: `src-tauri/src/models/mod.rs`
  - Export `sticker`.
- Create: `src-tauri/src/db/sticker.rs`
  - All raw SQL for pack CRUD, sticker CRUD, agent assignments, ref resolution, prompt listing, import/export helpers.
- Modify: `src-tauri/src/db/mod.rs`
  - Export `sticker`.
- Create: `src-tauri/src/commands/sticker.rs`
  - Tauri commands for pack management, sticker upload/delete, assignment, resolution, import/export.
- Modify: `src-tauri/src/commands/mod.rs`
  - Export `sticker`.
- Modify: `src-tauri/src/lib.rs`
  - Register sticker commands.
- Modify: `src-tauri/src/llm/prompt.rs`
  - Add available sticker prompt section to normal prompt assembly.
- Modify: `src-tauri/src/llm/history_prompt.rs`
  - Add the same section to history prompt assembly.
- Modify: `src/lib/types.ts`
  - Add sticker-related TypeScript interfaces.
- Create: `src/lib/stores/stickerStore.svelte.ts`
  - Frontend sticker pack cache and ref lookup.
- Create: `src/lib/stickerParser.ts`
  - Parse message content into text/sticker parts.
- Modify: `src/lib/components/MessageBubble.svelte`
  - Render text, valid stickers, and invalid sticker labels in one bubble.
- Modify: `src/lib/components/MessageBubble.test.ts`
  - Cover sticker rendering behavior.
- Create: `src/lib/components/StickerPackManager.svelte`
  - Personal settings sticker management entry content.
- Modify: `src/lib/components/SettingsPanel.svelte`
  - Add sticker configuration entry and render `StickerPackManager`.
- Create: `src/lib/components/AgentStickerPackPanel.svelte`
  - Agent sticker-pack cover grid and save behavior.
- Modify: `src/lib/components/AgentDetail.svelte`
  - Add "Sticker Packs" tab after Timed Tasks.
- Create: `src/lib/components/StickerPicker.svelte`
  - Chat input sticker picker.
- Modify: `src/lib/components/ChatView.svelte`
  - Place `StickerPicker` next to input and insert tags into the textarea.

---

### Task 1: Database Schema and Rust Models

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/db/schema.rs`
- Modify: `src-tauri/src/db/migration.rs`
- Create: `src-tauri/src/models/sticker.rs`
- Modify: `src-tauri/src/models/mod.rs`

- [ ] **Step 1: Add static-image dependency**

Add this dependency to `src-tauri/Cargo.toml`:

```toml
image = { version = "0.25", default-features = false, features = ["png", "jpeg"] }
```

Do not add the `gif` feature. GIF files are stored unchanged and their dimensions are read from the GIF header manually in Task 3.

Run:

```bash
cd src-tauri
cargo check
```

Expected: dependency resolution succeeds and the crate still type-checks or only fails on code not yet written in later steps if this is run after partial edits.

- [ ] **Step 2: Add schema constants**

Append a migration after V22 in `src-tauri/src/db/schema.rs`:

```rust
pub const MIGRATION_V23: &str = r#"
CREATE TABLE IF NOT EXISTS sticker_packs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    is_deleted INTEGER DEFAULT 0 CHECK(is_deleted IN (0, 1)),
    deleted_at INTEGER
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_sticker_packs_name_active
    ON sticker_packs(name)
    WHERE is_deleted = 0;

CREATE INDEX IF NOT EXISTS idx_sticker_packs_list
    ON sticker_packs(is_deleted, updated_at DESC);

CREATE TABLE IF NOT EXISTS stickers (
    id TEXT PRIMARY KEY,
    pack_id TEXT NOT NULL REFERENCES sticker_packs(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    file_path TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    file_size INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    is_deleted INTEGER DEFAULT 0 CHECK(is_deleted IN (0, 1)),
    deleted_at INTEGER
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_stickers_pack_name_active
    ON stickers(pack_id, name)
    WHERE is_deleted = 0;

CREATE INDEX IF NOT EXISTS idx_stickers_pack_list
    ON stickers(pack_id, is_deleted, created_at ASC);

CREATE TABLE IF NOT EXISTS agent_sticker_packs (
    agent_id TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    pack_id TEXT NOT NULL REFERENCES sticker_packs(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (agent_id, pack_id)
);

CREATE INDEX IF NOT EXISTS idx_agent_sticker_packs_pack
    ON agent_sticker_packs(pack_id);
"#;
```

Also include the same SQL in `BASE_SCHEMA`.

- [ ] **Step 3: Register migration**

Add this entry in `src-tauri/src/db/migration.rs` after V22:

```rust
Migration {
    version: 23,
    name: "sticker packs",
    sql: super::schema::MIGRATION_V23,
},
```

- [ ] **Step 4: Add Rust model module**

Create `src-tauri/src/models/sticker.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StickerResponse {
    pub id: String,
    pub pack_id: String,
    pub name: String,
    pub file_path: String,
    pub mime_type: String,
    pub width: i32,
    pub height: i32,
    pub file_size: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StickerPackResponse {
    pub id: String,
    pub name: String,
    pub stickers: Vec<StickerResponse>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateStickerPackRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStickerPackRequest {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteStickerPackRequest {
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddStickerRequest {
    pub pack_id: String,
    pub name: String,
    pub image_data_base64: String,
    pub compression_ratio: f32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStickerRequest {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteStickersRequest {
    pub ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAgentStickerPacksRequest {
    pub agent_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetAgentStickerPacksRequest {
    pub agent_id: String,
    pub pack_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveStickerRefsRequest {
    pub refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedStickerResponse {
    pub reference: String,
    pub status: String,
    pub pack_id: Option<String>,
    pub sticker_id: Option<String>,
    pub file_path: Option<String>,
    pub mime_type: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportStickerPackRequest {
    pub pack_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportStickerPackResponse {
    pub exported_path: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportStickerPackRequest {
    pub file_content: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportStickerPackResponse {
    pub pack: StickerPackResponse,
    pub renamed: bool,
    pub warnings: Vec<String>,
}
```

`ImportStickerPackRequest.file_content` is the raw JSON text from a `.agentsticker` file. It is not base64.

Update `src-tauri/src/models/mod.rs`:

```rust
pub mod sticker;
```

- [ ] **Step 5: Verify schema compiles**

Run:

```bash
cd src-tauri
cargo check
```

Expected: PASS.

---

### Task 2: Sticker Repository and Backend Tests

**Files:**
- Create: `src-tauri/src/db/sticker.rs`
- Modify: `src-tauri/src/db/mod.rs`

- [ ] **Step 1: Write repository tests first**

Create `src-tauri/src/db/sticker.rs` with test scaffolding and failing tests:

```rust
#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    fn init() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::BASE_SCHEMA).unwrap();
        conn
    }

    #[test]
    fn create_pack_rejects_underscore() {
        let conn = init();
        let err = super::create_pack(&conn, "猫_pack").unwrap_err();
        assert!(err.contains("不能包含下划线"));
    }

    #[test]
    fn resolve_deleted_sticker_is_invalid() {
        let conn = init();
        let pack = super::create_pack(&conn, "猫").unwrap();
        let sticker = super::insert_sticker_metadata(
            &conn,
            &pack.id,
            "可爱",
            "stickers/p/s.png",
            "image/png",
            128,
            128,
            100,
        ).unwrap();
        let valid = super::resolve_refs(&conn, &[String::from("猫_可爱")]).unwrap();
        assert_eq!(valid[0].status, "valid");
        super::delete_stickers(&conn, &[sticker.id]).unwrap();
        let invalid = super::resolve_refs(&conn, &[String::from("猫_可爱")]).unwrap();
        assert_eq!(invalid[0].status, "invalid");
    }

    #[test]
    fn set_agent_packs_is_idempotent() {
        let conn = init();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES ('a1', 'A', '', '', 0, 0)",
            [],
        ).unwrap();
        let pack = super::create_pack(&conn, "猫").unwrap();
        super::set_agent_pack_ids(&conn, "a1", &[pack.id.clone()]).unwrap();
        super::set_agent_pack_ids(&conn, "a1", &[pack.id.clone()]).unwrap();
        let ids = super::list_agent_pack_ids(&conn, "a1").unwrap();
        assert_eq!(ids, vec![pack.id]);
    }
}
```

Run:

```bash
cd src-tauri
cargo test db::sticker
```

Expected: FAIL because repository functions do not exist yet.

- [ ] **Step 2: Implement repository types and validation**

Add to `src-tauri/src/db/sticker.rs` above tests:

```rust
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::models::sticker::{ResolvedStickerResponse, StickerPackResponse, StickerResponse};

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

pub fn validate_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("名称不能为空".to_string());
    }
    if trimmed.contains('_') {
        return Err("名称不能包含下划线".to_string());
    }
    Ok(trimmed.to_string())
}

pub fn next_available_name<F>(base: &str, exists: F) -> String
where
    F: Fn(&str) -> bool,
{
    if !exists(base) {
        return base.to_string();
    }
    let mut idx = 1;
    loop {
        let candidate = format!("{}{}", base, idx);
        if !exists(&candidate) {
            return candidate;
        }
        idx += 1;
    }
}
```

- [ ] **Step 3: Implement pack and sticker CRUD helpers**

Add:

```rust
pub fn create_pack(conn: &Connection, name: &str) -> Result<StickerPackResponse, String> {
    let name = validate_name(name)?;
    let id = Uuid::new_v4().to_string();
    let now = now_ms();
    conn.execute(
        "INSERT INTO sticker_packs (id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        params![id, name, now],
    ).map_err(|e| e.to_string())?;
    get_pack(conn, &id)?.ok_or_else(|| "创建表情包后读取失败".to_string())
}

pub fn get_pack(conn: &Connection, id: &str) -> Result<Option<StickerPackResponse>, String> {
    let row = conn.query_row(
        "SELECT id, name, created_at, updated_at FROM sticker_packs WHERE id = ?1 AND is_deleted = 0",
        [id],
        |row| Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
        )),
    ).optional().map_err(|e| e.to_string())?;

    let Some((id, name, created_at, updated_at)) = row else {
        return Ok(None);
    };
    let stickers = list_stickers_by_pack(conn, &id)?;
    Ok(Some(StickerPackResponse { id, name, stickers, created_at, updated_at }))
}

pub fn list_packs(conn: &Connection) -> Result<Vec<StickerPackResponse>, String> {
    let mut stmt = conn.prepare(
        "SELECT id, name, created_at, updated_at FROM sticker_packs WHERE is_deleted = 0 ORDER BY updated_at DESC"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| Ok((
        row.get::<_, String>(0)?,
        row.get::<_, String>(1)?,
        row.get::<_, i64>(2)?,
        row.get::<_, i64>(3)?,
    ))).map_err(|e| e.to_string())?;

    let mut packs = Vec::new();
    for row in rows {
        let (id, name, created_at, updated_at) = row.map_err(|e| e.to_string())?;
        let stickers = list_stickers_by_pack(conn, &id)?;
        packs.push(StickerPackResponse { id, name, stickers, created_at, updated_at });
    }
    Ok(packs)
}

pub fn insert_sticker_metadata(
    conn: &Connection,
    pack_id: &str,
    name: &str,
    file_path: &str,
    mime_type: &str,
    width: i32,
    height: i32,
    file_size: i64,
) -> Result<StickerResponse, String> {
    let name = validate_name(name)?;
    let pack_exists: bool = conn.query_row(
        "SELECT 1 FROM sticker_packs WHERE id = ?1 AND is_deleted = 0",
        [pack_id],
        |_| Ok(true),
    ).unwrap_or(false);
    if !pack_exists {
        return Err("表情包不存在或已删除".to_string());
    }
    let id = Uuid::new_v4().to_string();
    let now = now_ms();
    conn.execute(
        "INSERT INTO stickers (id, pack_id, name, file_path, mime_type, width, height, file_size, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
        params![id, pack_id, name, file_path, mime_type, width, height, file_size, now],
    ).map_err(|e| e.to_string())?;
    get_sticker(conn, &id)?.ok_or_else(|| "创建表情后读取失败".to_string())
}
```

Implement these repository functions with raw SQL and the same validation rules:

```rust
pub fn get_sticker(conn: &Connection, id: &str) -> Result<Option<StickerResponse>, String>;

pub fn list_stickers_by_pack(conn: &Connection, pack_id: &str) -> Result<Vec<StickerResponse>, String>;

pub fn update_pack_name(conn: &Connection, id: &str, name: &str) -> Result<StickerPackResponse, String>;

pub fn delete_pack(conn: &Connection, id: &str) -> Result<(), String>;

pub fn update_sticker_name(conn: &Connection, id: &str, name: &str) -> Result<StickerResponse, String>;

pub fn delete_stickers(conn: &Connection, ids: &[String]) -> Result<(), String>;

pub fn list_agent_pack_ids(conn: &Connection, agent_id: &str) -> Result<Vec<String>, String>;

pub fn set_agent_pack_ids(conn: &Connection, agent_id: &str, pack_ids: &[String]) -> Result<(), String>;

pub fn resolve_refs(conn: &Connection, refs: &[String]) -> Result<Vec<ResolvedStickerResponse>, String>;

pub fn list_prompt_stickers(conn: &Connection, agent_id: &str) -> Result<Vec<StickerPackResponse>, String>;
```

Required behavior:

- `delete_pack` soft deletes the pack and its active stickers in one transaction, then removes rows from `agent_sticker_packs`.
- `delete_stickers` soft deletes every matching active sticker in one transaction.
- `set_agent_pack_ids` rejects nonexistent or deleted pack IDs before replacing rows.
- `resolve_refs` returns one response for every requested ref, preserving request order.
- `list_prompt_stickers` returns only packs assigned to the agent that contain at least one undeleted sticker.
- `list_prompt_stickers` excludes assigned packs whose sticker list would be empty after filtering deleted stickers.

- [ ] **Step 4: Export module**

Update `src-tauri/src/db/mod.rs`:

```rust
pub mod sticker;
```

- [ ] **Step 5: Run repository tests**

Run:

```bash
cd src-tauri
cargo test db::sticker
```

Expected: PASS.

---

### Task 3: Sticker Commands, File Processing, and Bundle Import/Export

**Files:**
- Create: `src-tauri/src/commands/sticker.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write command helper tests for image and names**

Add unit tests in `src-tauri/src/commands/sticker.rs`:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn extension_from_mime_supports_expected_types() {
        assert_eq!(super::extension_from_mime("image/png").unwrap(), "png");
        assert_eq!(super::extension_from_mime("image/jpeg").unwrap(), "jpg");
        assert_eq!(super::extension_from_mime("image/gif").unwrap(), "gif");
        assert!(super::extension_from_mime("image/webp").is_err());
    }
}
```

Run:

```bash
cd src-tauri
cargo test commands::sticker
```

Expected: FAIL because command module is not wired yet.

- [ ] **Step 2: Implement command module**

Create command functions:

```rust
use std::fs;
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::{connection::{get_db, DbState}, sticker as sticker_repo};
use crate::models::sticker::*;

#[tauri::command]
pub async fn list_sticker_packs(state: State<'_, DbState>) -> Result<Vec<StickerPackResponse>, String> {
    let conn = get_db(&state).await?;
    sticker_repo::list_packs(&conn)
}

#[tauri::command]
pub async fn create_sticker_pack(
    state: State<'_, DbState>,
    req: CreateStickerPackRequest,
) -> Result<StickerPackResponse, String> {
    let conn = get_db(&state).await?;
    sticker_repo::create_pack(&conn, &req.name)
}

pub fn extension_from_mime(mime: &str) -> Result<&'static str, String> {
    match mime {
        "image/png" => Ok("png"),
        "image/jpeg" => Ok("jpg"),
        "image/gif" => Ok("gif"),
        _ => Err("不支持的图片类型".to_string()),
    }
}
```

Add these command functions with exact command names:

```rust
#[tauri::command]
pub async fn update_sticker_pack(
    state: State<'_, DbState>,
    req: UpdateStickerPackRequest,
) -> Result<StickerPackResponse, String>;

#[tauri::command]
pub async fn delete_sticker_pack(
    state: State<'_, DbState>,
    req: DeleteStickerPackRequest,
) -> Result<(), String>;

#[tauri::command]
pub async fn add_sticker_to_pack(
    state: State<'_, DbState>,
    req: AddStickerRequest,
) -> Result<StickerResponse, String>;

#[tauri::command]
pub async fn update_sticker(
    state: State<'_, DbState>,
    req: UpdateStickerRequest,
) -> Result<StickerResponse, String>;

#[tauri::command]
pub async fn delete_stickers(
    state: State<'_, DbState>,
    req: DeleteStickersRequest,
) -> Result<(), String>;

#[tauri::command]
pub async fn list_agent_sticker_packs(
    state: State<'_, DbState>,
    req: ListAgentStickerPacksRequest,
) -> Result<Vec<String>, String>;

#[tauri::command]
pub async fn set_agent_sticker_packs(
    state: State<'_, DbState>,
    req: SetAgentStickerPacksRequest,
) -> Result<(), String>;

#[tauri::command]
pub async fn resolve_sticker_refs(
    state: State<'_, DbState>,
    req: ResolveStickerRefsRequest,
) -> Result<Vec<ResolvedStickerResponse>, String>;

#[tauri::command]
pub async fn export_sticker_pack(
    state: State<'_, DbState>,
    req: ExportStickerPackRequest,
) -> Result<ExportStickerPackResponse, String>;

#[tauri::command]
pub async fn import_sticker_pack(
    state: State<'_, DbState>,
    req: ImportStickerPackRequest,
) -> Result<ImportStickerPackResponse, String>;
```

Each command should lock the DB through `get_db(&state).await?`, delegate SQL to `db::sticker`, and keep frontend-facing request keys camelCase through `#[serde(rename_all = "camelCase")]`.

- [ ] **Step 3: Implement static image processing helper**

Use backend decoding and resizing:

```rust
fn decode_data_url(data: &str) -> Result<(Vec<u8>, Option<String>), String> {
    if let Some(comma) = data.find(',') {
        let header = &data[..comma];
        let mime = header
            .strip_prefix("data:")
            .and_then(|s| s.split(';').next())
            .map(|s| s.to_string());
        let bytes = general_purpose::STANDARD.decode(&data[comma + 1..]).map_err(|e| e.to_string())?;
        return Ok((bytes, mime));
    }
    Ok((general_purpose::STANDARD.decode(data).map_err(|e| e.to_string())?, None))
}

fn detect_mime(bytes: &[u8], hinted: Option<String>) -> Result<String, String> {
    if bytes.starts_with(b"\x89PNG") {
        return Ok("image/png".to_string());
    }
    if bytes.starts_with(b"\xff\xd8") {
        return Ok("image/jpeg".to_string());
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Ok("image/gif".to_string());
    }
    hinted.filter(|m| m == "image/png" || m == "image/jpeg" || m == "image/gif")
        .ok_or_else(|| "不支持的图片类型".to_string())
}
```

Add strict compression validation:

```rust
fn validate_compression_ratio(ratio: f32) -> Result<f32, String> {
    if !ratio.is_finite() || ratio <= 0.0 || ratio > 1.0 {
        return Err("压缩倍率必须大于 0 且小于等于 1".to_string());
    }
    Ok(ratio)
}
```

For PNG/JPEG, use `image::load_from_memory`, resize when `compression_ratio < 1.0`, and write PNG/JPEG output. `compression_ratio = 1.0` keeps the original dimensions. Reject `0`, negative values, values above `1.0`, `NaN`, and infinite values.

For GIF, call `validate_compression_ratio(req.compression_ratio)` but store the original GIF bytes unchanged regardless of the accepted ratio. Read GIF dimensions manually from the logical screen descriptor:

```rust
fn gif_dimensions(bytes: &[u8]) -> Result<(i32, i32), String> {
    if bytes.len() < 10 || !(bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) {
        return Err("无效的 GIF 文件".to_string());
    }
    let width = u16::from_le_bytes([bytes[6], bytes[7]]) as i32;
    let height = u16::from_le_bytes([bytes[8], bytes[9]]) as i32;
    if width <= 0 || height <= 0 {
        return Err("无效的 GIF 尺寸".to_string());
    }
    Ok((width, height))
}
```

- [ ] **Step 4: Implement bundle structs**

Inside `commands/sticker.rs` or move to repository if the file grows too large:

```rust
#[derive(Debug, Serialize, Deserialize)]
struct StickerPackBundle {
    format: String,
    version: i32,
    exported_at: i64,
    pack: StickerPackBundlePack,
}

#[derive(Debug, Serialize, Deserialize)]
struct StickerPackBundlePack {
    name: String,
    stickers: Vec<StickerPackBundleSticker>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StickerPackBundleSticker {
    name: String,
    mime_type: String,
    width: i32,
    height: i32,
    file_size: i64,
    base64_content: String,
}
```

Export writes JSON to:

```text
<app_root>/exports/stickers/<pack-name>.agentsticker
```

Import validates `format == "agentstage.sticker_pack"` and `version == 1`.

- [ ] **Step 5: Wire commands**

Update `src-tauri/src/commands/mod.rs`:

```rust
pub mod sticker;
```

Update `src-tauri/src/lib.rs` imports and `tauri::generate_handler!` with every sticker command.

- [ ] **Step 6: Verify backend**

Run:

```bash
cd src-tauri
cargo test db::sticker commands::sticker
cargo check
```

Expected: PASS.

---

### Task 4: Prompt Injection

**Files:**
- Modify: `src-tauri/src/llm/prompt.rs`
- Modify: `src-tauri/src/llm/history_prompt.rs`

- [ ] **Step 1: Write prompt tests**

Add tests in `src-tauri/src/llm/prompt.rs`:

```rust
#[test]
fn test_prompt_includes_assigned_stickers() {
    let conn = init_test_db();
    insert_agent(&conn, "agent1", "Test Agent", "A test persona");
    let pack = crate::db::sticker::create_pack(&conn, "猫").unwrap();
    crate::db::sticker::insert_sticker_metadata(
        &conn,
        &pack.id,
        "可爱",
        "stickers/p/s.png",
        "image/png",
        128,
        128,
        100,
    ).unwrap();
    crate::db::sticker::set_agent_pack_ids(&conn, "agent1", &[pack.id]).unwrap();
    let pending = std::collections::HashSet::new();
    let parts = PromptAssembler::assemble(&conn, "agent1", None, None, &[], &pending).unwrap();
    assert!(parts.user.contains("【可用的表情】"));
    assert!(parts.user.contains("<sticker>猫_可爱</sticker>"));
}
```

Add a companion test that unassigned stickers are omitted.
Add another test where an assigned pack has zero active stickers and assert the prompt omits `【可用的表情】`.

Run:

```bash
cd src-tauri
cargo test llm::prompt::tests::test_prompt_includes_assigned_stickers
```

Expected: FAIL before implementation.

- [ ] **Step 2: Add prompt builder helper**

Add to `PromptAssembler`:

```rust
fn build_sticker_section(conn: &Connection, agent_id: &str) -> Result<String, String> {
    let packs = crate::db::sticker::list_prompt_stickers(conn, agent_id)?;
    let has_stickers = packs.iter().any(|pack| !pack.stickers.is_empty());
    if !has_stickers {
        return Ok(String::new());
    }
    let mut section = String::from("【可用的表情】\n");
    section.push_str("你可以在回复消息中携带表情。表情不会替代文字内容，而是作为聊天中的情绪补充。\n\n");
    section.push_str("使用格式：\n");
    section.push_str("在回复内容中直接写入 <sticker>包名_表情名</sticker> 标签即可。系统会把该标签渲染成对应表情图片。\n");
    section.push_str("例如：早上好<sticker>猫_可爱</sticker>\n\n");
    section.push_str("使用建议：\n");
    section.push_str("- 如果你要使用表情，建议把 <sticker>...</sticker> 放在整条回复的开头或结尾。\n");
    section.push_str("- 一次回复中尽量最多只使用一个表情。\n");
    section.push_str("- 不要过于频繁地使用表情，只有在能自然增强语气、情绪或角色表现时再使用。\n");
    section.push_str("- 不要使用列表外的表情。\n");
    section.push_str("- 不要修改包名或表情名。\n");
    section.push_str("- 不要在标签内容里添加额外空格。\n");
    section.push_str("- 不要把表情标签拆开输出。\n\n");
    section.push_str("可用表情：\n");
    for pack in packs {
        if pack.stickers.is_empty() {
            continue;
        }
        section.push_str(&format!("- {}\n", pack.name));
        for sticker in pack.stickers {
            section.push_str(&format!("  - {}：<sticker>{}_{}</sticker>\n", sticker.name, pack.name, sticker.name));
        }
    }
    Ok(section)
}
```

Push the section into `user_layers` before the final instruction layer when non-empty.

- [ ] **Step 3: Mirror in history prompt**

Expose a repository helper usable by `HistoryPromptAssembler`, or move the prompt-section formatting into a shared helper in `src-tauri/src/llm/prompt.rs` with public visibility.

In `history_prompt.rs`, insert the same section before the tool instruction text.

- [ ] **Step 4: Run prompt tests**

Run:

```bash
cd src-tauri
cargo test llm::prompt llm::history_prompt
```

Expected: PASS.

---

### Task 5: Frontend Types, Store, Parser, and Message Rendering

**Files:**
- Modify: `src/lib/types.ts`
- Create: `src/lib/stores/stickerStore.svelte.ts`
- Create: `src/lib/stickerParser.ts`
- Modify: `src/lib/components/MessageBubble.svelte`
- Modify: `src/lib/components/MessageBubble.test.ts`

- [ ] **Step 1: Add TypeScript types**

Append to `src/lib/types.ts`:

```ts
export interface Sticker {
    id: string;
    packId: string;
    name: string;
    filePath: string;
    mimeType: string;
    width: number;
    height: number;
    fileSize: number;
    createdAt: number;
    updatedAt: number;
}

export interface StickerPack {
    id: string;
    name: string;
    stickers: Sticker[];
    createdAt: number;
    updatedAt: number;
}

export interface ResolvedSticker {
    reference: string;
    status: 'valid' | 'invalid';
    packId: string | null;
    stickerId: string | null;
    filePath: string | null;
    mimeType: string | null;
    width: number | null;
    height: number | null;
}
```

- [ ] **Step 2: Write parser tests**

Create `src/lib/stickerParser.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { parseStickerContent } from './stickerParser';

describe('parseStickerContent', () => {
    it('parses text and sticker tags', () => {
        expect(parseStickerContent('早上好<sticker>猫_可爱</sticker>')).toEqual([
            { type: 'text', text: '早上好' },
            { type: 'sticker', reference: '猫_可爱' },
        ]);
    });

    it('leaves malformed tags as text', () => {
        expect(parseStickerContent('<sticker>猫</sticker>')).toEqual([
            { type: 'text', text: '<sticker>猫</sticker>' },
        ]);
    });
});
```

Run:

```bash
pnpm test -- src/lib/stickerParser.test.ts
```

Expected: FAIL before parser implementation.

- [ ] **Step 3: Implement parser**

Create `src/lib/stickerParser.ts`:

```ts
export type StickerContentPart =
    | { type: 'text'; text: string }
    | { type: 'sticker'; reference: string };

const STICKER_RE = /<sticker>([^<]+)<\/sticker>/g;

export function parseStickerContent(content: string): StickerContentPart[] {
    const parts: StickerContentPart[] = [];
    let lastIndex = 0;
    for (const match of content.matchAll(STICKER_RE)) {
        const start = match.index ?? 0;
        if (start > lastIndex) {
            parts.push({ type: 'text', text: content.slice(lastIndex, start) });
        }
        const ref = match[1].trim();
        const pieces = ref.split('_');
        if (pieces.length === 2 && pieces[0] && pieces[1]) {
            parts.push({ type: 'sticker', reference: ref });
        } else {
            parts.push({ type: 'text', text: match[0] });
        }
        lastIndex = start + match[0].length;
    }
    if (lastIndex < content.length) {
        parts.push({ type: 'text', text: content.slice(lastIndex) });
    }
    return parts.length > 0 ? parts : [{ type: 'text', text: content }];
}
```

- [ ] **Step 4: Implement sticker store**

Create `src/lib/stores/stickerStore.svelte.ts`:

```ts
import { invoke } from '@tauri-apps/api/core';
import { convertFileSrc } from '@tauri-apps/api/core';
import type { ResolvedSticker, StickerPack } from '$lib/types';

class StickerStore {
    packs = $state<StickerPack[]>([]);
    loading = $state(false);
    private resolved = $state<Map<string, ResolvedSticker>>(new Map());

    async load() {
        this.loading = true;
        try {
            this.packs = await invoke<StickerPack[]>('list_sticker_packs');
            const next = new Map<string, ResolvedSticker>();
            for (const pack of this.packs) {
                for (const sticker of pack.stickers) {
                    const reference = `${pack.name}_${sticker.name}`;
                    next.set(reference, {
                        reference,
                        status: 'valid',
                        packId: pack.id,
                        stickerId: sticker.id,
                        filePath: sticker.filePath,
                        mimeType: sticker.mimeType,
                        width: sticker.width,
                        height: sticker.height,
                    });
                }
            }
            this.resolved = next;
        } finally {
            this.loading = false;
        }
    }

    resolve(reference: string): ResolvedSticker | null {
        return this.resolved.get(reference) ?? null;
    }

    async resolveMissing(references: string[]) {
        const missing = references.filter((ref) => !this.resolved.has(ref));
        if (missing.length === 0) return;
        const results = await invoke<ResolvedSticker[]>('resolve_sticker_refs', {
            req: { refs: missing },
        });
        const next = new Map(this.resolved);
        for (const result of results) {
            next.set(result.reference, result);
        }
        this.resolved = next;
    }

    imageUrl(filePath: string): string {
        return convertFileSrc(filePath);
    }
}

export const stickerStore = new StickerStore();
```

The default render path uses the local index loaded by `list_sticker_packs`. `resolveMissing()` keeps the backend `resolve_sticker_refs` command useful for cache misses, stale data, and future partial-loading paths.

- [ ] **Step 5: Update MessageBubble rendering**

Use `parseStickerContent(message.content)` and `stickerStore.resolve(reference)`.

Valid sticker markup:

```svelte
<img
    src={stickerStore.imageUrl(resolved.filePath)}
    alt={part.reference}
    class="inline-block max-w-32 max-h-32 align-middle"
/>
```

Invalid sticker markup:

```svelte
<span class="inline-flex items-center px-2 py-1 text-xs rounded bg-bg text-text-secondary border border-border">
    失效表情
</span>
```

- [ ] **Step 6: Update MessageBubble tests**

Mock `stickerStore` or load it with test state so tests cover:

```ts
it('renders invalid sticker label for unknown sticker refs', () => {
    render(MessageBubble, {
        props: {
            message: { ...baseMessage, content: 'Hi<sticker>猫_丢失</sticker>' },
            isMe: true,
            senderName: 'User',
        },
    });
    expect(screen.getByText('Hi')).toBeInTheDocument();
    expect(screen.getByText('失效表情')).toBeInTheDocument();
});
```

- [ ] **Step 7: Run frontend unit tests**

Run:

```bash
pnpm test -- src/lib/stickerParser.test.ts src/lib/components/MessageBubble.test.ts
```

Expected: PASS.

---

### Task 6: Settings Sticker Manager

**Files:**
- Create: `src/lib/components/StickerPackManager.svelte`
- Modify: `src/lib/components/SettingsPanel.svelte`

- [ ] **Step 1: Create management component**

Create `StickerPackManager.svelte` with command calls:

```ts
await invoke('create_sticker_pack', { req: { name: packName } });
await invoke('update_sticker_pack', { req: { id: packId, name: newName } });
await invoke('update_sticker', { req: { id: stickerId, name: newStickerName } });
await invoke('delete_sticker_pack', { req: { id: packId } });
await invoke('add_sticker_to_pack', {
    req: {
        packId,
        name: stickerName,
        imageDataBase64,
        compressionRatio,
    },
});
await invoke('delete_stickers', { req: { ids: selectedStickerIds } });
await invoke('export_sticker_pack', { req: { packId } });
await invoke('import_sticker_pack', { req: { fileContent } });
```

After each mutation:

```ts
await stickerStore.load();
```

For import, use an ordinary file input:

```ts
async function importFromFile(file: File) {
    const fileContent = await file.text();
    await invoke('import_sticker_pack', { req: { fileContent } });
    await stickerStore.load();
}
```

`fileContent` is raw JSON text from the `.agentsticker` file, not base64. Native Tauri file dialogs and drag-and-drop are not required for the first implementation.

- [ ] **Step 2: Add deletion and rename warnings**

Before pack delete, sticker delete, pack rename, or sticker rename, call the existing `ConfirmDialog` pattern with copy that states historical stickers may become invalid.

- [ ] **Step 3: Add compression preview**

On selected image file:

```ts
const img = new Image();
img.onload = () => {
    previewWidth = Math.max(1, Math.round(img.width * compressionRatio));
    previewHeight = Math.max(1, Math.round(img.height * compressionRatio));
};
img.src = URL.createObjectURL(file);
```

For GIF, show original dimensions if available and explain that GIF may be saved unchanged. If a GIF is selected and the user chooses a compression ratio below `1.0`, keep the preview copy explicit that GIF files are stored unchanged in the first version.

- [ ] **Step 4: Wire settings tab**

In `SettingsPanel.svelte`, import and add a tab/entry:

```svelte
<button onclick={() => activeTab = 'stickers'}>表情包配置</button>
```

Render:

```svelte
{:else if activeTab === 'stickers'}
    <StickerPackManager />
{/if}
```

- [ ] **Step 5: Run checks**

Run:

```bash
pnpm check
pnpm test -- src/lib/stickerParser.test.ts src/lib/components/MessageBubble.test.ts
```

Expected: PASS.

---

### Task 7: Agent Sticker-Pack Assignment UI

**Files:**
- Create: `src/lib/components/AgentStickerPackPanel.svelte`
- Modify: `src/lib/components/AgentDetail.svelte`

- [ ] **Step 1: Create panel component**

Create props:

```ts
interface Props {
    agentId: string;
}
```

On mount or `agentId` change:

```ts
await stickerStore.load();
const selected = await invoke<string[]>('list_agent_sticker_packs', {
    req: { agentId },
});
selectedPackIds = new Set(selected);
```

Toggle behavior:

```ts
function togglePack(id: string) {
    const next = new Set(selectedPackIds);
    if (next.has(id)) {
        next.delete(id);
    } else {
        next.add(id);
    }
    selectedPackIds = next;
}
```

Save behavior:

```ts
await invoke('set_agent_sticker_packs', {
    req: {
        agentId,
        packIds: Array.from(selectedPackIds),
    },
});
```

Cover image:

```ts
const cover = pack.stickers[0] ?? null;
```

- [ ] **Step 2: Add AgentDetail tab**

Update active tab union:

```ts
let activeTab = $state<'config' | 'relationships' | 'memory' | 'timer' | 'stickers'>('config');
```

Add the tab button after timed tasks:

```svelte
<button onclick={() => activeTab = 'stickers'}>表情包</button>
```

Render:

```svelte
{:else if activeTab === 'stickers' && agent}
    <AgentStickerPackPanel agentId={agent.id} />
{/if}
```

- [ ] **Step 3: Run checks**

Run:

```bash
pnpm check
```

Expected: PASS.

---

### Task 8: Chat Input Sticker Picker

**Files:**
- Create: `src/lib/components/StickerPicker.svelte`
- Modify: `src/lib/components/ChatView.svelte`

- [ ] **Step 1: Create picker component**

Props:

```ts
interface Props {
    onPick: (tag: string) => void;
}
```

When a sticker is clicked:

```ts
onPick(`<sticker>${pack.name}_${sticker.name}</sticker>`);
```

Load data through `stickerStore.load()` if `packs` is empty.

The chat sticker picker shows all configured undeleted sticker packs and stickers. It is not limited by the current chat's agent assignments. Agent assignments only control prompt injection for agent replies.

- [ ] **Step 2: Insert into current textarea**

In `ChatView.svelte`, keep a textarea binding:

```ts
let inputEl: HTMLTextAreaElement | null = $state(null);
```

Attach:

```svelte
<textarea bind:this={inputEl} ... />
```

Insert tag:

```ts
function insertStickerTag(tag: string) {
    const start = inputEl?.selectionStart ?? inputText.length;
    const end = inputEl?.selectionEnd ?? inputText.length;
    inputText = inputText.slice(0, start) + tag + inputText.slice(end);
    const id = mode === 'chat' ? sessionStore.selectedSessionId : historyStore.selectedSessionId;
    if (id) {
        const next = new Map(inputBySession);
        next.set(id, inputText);
        inputBySession = next;
    }
    requestAnimationFrame(() => {
        inputEl?.focus();
        const pos = start + tag.length;
        inputEl?.setSelectionRange(pos, pos);
    });
}
```

Render `StickerPicker` next to the input controls:

```svelte
<StickerPicker onPick={insertStickerTag} />
```

- [ ] **Step 3: Run checks**

Run:

```bash
pnpm check
pnpm build
```

Expected: PASS.

---

### Task 9: End-to-End Verification and Regression Checks

**Files:**
- No new code files unless failures require fixes.

- [ ] **Step 1: Run Rust checks**

Run:

```bash
cd src-tauri
cargo test
cargo check
```

Expected: PASS.

- [ ] **Step 2: Run frontend checks**

Run:

```bash
pnpm test
pnpm check
pnpm build
```

Expected: PASS.

- [ ] **Step 3: Manual desktop verification**

Run:

```bash
pnpm tauri dev
```

Expected: app opens.

Verify manually:

- Create a sticker pack in personal settings.
- Add one PNG sticker with compression ratio below `1.0`.
- Add one GIF sticker and confirm it displays as animated where supported by WebView2.
- Export the pack to `.agentsticker`.
- Import it again and confirm duplicate names become numeric suffixes without `_`.
- Assign one pack to an agent through the cover grid.
- Send a user message by picking a sticker next to the input.
- Trigger an agent response and confirm prompt logs include `【可用的表情】`.
- Delete a sticker and confirm old messages render `失效表情`.

- [ ] **Step 4: Git review without committing**

Run:

```bash
git status --short
git diff --stat
```

Expected: changes are limited to sticker feature files plus planned dependency/registration edits. Do not commit unless the user explicitly asks.
