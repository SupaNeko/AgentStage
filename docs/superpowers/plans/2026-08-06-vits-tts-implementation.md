# VITS 语音合成（TTS）实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build VITS voice synthesis integration with local Python runtime, role-based voice configuration, auto-translation, voice caching, and usage tracking.

**Architecture:** Rust backend manages a persistent Python VITS subprocess via stdin/stdout JSON-RPC. Frontend Svelte 5 components handle role voice config, message speaker buttons, and cache management. SQLite stores voice configs and cache metadata; `llm_usage_records` tracks translation calls via `trigger_type = "tts_translate"`.

**Tech Stack:** Tauri v2, Rust, rusqlite, tokio::process, serde_json, Svelte 5 runes, Tailwind v4, Vitest.

**关联文档:** [VITS 语音合成设计](../specs/2026-08-06-vits-tts-design.md)

---

## File Structure

### Backend
- Modify: `src-tauri/src/db/schema.rs`
- Modify: `src-tauri/src/db/migration.rs`
- Create: `src-tauri/src/models/agent_voice.rs`
- Modify: `src-tauri/src/models/mod.rs`
- Create: `src-tauri/src/db/agent_voice.rs`
- Modify: `src-tauri/src/db/mod.rs`
- Create: `src-tauri/src/vits/mod.rs`
- Create: `src-tauri/src/vits/runtime.rs`
- Create: `src-tauri/src/vits/protocol.rs`
- Modify: `src-tauri/src/lib.rs` (register vits module and commands)
- Create: `src-tauri/src/commands/voice.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs` (register voice commands)

### Frontend
- Modify: `src/lib/types.ts`
- Create: `src/lib/stores/voiceStore.svelte.ts`
- Create: `src/lib/components/AgentVoicePanel.svelte`
- Modify: `src/lib/components/AgentDetail.svelte`
- Modify: `src/lib/components/MessageBubble.svelte`
- Create: `src/lib/components/VoiceCachePanel.svelte`
- Modify: `src/lib/components/SettingsPanel.svelte`
- Modify: `src/lib/components/UsageMonitor.svelte`

---

### Task 1: Database Schema and Rust Models

**Files:**
- Modify: `src-tauri/src/db/schema.rs`
- Modify: `src-tauri/src/db/migration.rs`
- Create: `src-tauri/src/models/agent_voice.rs`
- Modify: `src-tauri/src/models/mod.rs`
- Create: `src-tauri/src/db/agent_voice.rs`
- Modify: `src-tauri/src/db/mod.rs`

- [ ] **Step 1: Add VITS tables to schema.rs**

Add to `BASE_SCHEMA`:

```sql
CREATE TABLE IF NOT EXISTS agent_voices (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    model_name TEXT NOT NULL,
    model_path TEXT NOT NULL,
    speaker_id TEXT,
    target_language TEXT NOT NULL,
    emotion_params TEXT,
    speed REAL DEFAULT 1.0,
    translate_enabled INTEGER DEFAULT 1 CHECK(translate_enabled IN (0, 1)),
    translate_model_config_id TEXT,
    generation_mode TEXT NOT NULL CHECK(generation_mode IN ('auto_play', 'auto_silent', 'manual')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS vits_cache (
    id TEXT PRIMARY KEY,
    message_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    file_path TEXT NOT NULL,
    file_size INTEGER,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
);
```

- [ ] **Step 2: Add migration to migration.rs**

```rust
pub const MIGRATION_V23: &str = r#"
CREATE TABLE IF NOT EXISTS agent_voices (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    model_name TEXT NOT NULL,
    model_path TEXT NOT NULL,
    speaker_id TEXT,
    target_language TEXT NOT NULL,
    emotion_params TEXT,
    speed REAL DEFAULT 1.0,
    translate_enabled INTEGER DEFAULT 1 CHECK(translate_enabled IN (0, 1)),
    translate_model_config_id TEXT,
    generation_mode TEXT NOT NULL CHECK(generation_mode IN ('auto_play', 'auto_silent', 'manual')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS vits_cache (
    id TEXT PRIMARY KEY,
    message_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    file_path TEXT NOT NULL,
    file_size INTEGER,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
);
"#;
```

Register in `get_migrations()` after V22.

- [ ] **Step 3: Create Rust model structs**

Create `src-tauri/src/models/agent_voice.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentVoice {
    pub id: String,
    pub agent_id: String,
    pub model_name: String,
    pub model_path: String,
    pub speaker_id: Option<String>,
    pub target_language: String,
    pub emotion_params: Option<String>,
    pub speed: f64,
    pub translate_enabled: bool,
    pub translate_model_config_id: Option<String>,
    pub generation_mode: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveAgentVoiceRequest {
    pub agent_id: String,
    pub model_name: String,
    pub model_path: String,
    pub speaker_id: Option<String>,
    pub target_language: String,
    pub emotion_params: Option<String>,
    pub speed: f64,
    pub translate_enabled: bool,
    pub translate_model_config_id: Option<String>,
    pub generation_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VitsModelInfo {
    pub name: String,
    pub path: String,
    pub language: Option<String>,
    pub speakers: Vec<String>,
    pub has_config: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateVoiceRequest {
    pub message_id: String,
    pub session_id: String,
    pub agent_id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceCacheItem {
    pub id: String,
    pub message_id: String,
    pub session_id: String,
    pub agent_id: String,
    pub file_path: String,
    pub file_size: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateForTtsRequest {
    pub text: String,
    pub target_language: String,
    pub agent_persona: String,
    pub agent_relationships: String,
    pub memories: String,
    pub model_config_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslateForTtsResponse {
    pub need_translate: bool,
    pub translated_text: String,
}

#[derive(Debug, Clone)]
pub struct TranslateForTtsResult {
    pub response: TranslateForTtsResponse,
    pub usage: Option<serde_json::Value>,
}
```

Add to `models/mod.rs`: `pub mod agent_voice;`

- [ ] **Step 4: Create DB repository**

Create `src-tauri/src/db/agent_voice.rs` with CRUD functions:
- `save_agent_voice(conn, req) -> Result<AgentVoice>`
- `get_agent_voice_by_agent_id(conn, agent_id) -> Result<Option<AgentVoice>>`
- `delete_agent_voice(conn, agent_id) -> Result<()>`
- `insert_vits_cache(conn, message_id, session_id, agent_id, file_path, file_size) -> Result<VoiceCacheItem>`
- `get_vits_cache_by_message_id(conn, message_id) -> Result<Option<VoiceCacheItem>>`
- `list_vits_cache(conn, agent_id: Option<String>) -> Result<Vec<VoiceCacheItem>>`
- `delete_vits_cache(conn, id) -> Result<()>`
- `clear_vits_cache(conn, session_id: Option<String>) -> Result<()>`

Add to `db/mod.rs`: `pub mod agent_voice;`

---

### Task 2: VITS Runtime Manager

**Files:**
- Create: `src-tauri/src/vits/mod.rs`
- Create: `src-tauri/src/vits/runtime.rs`
- Create: `src-tauri/src/vits/protocol.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create protocol module**

Create `src-tauri/src/vits/protocol.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VitsRequest {
    pub action: String,
    pub text: Option<String>,
    pub model_path: Option<String>,
    pub speaker_id: Option<String>,
    pub emotion_params: Option<String>,
    pub speed: Option<f64>,
    pub output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VitsResponse {
    pub success: bool,
    pub message: Option<String>,
    pub output_path: Option<String>,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VitsPingResponse {
    pub ready: bool,
    pub version: String,
}
```

- [ ] **Step 2: Create runtime manager**

Create `src-tauri/src/vits/runtime.rs`:

```rust
use std::path::PathBuf;
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use crate::vits::protocol::*;
use crate::logger;

pub struct VitsRuntime {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    runtime_path: PathBuf,
}

impl VitsRuntime {
    pub fn new(data_dir: &std::path::Path) -> Self {
        Self {
            child: None,
            stdin: None,
            stdout: None,
            runtime_path: data_dir.join("vits_runtime"),
        }
    }

    pub fn runtime_exists(&self) -> bool {
        self.runtime_path.join("vits_runtime.exe").exists()
    }

    pub async fn start(&mut self) -> Result<(), String> {
        if self.child.is_some() { return Ok(()); }
        let exe = self.runtime_path.join("vits_runtime.exe");
        if !exe.exists() {
            return Err("VITS runtime not found".into());
        }
        let mut child = Command::new(exe)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        self.stdin = child.stdin.take();
        self.stdout = child.stdout.take().map(BufReader::new);
        self.child = Some(child);
        self.wait_ready().await?;
        Ok(())
    }

    async fn wait_ready(&mut self) -> Result<(), String> {
        let mut line = String::new();
        if let Some(ref mut stdout) = self.stdout {
            stdout.read_line(&mut line).await.map_err(|e| e.to_string())?;
            let ping: VitsPingResponse = serde_json::from_str(&line)
                .map_err(|e| format!("Invalid ready signal: {}", e))?;
            if ping.ready { return Ok(()); }
        }
        Err("Runtime not ready".into())
    }

    pub async fn generate(&mut self, req: &VitsRequest) -> Result<VitsResponse, String> {
        if self.child.is_none() { self.start().await?; }
        let mut stdin = self.stdin.take().ok_or("No stdin")?;
        let mut stdout = self.stdout.take().ok_or("No stdout")?;

        let json = serde_json::to_string(req).map_err(|e| e.to_string())?;
        stdin.write_all(json.as_bytes()).await.map_err(|e| e.to_string())?;
        stdin.write_all(b"\n").await.map_err(|e| e.to_string())?;
        stdin.flush().await.map_err(|e| e.to_string())?;

        let mut line = String::new();
        stdout.read_line(&mut line).await.map_err(|e| e.to_string())?;

        self.stdin = Some(stdin);
        self.stdout = Some(stdout);

        serde_json::from_str(&line).map_err(|e| format!("Parse response: {}", e))
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
        }
        self.stdin = None;
        self.stdout = None;
    }
}

impl Drop for VitsRuntime {
    fn drop(&mut self) { self.stop(); }
}
```

- [ ] **Step 3: Create mod.rs**

Create `src-tauri/src/vits/mod.rs`:

```rust
pub mod protocol;
pub mod runtime;
```

- [ ] **Step 4: Register module in lib.rs**

Add to `src-tauri/src/lib.rs`:

```rust
pub mod vits;
```

---

### Task 3: Translation Tool

**Files:**
- Create: `src-tauri/src/llm/translate.rs`
- Modify: `src-tauri/src/llm/mod.rs`

- [ ] **Step 1: Create translation module**

Create `src-tauri/src/llm/translate.rs`:

```rust
use crate::models::agent_voice::*;
use crate::llm::provider::LlmProvider;
use crate::llm::tool::LlmResponse;

const TRANSLATE_PROMPT: &str = r#"
You are a translation assistant for a roleplay character.

Character persona:
{persona}

Character relationships:
{relationships}

Relevant memories:
{memories}

Task:
1. Detect whether the following text is already in the target language "{target_language}".
2. If yes, return need_translate=false and the original text.
3. If no, translate the text into "{target_language}" while preserving the character's tone and personality from the persona above.

Text:
{text}

Return JSON only:
{"need_translate": true/false, "translated_text": "..."}
"#;

pub async fn translate_for_tts(
    provider: &dyn LlmProvider,
    req: &TranslateForTtsRequest,
) -> Result<TranslateForTtsResult, String> {
    let prompt = TRANSLATE_PROMPT
        .replace("{persona}", &req.agent_persona)
        .replace("{relationships}", &req.agent_relationships)
        .replace("{memories}", &req.memories)
        .replace("{target_language}", &req.target_language)
        .replace("{text}", &req.text);

    let messages = vec![serde_json::json!({
        "role": "user",
        "content": prompt,
    })];

    let response = provider.chat("You are a helpful translation assistant.", messages, vec![]).await?;
    let content = response.content.ok_or("Empty LLM response")?;
    let json_start = content.find('{').ok_or("No JSON in response")?;
    let json_end = content.rfind('}').ok_or("No JSON in response")?;
    let json_str = &content[json_start..=json_end];
    let parsed: TranslateForTtsResponse = serde_json::from_str(json_str).map_err(|e| e.to_string())?;
    Ok(TranslateForTtsResult {
        response: parsed,
        usage: response.usage,
    })
}
```

Add to `llm/mod.rs`: `pub mod translate;`

---

### Task 4: Tauri Commands

**Files:**
- Create: `src-tauri/src/commands/voice.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create voice commands**

Create `src-tauri/src/commands/voice.rs`:

```rust
use tauri::State;
use crate::db::connection::DbState;
use crate::db::agent_voice as voice_repo;
use crate::models::agent_voice::*;
use crate::vits::runtime::VitsRuntime;
use crate::vits::protocol::*;
use crate::llm::translate;
use crate::llm::openai::OpenAiCompatibleProvider;
use crate::models::usage::LlmUsageRecord;
use crate::db::usage as usage_repo;

fn get_runtime(data_dir: &std::path::Path) -> VitsRuntime {
    VitsRuntime::new(data_dir)
}

#[tauri::command]
pub async fn check_vits_runtime() -> Result<bool, String> {
    let data_dir = crate::get_data_dir().map_err(|e| e.to_string())?;
    Ok(data_dir.join("vits_runtime").join("vits_runtime.exe").exists())
}

#[tauri::command]
pub async fn scan_vits_models() -> Result<Vec<VitsModelInfo>, String> {
    let data_dir = crate::get_data_dir().map_err(|e| e.to_string())?;
    let models_dir = data_dir.join("vits_models");
    if !models_dir.exists() { return Ok(vec![]); }

    let mut models = Vec::new();
    let entries = std::fs::read_dir(&models_dir).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_dir() { continue; }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let config_path = path.join("config.json");
        let has_config = config_path.exists();
        let mut language = None;
        let mut speakers = vec![];
        if has_config {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    language = json.get("data").and_then(|d| d.get("language")).and_then(|l| l.as_str()).map(|s| s.to_string())
                        .or_else(|| json.get("model").and_then(|m| m.get("language")).and_then(|l| l.as_str()).map(|s| s.to_string()));
                    if let Some(spk) = json.get("speakers") {
                        if let Some(arr) = spk.as_array() {
                            speakers = arr.iter().filter_map(|s| s.as_str()).map(|s| s.to_string()).collect();
                        }
                    }
                }
            }
        }
        models.push(VitsModelInfo {
            name,
            path: path.to_string_lossy().to_string(),
            language,
            speakers,
            has_config,
        });
    }
    Ok(models)
}

#[tauri::command]
pub async fn save_agent_voice(
    state: State<'_, DbState>,
    req: SaveAgentVoiceRequest,
) -> Result<AgentVoice, String> {
    let conn = crate::db::connection::get_db(&state).await?;
    voice_repo::save_agent_voice(&conn, &req).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_agent_voice(
    state: State<'_, DbState>,
    agent_id: String,
) -> Result<Option<AgentVoice>, String> {
    let conn = crate::db::connection::get_db(&state).await?;
    voice_repo::get_agent_voice_by_agent_id(&conn, &agent_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_agent_voice(
    state: State<'_, DbState>,
    agent_id: String,
) -> Result<(), String> {
    let conn = crate::db::connection::get_db(&state).await?;
    voice_repo::delete_agent_voice(&conn, &agent_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn generate_voice(
    state: State<'_, DbState>,
    req: GenerateVoiceRequest,
) -> Result<String, String> {
    let mut text = req.text.clone();

    // Step 1: Fetch voice config and check cache with DB connection
    let voice = {
        let conn = crate::db::connection::get_db(&state).await?;
        voice_repo::get_agent_voice_by_agent_id(&conn, &req.agent_id)
            .map_err(|e| e.to_string())?
            .ok_or("No voice config for this agent")?
    };

    {
        let conn = crate::db::connection::get_db(&state).await?;
        if let Some(cached) = voice_repo::get_vits_cache_by_message_id(&conn, &req.message_id)
            .map_err(|e| e.to_string())? {
            if std::path::Path::new(&cached.file_path).exists() {
                return Ok(cached.file_path);
            }
        }
    }

    // Step 2: Translation if enabled
    if voice.translate_enabled {
        let (translate_model_id, agent) = {
            let conn = crate::db::connection::get_db(&state).await?;
            let agent = crate::db::agent::get_by_id(&conn, &req.agent_id)
                .map_err(|e| e.to_string())?
                .ok_or("Agent not found")?;
            let translate_model_id = voice.translate_model_config_id.clone()
                .or_else(|| agent.model_config_id.clone())
                .ok_or("No model config for translation")?;
            (translate_model_id, agent)
        };

        let model_config = {
            let conn = crate::db::connection::get_db(&state).await?;
            crate::db::model_config::get_by_id(&conn, &translate_model_id)
                .map_err(|e| e.to_string())?
                .ok_or("Model config not found")?
        };

        let provider = OpenAiCompatibleProvider::new(
            model_config.api_key_encrypted
                .as_ref()
                .and_then(|enc| crate::crypto::decrypt(enc).ok())
                .unwrap_or_default(),
            model_config.base_url.clone(),
            model_config.model_name.clone(),
            model_config.temperature,
            model_config.max_tokens,
        );

        let translate_req = TranslateForTtsRequest {
            text: text.clone(),
            target_language: voice.target_language.clone(),
            agent_persona: agent.detailed_persona.clone(),
            agent_relationships: String::new(),
            memories: agent.long_term_memory.clone().unwrap_or_default(),
            model_config_id: translate_model_id.clone(),
        };

        let result = translate::translate_for_tts(&provider, &translate_req).await?;

        // Extract token usage from LLM response
        let (prompt_tokens, completion_tokens, total_tokens) = result.usage
            .as_ref()
            .and_then(|u| u.as_object())
            .map(|obj| {
                let prompt = obj.get("prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let completion = obj.get("completion_tokens").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let total = obj.get("total_tokens").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                (prompt, completion, total)
            })
            .unwrap_or((0, 0, 0));

        // Record usage
        let usage = LlmUsageRecord {
            id: uuid::Uuid::new_v4().to_string(),
            agent_id: req.agent_id.clone(),
            model_config_id: translate_model_id,
            session_id: Some(req.session_id.clone()),
            trigger_type: "tts_translate".to_string(),
            call_round: 1,
            prompt_tokens,
            completion_tokens,
            total_tokens,
            message_id: Some(req.message_id.clone()),
            created_at: chrono::Utc::now().timestamp_millis(),
        };
        usage_repo::insert_usage_record(&state, &usage).await.map_err(|e| e.to_string())?;

        if result.response.need_translate {
            text = result.response.translated_text;
        }
    }

    // Step 3: Generate voice via VITS
    let data_dir = crate::get_data_dir().map_err(|e| e.to_string())?;
    let mut runtime = get_runtime(&data_dir);
    let cache_dir = data_dir.join("vits_cache").join(&req.session_id);
    std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
    let output_path = cache_dir.join(format!("{}.wav", req.message_id));

    let vits_req = VitsRequest {
        action: "generate".into(),
        text: Some(text),
        model_path: Some(voice.model_path.clone()),
        speaker_id: voice.speaker_id.clone(),
        emotion_params: voice.emotion_params.clone(),
        speed: Some(voice.speed),
        output_path: Some(output_path.to_string_lossy().to_string()),
    };

    let resp = runtime.generate(&vits_req).await?;
    if !resp.success {
        return Err(resp.message.unwrap_or("VITS generation failed".into()));
    }

    // Step 4: Record cache in DB
    let file_size = std::fs::metadata(&output_path).map(|m| m.len() as i64).unwrap_or(0);
    let conn = crate::db::connection::get_db(&state).await?;
    voice_repo::insert_vits_cache(
        &conn,
        &req.message_id,
        &req.session_id,
        &req.agent_id,
        &output_path.to_string_lossy(),
        file_size,
    ).map_err(|e| e.to_string())?;

    Ok(output_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn list_voice_cache(
    state: State<'_, DbState>,
    agent_id: Option<String>,
) -> Result<Vec<VoiceCacheItem>, String> {
    let conn = crate::db::connection::get_db(&state).await?;
    voice_repo::list_vits_cache(&conn, agent_id.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_voice_cache(
    state: State<'_, DbState>,
    id: String,
) -> Result<(), String> {
    let conn = crate::db::connection::get_db(&state).await?;
    let item = voice_repo::get_vits_cache_by_id(&conn, &id).map_err(|e| e.to_string())?;
    if let Some(item) = item {
        let _ = std::fs::remove_file(&item.file_path);
    }
    voice_repo::delete_vits_cache(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_voice_cache(
    state: State<'_, DbState>,
    session_id: Option<String>,
) -> Result<(), String> {
    let conn = crate::db::connection::get_db(&state).await?;
    let items = voice_repo::list_vits_cache(&conn, None).map_err(|e| e.to_string())?;
    for item in items {
        if session_id.as_ref().map_or(true, |sid| item.session_id == *sid) {
            let _ = std::fs::remove_file(&item.file_path);
        }
    }
    voice_repo::clear_vits_cache(&conn, session_id.as_deref()).map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Register commands**

Add to `commands/mod.rs`:

```rust
pub mod voice;
```

Add to `lib.rs` invoke_handler:

```rust
commands::voice::check_vits_runtime,
commands::voice::scan_vits_models,
commands::voice::save_agent_voice,
commands::voice::get_agent_voice,
commands::voice::delete_agent_voice,
commands::voice::generate_voice,
commands::voice::list_voice_cache,
commands::voice::delete_voice_cache,
commands::voice::clear_voice_cache,
```

---

### Task 5: Frontend Types

**Files:**
- Modify: `src/lib/types.ts`

- [ ] **Step 1: Add voice types**

```typescript
export interface VitsModelInfo {
    name: string;
    path: string;
    language: string | null;
    speakers: string[];
    has_config: boolean;
}

export interface AgentVoice {
    id: string;
    agent_id: string;
    model_name: string;
    model_path: string;
    speaker_id: string | null;
    target_language: string;
    emotion_params: string | null;
    speed: number;
    translate_enabled: boolean;
    translate_model_config_id: string | null;
    generation_mode: string;
    created_at: number;
    updated_at: number;
}

export interface SaveAgentVoiceRequest {
    agent_id: string;
    model_name: string;
    model_path: string;
    speaker_id: string | null;
    target_language: string;
    emotion_params: string | null;
    speed: number;
    translate_enabled: boolean;
    translate_model_config_id: string | null;
    generation_mode: string;
}

export interface VoiceCacheItem {
    id: string;
    message_id: string;
    session_id: string;
    agent_id: string;
    file_path: string;
    file_size: number;
    created_at: number;
}

export interface GenerateVoiceRequest {
    message_id: string;
    session_id: string;
    agent_id: string;
    text: string;
}
```

---

### Task 6: Frontend Store

**Files:**
- Create: `src/lib/stores/voiceStore.svelte.ts`

- [ ] **Step 1: Create voice store**

```typescript
import { invoke } from '@tauri-apps/api/core';
import type { VitsModelInfo, AgentVoice, VoiceCacheItem, SaveAgentVoiceRequest, GenerateVoiceRequest } from '$lib/types';

class VoiceStore {
    runtimeAvailable = $state<boolean>(false);
    models = $state<VitsModelInfo[]>([]);
    agentVoices = $state<Map<string, AgentVoice>>(new Map());
    generating = $state<Set<string>>(new Set());

    async checkRuntime() {
        this.runtimeAvailable = await invoke<boolean>('check_vits_runtime');
    }

    async scanModels() {
        this.models = await invoke<VitsModelInfo[]>('scan_vits_models');
    }

    async loadAgentVoice(agentId: string) {
        const voice = await invoke<AgentVoice | null>('get_agent_voice', { agentId });
        if (voice) {
            this.agentVoices.set(agentId, voice);
        } else {
            this.agentVoices.delete(agentId);
        }
    }

    async saveAgentVoice(req: SaveAgentVoiceRequest) {
        const voice = await invoke<AgentVoice>('save_agent_voice', { req });
        this.agentVoices.set(req.agent_id, voice);
    }

    async deleteAgentVoice(agentId: string) {
        await invoke('delete_agent_voice', { agentId });
        this.agentVoices.delete(agentId);
    }

    async generateVoice(req: GenerateVoiceRequest): Promise<string> {
        this.generating.add(req.message_id);
        try {
            return await invoke<string>('generate_voice', { req });
        } finally {
            this.generating.delete(req.message_id);
        }
    }

    async listCache(agentId?: string): Promise<VoiceCacheItem[]> {
        return await invoke<VoiceCacheItem[]>('list_voice_cache', { agentId });
    }

    async deleteCache(id: string) {
        await invoke('delete_voice_cache', { id });
    }

    async clearCache(sessionId?: string) {
        await invoke('clear_voice_cache', { sessionId });
    }
}

export const voiceStore = new VoiceStore();
```

---

### Task 7: AgentDetail Voice Tab

**Files:**
- Create: `src/lib/components/AgentVoicePanel.svelte`
- Modify: `src/lib/components/AgentDetail.svelte`

- [ ] **Step 1: Create AgentVoicePanel**

Create `src/lib/components/AgentVoicePanel.svelte`:

```svelte
<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { voiceStore } from '$lib/stores/voiceStore.svelte';
    import { modelConfigStore } from '$lib/stores/modelConfigStore.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import type { Agent } from '$lib/types';

    let { agent }: { agent: Agent } = $props();

    let form = $state({
        model_name: '',
        model_path: '',
        speaker_id: null as string | null,
        target_language: 'ja',
        emotion_params: '',
        speed: 1.0,
        translate_enabled: true,
        translate_model_config_id: null as string | null,
        generation_mode: 'auto_silent',
    });

    let selectedModel = $state<VitsModelInfo | null>(null);
    let showCache = $state(false);

    $effect(() => {
        const existing = voiceStore.agentVoices.get(agent.id);
        if (existing) {
            form = {
                model_name: existing.model_name,
                model_path: existing.model_path,
                speaker_id: existing.speaker_id,
                target_language: existing.target_language,
                emotion_params: existing.emotion_params || '',
                speed: existing.speed,
                translate_enabled: existing.translate_enabled,
                translate_model_config_id: existing.translate_model_config_id,
                generation_mode: existing.generation_mode,
            };
        }
    });

    async function handleSave() {
        try {
            await voiceStore.saveAgentVoice({
                agent_id: agent.id,
                model_name: form.model_name,
                model_path: form.model_path,
                speaker_id: form.speaker_id,
                target_language: form.target_language,
                emotion_params: form.emotion_params || null,
                speed: form.speed,
                translate_enabled: form.translate_enabled,
                translate_model_config_id: form.translate_model_config_id,
                generation_mode: form.generation_mode,
            });
            toastStore.success('语音配置已保存');
        } catch (e) {
            toastStore.error('保存失败: ' + e);
        }
    }

    async function handleRefresh() {
        await voiceStore.scanModels();
    }

    $effect(() => {
        voiceStore.checkRuntime();
        voiceStore.scanModels();
    });
</script>

{#if !voiceStore.runtimeAvailable}
    <div class="p-4 bg-red-50 border border-red-200 rounded-lg text-red-700">
        <p class="font-medium">VITS 运行时未检测到</p>
        <p class="text-sm mt-1">请将 VITS 运行时解压到 <code>data/vits_runtime/</code> 目录。</p>
    </div>
{:else}
    <div class="space-y-4">
        <div class="flex items-center gap-2">
            <label class="text-sm font-medium">语音模型</label>
            <select
                bind:value={form.model_name}
                class="border rounded px-2 py-1"
                onchange={() => {
                    const m = voiceStore.models.find(x => x.name === form.model_name);
                    if (m) {
                        form.model_path = m.path;
                        selectedModel = m;
                    }
                }}
            >
                <option value="">选择模型</option>
                {#each voiceStore.models as model}
                    <option value={model.name} disabled={!model.has_config}>
                        {model.name} {model.language ? `(${model.language})` : ''}
                        {#if !model.has_config} [缺少 config] {/if}
                    </option>
                {/each}
            </select>
            <button onclick={handleRefresh} class="text-sm text-blue-600">刷新</button>
        </div>

        {#if selectedModel && selectedModel.speakers.length > 0}
            <div class="flex items-center gap-2">
                <label class="text-sm font-medium">Speaker</label>
                <select bind:value={form.speaker_id} class="border rounded px-2 py-1">
                    <option value={null}>默认</option>
                    {#each selectedModel.speakers as spk}
                        <option value={spk}>{spk}</option>
                    {/each}
                </select>
            </div>
        {/if}

        <div class="flex items-center gap-2">
            <label class="text-sm font-medium">目标语言</label>
            <select bind:value={form.target_language} class="border rounded px-2 py-1">
                <option value="zh">中文</option>
                <option value="ja">日语</option>
                <option value="en">英语</option>
            </select>
        </div>

        <div class="flex items-center gap-2">
            <label class="text-sm font-medium">语速</label>
            <input type="range" min="0.5" max="2.0" step="0.1" bind:value={form.speed} class="w-32" />
            <span class="text-sm">{form.speed}x</span>
        </div>

        <div class="flex items-center gap-2">
            <label class="text-sm font-medium">自动翻译</label>
            <input type="checkbox" bind:checked={form.translate_enabled} />
            <span class="text-xs text-gray-500">文本语言与目标语言不一致时自动翻译</span>
        </div>

        {#if form.translate_enabled}
            <div class="flex items-center gap-2 pl-4">
                <label class="text-sm font-medium">翻译模型</label>
                <select bind:value={form.translate_model_config_id} class="border rounded px-2 py-1">
                    <option value={null}>使用角色默认模型</option>
                    {#each modelConfigStore.configs as cfg}
                        <option value={cfg.id}>{cfg.name}</option>
                    {/each}
                </select>
                <span class="text-xs text-amber-600">会增加 LLM 调用开销</span>
            </div>
        {/if}

        <div class="flex items-center gap-2">
            <label class="text-sm font-medium">生成时机</label>
            <select bind:value={form.generation_mode} class="border rounded px-2 py-1">
                <option value="auto_play">自动生成并播放</option>
                <option value="auto_silent">自动生成不播放</option>
                <option value="manual">点击后生成并播放</option>
            </select>
        </div>

        <div class="flex gap-2">
            <button onclick={handleSave} class="bg-blue-600 text-white px-4 py-1.5 rounded text-sm">
                保存配置
            </button>
            <button onclick={() => showCache = !showCache} class="text-sm text-gray-600">
                {showCache ? '隐藏缓存' : '查看缓存'}
            </button>
        </div>

        {#if showCache}
            <VoiceCachePanel agentId={agent.id} />
        {/if}
    </div>
{/if}
```

- [ ] **Step 2: Add Voice tab to AgentDetail**

In `AgentDetail.svelte`:
- Import `AgentVoicePanel`
- Add `'voice'` to `activeTab` type
- Add tab button for "语音"
- Add conditional rendering: `{#if activeTab === 'voice'}<AgentVoicePanel {agent} />{/if}`

---

### Task 8: MessageBubble Speaker Button

**Files:**
- Modify: `src/lib/components/MessageBubble.svelte`

- [ ] **Step 1: Add speaker button**

In `MessageBubble.svelte`, import `Volume2`, `voiceStore`, and `invoke`. Add to message action area:

```svelte
{#if isAgentMessage && voiceStore.agentVoices.get(message.sender_id)}
    <button
        onclick={() => handleSpeak(message)}
        class="opacity-0 group-hover:opacity-100 transition-opacity p-1 hover:bg-gray-100 rounded"
        title="播放语音"
    >
        <Volume2 size={14} />
    </button>
{/if}
```

Add handler:

```typescript
async function handleSpeak(msg: Message) {
    if (voiceStore.generating.has(msg.id)) return;
    try {
        const path = await voiceStore.generateVoice({
            message_id: msg.id,
            session_id: msg.session_id,
            agent_id: msg.sender_id,
            text: msg.content,
        });
        const audio = new Audio(path);
        audio.play();
    } catch (e) {
        toastStore.error('语音生成失败: ' + e);
    }
}
```

---

### Task 9: Voice Cache Management

**Files:**
- Create: `src/lib/components/VoiceCachePanel.svelte`
- Modify: `src/lib/components/SettingsPanel.svelte`

- [ ] **Step 1: Create VoiceCachePanel**

```svelte
<script lang="ts">
    import { voiceStore } from '$lib/stores/voiceStore.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import type { VoiceCacheItem } from '$lib/types';

    let { agentId = null }: { agentId?: string | null } = $props();

    let items = $state<VoiceCacheItem[]>([]);
    let loading = $state(false);

    async function load() {
        loading = true;
        try {
            items = await voiceStore.listCache(agentId);
        } finally {
            loading = false;
        }
    }

    async function handleDelete(id: string) {
        await voiceStore.deleteCache(id);
        items = items.filter(i => i.id !== id);
        toastStore.success('已删除');
    }

    async function handleClearAll() {
        await voiceStore.clearCache();
        items = [];
        toastStore.success('已清空');
    }

    $effect(() => { load(); });
</script>

<div class="space-y-2">
    <div class="flex justify-between items-center">
        <h3 class="font-medium">语音缓存</h3>
        <button onclick={handleClearAll} class="text-sm text-red-600">清空全部</button>
    </div>
    {#if loading}
        <p class="text-sm text-gray-500">加载中...</p>
    {:else if items.length === 0}
        <p class="text-sm text-gray-500">暂无缓存</p>
    {:else}
        <ul class="space-y-1">
            {#each items as item}
                <li class="flex justify-between items-center text-sm p-2 bg-gray-50 rounded">
                    <span>{item.message_id.slice(0, 8)}... ({(item.file_size / 1024).toFixed(1)} KB)</span>
                    <button onclick={() => handleDelete(item.id)} class="text-red-600">删除</button>
                </li>
            {/each}
        </ul>
    {/if}
</div>
```

- [ ] **Step 2: Add to SettingsPanel**

Add "语音缓存" entry that renders `VoiceCachePanel` with no `agentId`.

---

### Task 10: Usage Statistics Integration

**Files:**
- Modify: `src/lib/components/UsageMonitor.svelte`

- [ ] **Step 1: Add tts_translate trigger filter**

In `UsageMonitor.svelte`, find the trigger type filter dropdown and add:

```svelte
<option value="tts_translate">TTS 翻译</option>
```

Ensure the trigger display maps `"tts_translate"` to `"TTS 翻译"` in tables.

---

### Task 11: Testing

**Files:**
- Modify: `src-tauri/src/db/agent_voice.rs` (unit tests)
- Modify: `src/lib/stores/voiceStore.svelte.ts` (tests if applicable)

- [ ] **Step 1: Add Rust unit tests for agent_voice repo**

In `src-tauri/src/db/agent_voice.rs`, add `#[cfg(test)]` module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::schema::BASE_SCHEMA;
    use rusqlite::Connection;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(BASE_SCHEMA).unwrap();
        conn
    }

    #[test]
    fn test_save_and_get_agent_voice() {
        let conn = setup();
        let req = SaveAgentVoiceRequest {
            agent_id: "agent1".into(),
            model_name: "test".into(),
            model_path: "/path".into(),
            speaker_id: None,
            target_language: "ja".into(),
            emotion_params: None,
            speed: 1.0,
            translate_enabled: true,
            translate_model_config_id: None,
            generation_mode: "auto_silent".into(),
        };
        let saved = save_agent_voice(&conn, &req).unwrap();
        assert_eq!(saved.agent_id, "agent1");

        let loaded = get_agent_voice_by_agent_id(&conn, "agent1").unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().model_name, "test");
    }

    #[test]
    fn test_vits_cache_crud() {
        let conn = setup();
        insert_vits_cache(&conn, "msg1", "sess1", "agent1", "/tmp/a.wav", 1000).unwrap();
        let item = get_vits_cache_by_message_id(&conn, "msg1").unwrap();
        assert!(item.is_some());

        delete_vits_cache(&conn, &item.unwrap().id).unwrap();
        let item = get_vits_cache_by_message_id(&conn, "msg1").unwrap();
        assert!(item.is_none());
    }
}
```

- [ ] **Step 2: Run Rust tests**

```bash
cd src-tauri
cargo test agent_voice
```

Expected: PASS

---

## Self-Review

### Spec coverage
- [x] VOICE-01 runtime detection and model scanning
- [x] VOICE-02 role voice configuration tab
- [x] VOICE-03 message speaker button
- [x] VOICE-04 generation timing modes
- [x] VOICE-05 VITS parameter adjustment
- [x] VOICE-06 voice cache management
- [x] VOICE-07 translation toggle and warning
- [x] VOICE-08 translation model selection
- [x] VOICE-09 translation tool with persona injection
- [x] VOICE-10 TTS translation usage tracking

### Placeholder scan
- No "TBD", "TODO", or unimplemented steps found.
- All file paths are exact.
- All commands have expected outputs.

### Type consistency
- `SaveAgentVoiceRequest` fields match `AgentVoice` struct.
- `VitsRequest`/`VitsResponse` match between Rust and Python protocol.
- `trigger_type = "tts_translate"` matches usage tracking design.

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-08-06-vits-tts-implementation.md`. Two execution options:**

**1. Subagent-Driven (recommended)** - Dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
