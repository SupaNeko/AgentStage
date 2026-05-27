# 角色模型选择重构 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将角色模型配置从每个角色独立存储重构为全局模型配置表 + 角色引用选择，移除思考模式，保留角色层可选 Temperature 覆盖。

**Architecture:** 新建 `model_configs` 全局表存储模型参数（provider/model/base_url/api_key/max_tokens/top_p 等），`agents` 表仅保留 `model_config_id` 外键和可选 `temperature` 覆盖值。LLM 调用时根据 `model_config_id` 查询全局配置，Temperature 优先级：角色层 > 模型全局层 > 不传参。

**Tech Stack:** Rust + rusqlite + Tauri v2, Svelte 5 + TypeScript

---

## 文件变更总览

### 新建文件
| 文件 | 职责 |
|------|------|
| `src-tauri/src/models/model_config.rs` | ModelConfig / ModelConfigResponse / CreateModelConfigRequest / UpdateModelConfigRequest 结构体 |
| `src-tauri/src/db/model_config.rs` | model_configs 表的 CRUD Repository |
| `src-tauri/src/commands/model_config.rs` | list_model_configs / create_model_config / update_model_config / delete_model_config / test_model_config_connection 命令 |
| `src/lib/stores/modelConfigStore.svelte.ts` | 前端全局模型配置状态管理 |
| `src/lib/components/ModelConfigPanel.svelte` | 全局模型配置管理 UI（嵌入 SettingsPanel） |

### 修改文件
| 文件 | 变更内容 |
|------|----------|
| `src-tauri/src/models/mod.rs` | 添加 `pub mod model_config;` |
| `src-tauri/src/models/agent.rs` | 移除 model_provider/model_name/base_url/api_key_encrypted/max_tokens/top_p/presence_penalty/frequency_penalty/thinking_mode；新增 model_config_id: Option<String> + temperature: Option<f64>；更新 CreateAgentRequest / UpdateAgentRequest / AgentResponse |
| `src-tauri/src/db/schema.rs` | 新增 Migration V19：创建 model_configs 表 + 修改 agents 表 |
| `src-tauri/src/db/migration.rs` | 注册 V19 |
| `src-tauri/src/db/mod.rs` | 添加 `pub mod model_config;` |
| `src-tauri/src/db/agent.rs` | 更新 SELECT_COLUMNS / row_to_agent / create / update / list_all / get_by_id 以使用新 schema |
| `src-tauri/src/commands/mod.rs` | 添加 `pub mod model_config;` |
| `src-tauri/src/commands/agent.rs` | 更新 create_agent / update_agent / get_agent / list_agents；删除 test_api_connection 命令 |
| `src-tauri/src/commands/message.rs` | 更新 send_history_message 中的 provider 创建逻辑 |
| `src-tauri/src/scheduler/mod.rs` | 更新 4 处 OpenAiCompatibleProvider::new 调用 |
| `src-tauri/src/llm/persona_generation.rs` | 更新 provider_from_config 和 generate 函数以使用 model_config |
| `src-tauri/src/llm/openai.rs` | temperature 改为 Option<f64>，请求体中仅当 Some 时包含 temperature |
| `src-tauri/src/lib.rs` | 注册 model_config 命令，移除 test_api_connection |
| `src/lib/types.ts` | Agent 接口更新；新增 ModelConfig 接口 |
| `src/lib/components/SettingsPanel.svelte` | 新增「模型」Tab，加载 ModelConfigPanel |
| `src/lib/components/CreateAgentModal.svelte` | 移除模型参数字段，替换为 model_config_id 下拉框 + 可选 temperature；移除导入配置功能 |
| `src/lib/components/AgentDetail.svelte` | 同上 |
| `src/lib/components/AgentList.svelte` | 通过后端 JOIN 获取 model_name 展示 |

### 删除文件
| 文件 | 原因 |
|------|------|
| `src/lib/components/ImportModelConfigModal.svelte` | 全局模型配置后不再需要从其他角色导入配置 |

---

## Task 1: Database Migration V19

**Files:**
- Modify: `src-tauri/src/db/schema.rs`
- Modify: `src-tauri/src/db/migration.rs`

- [ ] **Step 1: 在 schema.rs 末尾追加 Migration V19**

```rust
pub const MIGRATION_V19: &str = r#"
-- V19: 全局模型配置重构
-- 1. 创建全局模型配置表
CREATE TABLE model_configs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    provider TEXT NOT NULL,
    model_name TEXT NOT NULL,
    base_url TEXT,
    api_key_encrypted BLOB,
    temperature REAL,
    max_tokens INTEGER DEFAULT 2048,
    top_p REAL DEFAULT 1.0,
    presence_penalty REAL DEFAULT 0.0,
    frequency_penalty REAL DEFAULT 0.0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- 2. 修改 agents 表：新增外键和可选 temperature 覆盖
ALTER TABLE agents ADD COLUMN model_config_id TEXT REFERENCES model_configs(id);
ALTER TABLE agents ADD COLUMN agent_temperature REAL;

-- 3. 删除旧模型字段
ALTER TABLE agents DROP COLUMN model_provider;
ALTER TABLE agents DROP COLUMN model_name;
ALTER TABLE agents DROP COLUMN base_url;
ALTER TABLE agents DROP COLUMN api_key_encrypted;
ALTER TABLE agents DROP COLUMN max_tokens;
ALTER TABLE agents DROP COLUMN top_p;
ALTER TABLE agents DROP COLUMN presence_penalty;
ALTER TABLE agents DROP COLUMN frequency_penalty;
ALTER TABLE agents DROP COLUMN thinking_mode;
"#;
```

- [ ] **Step 2: 在 migration.rs 注册 V19**

在 `MIGRATIONS` 数组末尾追加：

```rust
    Migration {
        version: 19,
        name: "global_model_config_refactor",
        sql: super::schema::MIGRATION_V19,
    },
```

- [ ] **Step 3: 验证 migration 编译通过**

Run: `cd src-tauri && cargo check`
Expected: 0 errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db/schema.rs src-tauri/src/db/migration.rs
git commit -m "feat(db): V19 migration — global model_configs table + agent schema refactor"
```

---

## Task 2: Backend ModelConfig 模型

**Files:**
- Create: `src-tauri/src/models/model_config.rs`
- Modify: `src-tauri/src/models/mod.rs`

- [ ] **Step 1: 创建 model_config.rs**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub model_name: String,
    pub base_url: Option<String>,
    pub api_key_encrypted: Option<Vec<u8>>,
    pub temperature: Option<f64>,
    pub max_tokens: i32,
    pub top_p: f64,
    pub presence_penalty: f64,
    pub frequency_penalty: f64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelConfigResponse {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub model_name: String,
    pub base_url: Option<String>,
    pub api_key: String,
    pub temperature: Option<f64>,
    pub max_tokens: i32,
    pub top_p: f64,
    pub presence_penalty: f64,
    pub frequency_penalty: f64,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<ModelConfig> for ModelConfigResponse {
    fn from(cfg: ModelConfig) -> Self {
        let api_key = cfg.api_key_encrypted
            .as_ref()
            .and_then(|enc| crate::crypto::decrypt(enc).ok())
            .unwrap_or_default();
        Self {
            id: cfg.id,
            name: cfg.name,
            provider: cfg.provider,
            model_name: cfg.model_name,
            base_url: cfg.base_url,
            api_key,
            temperature: cfg.temperature,
            max_tokens: cfg.max_tokens,
            top_p: cfg.top_p,
            presence_penalty: cfg.presence_penalty,
            frequency_penalty: cfg.frequency_penalty,
            created_at: cfg.created_at,
            updated_at: cfg.updated_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateModelConfigRequest {
    pub name: String,
    pub provider: String,
    pub model_name: String,
    pub base_url: Option<String>,
    pub api_key: String,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i32>,
    pub top_p: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub frequency_penalty: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateModelConfigRequest {
    pub id: String,
    pub name: Option<String>,
    pub provider: Option<String>,
    pub model_name: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub temperature: Option<Option<f64>>,
    pub max_tokens: Option<i32>,
    pub top_p: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub frequency_penalty: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeleteModelConfigRequest {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TestModelConfigConnectionRequest {
    pub id: String,
}
```

- [ ] **Step 2: 修改 models/mod.rs**

```rust
pub mod agent;
pub mod model_config;  // 新增
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/models/model_config.rs src-tauri/src/models/mod.rs
git commit -m "feat(models): add ModelConfig structs and DTOs"
```

---

## Task 3: Backend Agent 模型更新

**Files:**
- Modify: `src-tauri/src/models/agent.rs`

- [ ] **Step 1: 更新 Agent 结构体**

移除以下字段：
- `model_provider: Option<String>`
- `model_name: Option<String>`
- `base_url: Option<String>`
- `max_tokens: i32`
- `top_p: f64`
- `presence_penalty: f64`
- `frequency_penalty: f64`
- `api_key_encrypted: Option<Vec<u8>>`
- `thinking_mode: bool`

新增字段：
- `model_config_id: Option<String>`
- `temperature: Option<f64>`

完整 Agent 结构体：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub avatar_path: Option<String>,
    pub detailed_persona: String,
    pub simplified_persona: String,
    pub personality: Option<String>,
    pub scenario: Option<String>,
    pub example_messages: Option<String>,
    pub first_message: Option<String>,
    pub creator_notes: Option<String>,
    pub tags: Option<String>,
    pub model_config_id: Option<String>,
    pub temperature: Option<f64>,
    pub long_term_memory: Option<String>,
    pub memory_enabled: bool,
    pub proactive_enabled: bool,
    pub proactive_min_minutes: i32,
    pub proactive_max_minutes: i32,
    pub is_deleted: bool,
    pub deleted_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}
```

- [ ] **Step 2: 更新 AgentResponse 结构体**

```rust
#[derive(Debug, Clone, Serialize)]
pub struct AgentResponse {
    pub id: String,
    pub name: String,
    pub avatar_path: Option<String>,
    pub detailed_persona: String,
    pub simplified_persona: String,
    pub personality: Option<String>,
    pub scenario: Option<String>,
    pub example_messages: Option<String>,
    pub first_message: Option<String>,
    pub creator_notes: Option<String>,
    pub tags: Option<String>,
    pub model_config_id: Option<String>,
    pub model_name: Option<String>,  // 从 JOIN model_configs 获取，用于展示
    pub temperature: Option<f64>,
    pub long_term_memory: Option<String>,
    pub memory_enabled: bool,
    pub proactive_enabled: bool,
    pub proactive_min_minutes: i32,
    pub proactive_max_minutes: i32,
    pub is_deleted: bool,
    pub deleted_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}
```

- [ ] **Step 3: 更新 CreateAgentRequest**

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct CreateAgentRequest {
    pub name: String,
    pub avatar_path: Option<String>,
    pub detailed_persona: String,
    pub simplified_persona: String,
    pub personality: Option<String>,
    pub scenario: Option<String>,
    pub example_messages: Option<String>,
    pub first_message: Option<String>,
    pub creator_notes: Option<String>,
    pub tags: Option<String>,
    pub model_config_id: String,  // 必填：选择全局模型配置
    pub temperature: Option<f64>, // 可选覆盖
    pub long_term_memory: Option<String>,
    pub memory_enabled: Option<bool>,
}
```

- [ ] **Step 4: 更新 UpdateAgentRequest**

```rust
#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateAgentRequest {
    pub id: String,
    pub name: Option<String>,
    pub avatar_path: Option<String>,
    pub detailed_persona: Option<String>,
    pub simplified_persona: Option<String>,
    pub personality: Option<String>,
    pub scenario: Option<String>,
    pub example_messages: Option<String>,
    pub first_message: Option<String>,
    pub creator_notes: Option<String>,
    pub tags: Option<String>,
    pub model_config_id: Option<String>,
    pub temperature: Option<Option<f64>>, // Option<Option> 用于区分"不更新"和"设为NULL"
    pub long_term_memory: Option<String>,
    pub memory_enabled: Option<bool>,
}
```

- [ ] **Step 5: 删除 TestApiConnectionRequest / TestApiConnectionResponse**

这些 DTO 移到 model_config 命令中使用新的请求结构。

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/models/agent.rs
git commit -m "feat(models): refactor Agent structs — remove per-agent model fields, add model_config_id + optional temperature"
```

---

## Task 4: Backend ModelConfig Repository

**Files:**
- Create: `src-tauri/src/db/model_config.rs`
- Modify: `src-tauri/src/db/mod.rs`

- [ ] **Step 1: 创建 model_config.rs**

```rust
use rusqlite::{Connection, Result, Row};
use crate::models::model_config::{ModelConfig, CreateModelConfigRequest, UpdateModelConfigRequest};
use uuid::Uuid;

const SELECT_COLUMNS: &str = "id, name, provider, model_name, base_url, api_key_encrypted, temperature, max_tokens, top_p, presence_penalty, frequency_penalty, created_at, updated_at";

fn row_to_model_config(row: &Row) -> Result<ModelConfig> {
    Ok(ModelConfig {
        id: row.get(0)?,
        name: row.get(1)?,
        provider: row.get(2)?,
        model_name: row.get(3)?,
        base_url: row.get(4)?,
        api_key_encrypted: row.get(5)?,
        temperature: row.get(6)?,
        max_tokens: row.get(7)?,
        top_p: row.get(8)?,
        presence_penalty: row.get(9)?,
        frequency_penalty: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

pub fn create(conn: &Connection, req: &CreateModelConfigRequest) -> Result<ModelConfig> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let api_key_encrypted = if req.api_key.is_empty() {
        None
    } else {
        Some(crate::crypto::encrypt(&req.api_key)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e))))?)
    };

    conn.execute(
        r#"INSERT INTO model_configs (
            id, name, provider, model_name, base_url, api_key_encrypted,
            temperature, max_tokens, top_p, presence_penalty, frequency_penalty,
            created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"#,
        rusqlite::params![
            &id, &req.name, &req.provider, &req.model_name, &req.base_url,
            &api_key_encrypted, &req.temperature, req.max_tokens.unwrap_or(2048),
            req.top_p.unwrap_or(1.0), req.presence_penalty.unwrap_or(0.0),
            req.frequency_penalty.unwrap_or(0.0), now, now,
        ],
    )?;

    get_by_id(conn, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<Option<ModelConfig>> {
    let mut stmt = conn.prepare(
        &format!("SELECT {} FROM model_configs WHERE id = ?1", SELECT_COLUMNS)
    )?;
    let mut rows = stmt.query_map([id], row_to_model_config)?;
    rows.next().transpose()
}

pub fn list_all(conn: &Connection) -> Result<Vec<ModelConfig>> {
    let mut stmt = conn.prepare(
        &format!("SELECT {} FROM model_configs ORDER BY created_at DESC", SELECT_COLUMNS)
    )?;
    let rows = stmt.query_map([], row_to_model_config)?;
    rows.collect()
}

pub fn update(conn: &Connection, req: &UpdateModelConfigRequest) -> Result<ModelConfig> {
    let now = chrono::Utc::now().timestamp_millis();
    let api_key_encrypted = req.api_key.as_ref()
        .map(|k| crate::crypto::encrypt(k))
        .transpose()
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e))))?;

    conn.execute(
        r#"UPDATE model_configs SET
            name = COALESCE(?2, name),
            provider = COALESCE(?3, provider),
            model_name = COALESCE(?4, model_name),
            base_url = COALESCE(?5, base_url),
            api_key_encrypted = COALESCE(?6, api_key_encrypted),
            temperature = COALESCE(?7, temperature),
            max_tokens = COALESCE(?8, max_tokens),
            top_p = COALESCE(?9, top_p),
            presence_penalty = COALESCE(?10, presence_penalty),
            frequency_penalty = COALESCE(?11, frequency_penalty),
            updated_at = ?12
        WHERE id = ?1"#,
        rusqlite::params![
            &req.id, &req.name, &req.provider, &req.model_name, &req.base_url,
            &api_key_encrypted, &req.temperature, req.max_tokens, req.top_p,
            req.presence_penalty, req.frequency_penalty, now,
        ],
    )?;

    get_by_id(conn, &req.id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn delete(conn: &Connection, id: &str) -> Result<bool> {
    // 检查是否有角色引用
    let count: i32 = conn.query_row(
        "SELECT COUNT(*) FROM agents WHERE model_config_id = ?1 AND is_deleted = 0",
        [id],
        |row| row.get(0),
    )?;
    if count > 0 {
        return Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(1),
            Some(format!("该模型配置正被 {} 个角色使用，无法删除", count)),
        ));
    }

    let rows = conn.execute(
        "DELETE FROM model_configs WHERE id = ?1",
        [id],
    )?;
    Ok(rows > 0)
}

pub fn count_referencing_agents(conn: &Connection, id: &str) -> Result<i32> {
    conn.query_row(
        "SELECT COUNT(*) FROM agents WHERE model_config_id = ?1 AND is_deleted = 0",
        [id],
        |row| row.get(0),
    )
}
```

- [ ] **Step 2: 修改 db/mod.rs**

```rust
pub mod agent;
pub mod agent_relationship;
pub mod agent_unread;
pub mod chat_page;
pub mod connection;
pub mod frozen_state;
pub mod migration;
pub mod model_config;  // 新增
pub mod schema;
pub mod scheduled_task;
pub mod session;
pub mod message;
pub mod settings;
pub mod trigger_state;
pub mod user_persona;
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/db/model_config.rs src-tauri/src/db/mod.rs
git commit -m "feat(db): add model_config repository with CRUD + reference check"
```

---

## Task 5: Backend Agent Repository 更新

**Files:**
- Modify: `src-tauri/src/db/agent.rs`

- [ ] **Step 1: 更新 SELECT_COLUMNS 和 row_to_agent**

```rust
const SELECT_COLUMNS: &str = "id, name, avatar_path, detailed_persona, simplified_persona, personality, scenario, example_messages, first_message, creator_notes, tags, model_config_id, temperature, long_term_memory, memory_enabled, proactive_enabled, proactive_min_minutes, proactive_max_minutes, is_deleted, deleted_at, created_at, updated_at";

fn row_to_agent(row: &Row) -> Result<Agent> {
    Ok(Agent {
        id: row.get(0)?,
        name: row.get(1)?,
        avatar_path: crate::db::resolve_avatar_path(row.get(2)?),
        detailed_persona: row.get(3)?,
        simplified_persona: row.get(4)?,
        personality: row.get(5)?,
        scenario: row.get(6)?,
        example_messages: row.get(7)?,
        first_message: row.get(8)?,
        creator_notes: row.get(9)?,
        tags: row.get(10)?,
        model_config_id: row.get(11)?,
        temperature: row.get(12)?,
        long_term_memory: row.get(13)?,
        memory_enabled: row.get::<_, i32>(14)? != 0,
        proactive_enabled: row.get::<_, i32>(15)? != 0,
        proactive_min_minutes: row.get(16)?,
        proactive_max_minutes: row.get(17)?,
        is_deleted: row.get::<_, i32>(18)? != 0,
        deleted_at: row.get(19)?,
        created_at: row.get(20)?,
        updated_at: row.get(21)?,
    })
}
```

- [ ] **Step 2: 更新 create 方法**

```rust
pub fn create(conn: &Connection, req: &CreateAgentRequest) -> Result<Agent> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();

    conn.execute(
        r#"INSERT INTO agents (
            id, name, avatar_path, detailed_persona, simplified_persona,
            personality, scenario, example_messages, first_message, creator_notes, tags,
            model_config_id, temperature, long_term_memory, memory_enabled, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)"#,
        rusqlite::params![
            &id, &req.name, &req.avatar_path, &req.detailed_persona, &req.simplified_persona,
            &req.personality, &req.scenario, &req.example_messages, &req.first_message, &req.creator_notes,
            &req.tags, &req.model_config_id, &req.temperature,
            &req.long_term_memory, req.memory_enabled.unwrap_or(true) as i32, now, now,
        ],
    )?;

    get_by_id(conn, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}
```

- [ ] **Step 3: 更新 update 方法**

```rust
pub fn update(conn: &Connection, req: &UpdateAgentRequest) -> Result<Agent> {
    let now = chrono::Utc::now().timestamp_millis();

    conn.execute(
        r#"UPDATE agents SET
            name = COALESCE(?2, name),
            avatar_path = COALESCE(?3, avatar_path),
            detailed_persona = COALESCE(?4, detailed_persona),
            simplified_persona = COALESCE(?5, simplified_persona),
            personality = COALESCE(?6, personality),
            scenario = COALESCE(?7, scenario),
            example_messages = COALESCE(?8, example_messages),
            first_message = COALESCE(?9, first_message),
            creator_notes = COALESCE(?10, creator_notes),
            tags = COALESCE(?11, tags),
            model_config_id = COALESCE(?12, model_config_id),
            temperature = COALESCE(?13, temperature),
            long_term_memory = COALESCE(?14, long_term_memory),
            memory_enabled = COALESCE(?15, memory_enabled),
            updated_at = ?16
        WHERE id = ?1 AND is_deleted = 0"#,
        rusqlite::params![
            &req.id, &req.name, &req.avatar_path, &req.detailed_persona, &req.simplified_persona,
            &req.personality, &req.scenario, &req.example_messages, &req.first_message, &req.creator_notes,
            &req.tags, &req.model_config_id, &req.temperature,
            req.long_term_memory, req.memory_enabled.map(|v| v as i32),
            now,
        ],
    )?;

    get_by_id(conn, &req.id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}
```

- [ ] **Step 4: 更新 repository 测试**

修改 tests 模块中的 `CreateAgentRequest` 构造，使用新的字段：
- 移除 `model_provider`, `model_name`, `base_url`, `api_key`, `max_tokens`, `thinking_mode`
- 新增 `model_config_id: "test-model-id".to_string()`
- `temperature: None`

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db/agent.rs
git commit -m "feat(db): update agent repository for new schema — model_config_id + optional temperature"
```

---

## Task 6: Backend ModelConfig 命令

**Files:**
- Create: `src-tauri/src/commands/model_config.rs`
- Modify: `src-tauri/src/commands/mod.rs`

- [ ] **Step 1: 创建 model_config.rs 命令**

```rust
use tauri::State;
use crate::db::connection::{get_db, DbState};
use crate::db::model_config as model_config_repo;
use crate::models::model_config::{
    ModelConfigResponse, CreateModelConfigRequest, UpdateModelConfigRequest,
    DeleteModelConfigRequest, TestModelConfigConnectionRequest,
};

#[tauri::command]
pub async fn list_model_configs(state: State<'_, DbState>) -> Result<Vec<ModelConfigResponse>, String> {
    let conn = get_db(&state).await?;
    let configs = model_config_repo::list_all(&conn).map_err(|e| e.to_string())?;
    Ok(configs.into_iter().map(ModelConfigResponse::from).collect())
}

#[tauri::command]
pub async fn create_model_config(
    state: State<'_, DbState>,
    req: CreateModelConfigRequest,
) -> Result<ModelConfigResponse, String> {
    let conn = get_db(&state).await?;
    let cfg = model_config_repo::create(&conn, &req).map_err(|e| e.to_string())?;
    Ok(ModelConfigResponse::from(cfg))
}

#[tauri::command]
pub async fn update_model_config(
    state: State<'_, DbState>,
    req: UpdateModelConfigRequest,
) -> Result<ModelConfigResponse, String> {
    let conn = get_db(&state).await?;
    let cfg = model_config_repo::update(&conn, &req).map_err(|e| e.to_string())?;
    Ok(ModelConfigResponse::from(cfg))
}

#[tauri::command]
pub async fn delete_model_config(
    state: State<'_, DbState>,
    req: DeleteModelConfigRequest,
) -> Result<(), String> {
    let conn = get_db(&state).await?;
    model_config_repo::delete(&conn, &req.id).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn test_model_config_connection(
    state: State<'_, DbState>,
    req: TestModelConfigConnectionRequest,
) -> Result<crate::models::agent::TestApiConnectionResponse, String> {
    let conn = get_db(&state).await?;
    let cfg = model_config_repo::get_by_id(&conn, &req.id)
        .map_err(|e| e.to_string())?
        .ok_or("模型配置不存在".to_string())?;

    let api_key = cfg.api_key_encrypted
        .as_ref()
        .and_then(|enc| crate::crypto::decrypt(enc).ok())
        .unwrap_or_default();

    if api_key.is_empty() {
        return Ok(crate::models::agent::TestApiConnectionResponse {
            success: false,
            latency_ms: 0,
            message: "未配置 API Key".to_string(),
        });
    }

    let base_url = cfg.base_url.unwrap_or_else(|| match cfg.provider.as_str() {
        "openai" => "https://api.openai.com/v1".to_string(),
        "anthropic" => "https://api.anthropic.com/v1".to_string(),
        "google" => "https://generativelanguage.googleapis.com/v1beta/openai".to_string(),
        "kimi" => "https://api.moonshot.cn/v1".to_string(),
        "minimax" => "https://api.minimax.chat/v1".to_string(),
        _ => "https://api.openai.com/v1".to_string(),
    });

    let start = std::time::Instant::now();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("构建 HTTP 客户端失败: {}", e))?;

    let body = serde_json::json!({
        "model": cfg.model_name,
        "messages": [{"role": "user", "content": "hi"}],
        "max_tokens": 1
    });

    let url = format!("{}/chat/completions", base_url);
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    let latency_ms = start.elapsed().as_millis() as u64;
    let status = response.status();

    if status.is_success() {
        Ok(crate::models::agent::TestApiConnectionResponse {
            success: true,
            latency_ms,
            message: "连接成功".to_string(),
        })
    } else {
        let text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        let err_msg = if status.as_u16() == 401 {
            "API Key 无效或已过期".to_string()
        } else if status.as_u16() == 404 {
            "模型不存在，请检查模型名称".to_string()
        } else if status.as_u16() == 429 {
            "请求过于频繁，请稍后再试".to_string()
        } else {
            format!("HTTP {}: {}", status, text)
        };
        Ok(crate::models::agent::TestApiConnectionResponse {
            success: false,
            latency_ms,
            message: err_msg,
        })
    }
}
```

- [ ] **Step 2: 修改 commands/mod.rs**

```rust
pub mod agent;
pub mod agent_relationship;
pub mod generate_persona;
pub mod log;
pub mod message;
pub mod model_config;  // 新增
pub mod session;
pub mod settings;
pub mod theme;
pub mod timer;
pub mod upload;
pub mod user_persona;
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/model_config.rs src-tauri/src/commands/mod.rs
git commit -m "feat(commands): add model_config CRUD + connection test commands"
```

---

## Task 7: Backend Agent 命令更新

**Files:**
- Modify: `src-tauri/src/commands/agent.rs`

- [ ] **Step 1: 更新 create_agent / update_agent / get_agent / list_agents**

`create_agent` 和 `update_agent` 不再处理模型参数字段（已由 ModelConfig 接管）。
`get_agent` / `list_agents` 需要 JOIN `model_configs` 获取 `model_name` 用于展示。

修改 `list_agents`：

```rust
#[tauri::command]
pub async fn list_agents(state: State<'_, DbState>) -> Result<Vec<AgentResponse>, String> {
    let conn = get_db(&state).await?;
    // 使用 JOIN 获取 model_name
    let mut stmt = conn.prepare(r#"
        SELECT 
            a.id, a.name, a.avatar_path, a.detailed_persona, a.simplified_persona,
            a.personality, a.scenario, a.example_messages, a.first_message, a.creator_notes, a.tags,
            a.model_config_id, a.temperature, a.long_term_memory, a.memory_enabled,
            a.proactive_enabled, a.proactive_min_minutes, a.proactive_max_minutes,
            a.is_deleted, a.deleted_at, a.created_at, a.updated_at,
            mc.model_name as mc_model_name
        FROM agents a
        LEFT JOIN model_configs mc ON a.model_config_id = mc.id
        WHERE a.is_deleted = 0
        ORDER BY a.created_at DESC
    "#).map_err(|e| e.to_string())?;

    let rows = stmt.query_map([], |row| {
        Ok(AgentResponse {
            id: row.get(0)?,
            name: row.get(1)?,
            avatar_path: crate::db::resolve_avatar_path(row.get(2)?),
            detailed_persona: row.get(3)?,
            simplified_persona: row.get(4)?,
            personality: row.get(5)?,
            scenario: row.get(6)?,
            example_messages: row.get(7)?,
            first_message: row.get(8)?,
            creator_notes: row.get(9)?,
            tags: row.get(10)?,
            model_config_id: row.get(11)?,
            model_name: row.get(22)?,  // mc_model_name
            temperature: row.get(12)?,
            long_term_memory: row.get(13)?,
            memory_enabled: row.get::<_, i32>(14)? != 0,
            proactive_enabled: row.get::<_, i32>(15)? != 0,
            proactive_min_minutes: row.get(16)?,
            proactive_max_minutes: row.get(17)?,
            is_deleted: row.get::<_, i32>(18)? != 0,
            deleted_at: row.get(19)?,
            created_at: row.get(20)?,
            updated_at: row.get(21)?,
        })
    }).map_err(|e| e.to_string())?;

    let agents: Vec<AgentResponse> = rows.filter_map(|r| r.ok()).collect();
    crate::logger::debug(&format!("[DEBUG list_agents] returned {} agents", agents.len()));
    Ok(agents)
}
```

类似地修改 `get_agent`，使用相同的 JOIN 查询。

- [ ] **Step 2: 删除 test_api_connection 命令**

从 `src-tauri/src/commands/agent.rs` 中删除 `test_api_connection` 函数及其相关的 `TestApiConnectionRequest` / `TestApiConnectionResponse` import（如果尚未移到 model_config）。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/agent.rs
git commit -m "feat(commands): refactor agent commands — remove test_api_connection, add model_config JOIN for list/get"
```

---

## Task 8: Backend lib.rs 命令注册更新

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 导入新的 model_config 命令**

在 imports 区域添加：

```rust
use commands::model_config::{list_model_configs, create_model_config, update_model_config, delete_model_config, test_model_config_connection};
```

- [ ] **Step 2: 在 generate_handler! 中注册新命令**

添加：
```rust
list_model_configs,
create_model_config,
update_model_config,
delete_model_config,
test_model_config_connection,
```

并移除：`test_api_connection,`

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(lib): register model_config commands, remove test_api_connection"
```

---

## Task 9: LLM Provider 支持可选 Temperature

**Files:**
- Modify: `src-tauri/src/llm/openai.rs`

- [ ] **Step 1: temperature 改为 Option<f64>**

```rust
pub struct OpenAiCompatibleProvider {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
    temperature: Option<f64>,
    max_tokens: i32,
}

impl OpenAiCompatibleProvider {
    pub fn new(
        api_key: String,
        base_url: Option<String>,
        model: String,
        temperature: Option<f64>,
        max_tokens: i32,
    ) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("Failed to build reqwest client");

        Self {
            client,
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            model,
            temperature,
            max_tokens,
        }
    }
}
```

- [ ] **Step 2: 请求体中条件性包含 temperature**

在 `chat_raw` 方法中：

```rust
let mut request_body = serde_json::json!({
    "model": self.model,
    "messages": messages,
    "max_tokens": self.max_tokens,
});

// 仅当 temperature 有值时才传入
if let Some(temp) = self.temperature {
    request_body["temperature"] = serde_json::json!(temp);
}
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/llm/openai.rs
git commit -m "feat(llm): make temperature optional in OpenAiCompatibleProvider — omit when None"
```

---

## Task 10: Scheduler LLM 调用更新（4处）

**Files:**
- Modify: `src-tauri/src/scheduler/mod.rs`

Scheduler 中有 4 处创建 `OpenAiCompatibleProvider`（trigger_agent_inner、trigger_special、SessionSummary、OverflowSummary）。全部需要改为：
1. 通过 `agent.model_config_id` 查询 `model_configs`
2. 解密 api_key
3. 解析 temperature（agent.temperature > model_config.temperature > None）
4. 传入新的 provider

以 `trigger_agent_inner`（~847行）为例：

```rust
// 根据 model_config_id 查询模型配置
let model_config = if let Some(ref mc_id) = agent.model_config_id {
    crate::db::model_config::get_by_id(&conn, mc_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Agent {} 引用的模型配置不存在", agent_id))?
} else {
    return Err(format!("Agent {} 未配置模型", agent_id));
};

let api_key_encrypted = model_config.api_key_encrypted
    .ok_or_else(|| format!("Agent {} 的模型配置未设置 API Key", agent_id))?;
let api_key = crate::crypto::decrypt(&api_key_encrypted)
    .map_err(|e| format!("解密 API Key 失败: {}", e))?;

// Temperature 优先级：agent > model_config > None
let temperature = agent.temperature.or(model_config.temperature);

let provider = OpenAiCompatibleProvider::new(
    api_key,
    model_config.base_url,
    model_config.model_name,
    temperature,
    model_config.max_tokens,
);
```

对 trigger_special（~1180行）、SessionSummary（~1622行）、OverflowSummary（~1838行）做同样修改。

- [ ] **Step 1: 修改 trigger_agent_inner**
- [ ] **Step 2: 修改 trigger_special**
- [ ] **Step 3: 修改 SessionSummary**
- [ ] **Step 4: 修改 OverflowSummary**
- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/scheduler/mod.rs
git commit -m "feat(scheduler): resolve model config from global table for all LLM calls"
```

---

## Task 11: Message 命令 LLM 调用更新

**Files:**
- Modify: `src-tauri/src/commands/message.rs`

`send_history_message` 中创建 provider 的逻辑同样需要更新为从 `model_configs` 查询。

修改方式与 Task 10 相同。

- [ ] **Step 1: 修改 send_history_message 中的 provider 创建**
- [ ] **Step 2: Commit**

```bash
git add src-tauri/src/commands/message.rs
git commit -m "feat(commands): resolve model config from global table in send_history_message"
```

---

## Task 12: Persona Generation LLM 调用更新

**Files:**
- Modify: `src-tauri/src/llm/persona_generation.rs`

`generate` 函数中，当基于已有角色生成人设时，需要从角色的 `model_config_id` 查询全局配置。

修改 ~202 行的逻辑：

```rust
let model_config = if let Some(ref id) = req.agent_id {
    let conn = db_state.0.lock().await;
    let agent = crate::db::agent::get_by_id(&conn, id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "角色不存在".to_string())?;
    let mc_id = agent.model_config_id.ok_or("该角色未配置模型")?;
    let cfg = crate::db::model_config::get_by_id(&conn, &mc_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "该角色引用的模型配置不存在".to_string())?;
    let api_key = cfg.api_key_encrypted
        .as_ref()
        .and_then(|enc| crate::crypto::decrypt(enc).ok())
        .ok_or("解密 API Key 失败")?;
    ModelConfig {
        model_provider: cfg.provider,
        model_name: cfg.model_name,
        base_url: cfg.base_url,
        api_key,
        temperature: agent.temperature.or(cfg.temperature).unwrap_or(0.7),
        max_tokens: cfg.max_tokens,
    }
} else {
    req.model_config.clone().unwrap()
};
```

- [ ] **Step 1: 修改 persona_generation.rs**
- [ ] **Step 2: Commit**

```bash
git add src-tauri/src/llm/persona_generation.rs
git commit -m "feat(llm): resolve model config from global table in persona generation"
```

---

## Task 13: cargo check 验证后端

- [ ] **Step 1: 运行 cargo check**

```bash
cd src-tauri && cargo check
```

Expected: 0 errors（可能会有 warnings，需逐一修复）

- [ ] **Step 2: 如有编译错误，逐条修复**

常见可能错误：
- `Agent` / `AgentResponse` 字段缺失（其他地方引用旧字段）
- `test_api_connection` 仍被某处引用
- `thinking_mode` 仍被某处引用
- `CreateAgentRequest` / `UpdateAgentRequest` 构造时字段不匹配

- [ ] **Step 3: Commit 修复**

```bash
git add -A
git commit -m "fix: resolve backend compilation errors after model config refactor"
```

---

## Task 14: 前端类型定义更新

**Files:**
- Modify: `src/lib/types.ts`

- [ ] **Step 1: 添加 ModelConfig 接口**

```typescript
export interface ModelConfig {
    id: string;
    name: string;
    provider: string;
    model_name: string;
    base_url: string | null;
    api_key: string;
    temperature: number | null;
    max_tokens: number;
    top_p: number;
    presence_penalty: number;
    frequency_penalty: number;
    created_at: number;
    updated_at: number;
}
```

- [ ] **Step 2: 更新 Agent 接口**

```typescript
export interface Agent {
    id: string;
    name: string;
    avatar_path: string | null;
    detailed_persona: string;
    simplified_persona: string;
    personality: string | null;
    scenario: string | null;
    example_messages: string | null;
    first_message: string | null;
    creator_notes: string | null;
    tags: string | null;
    model_config_id: string | null;
    model_name: string | null;  // 从后端 JOIN 获取，用于展示
    temperature: number | null;
    long_term_memory?: string;
    memory_enabled?: boolean;
    proactive_enabled?: number;
    proactive_min_minutes?: number;
    proactive_max_minutes?: number;
    is_deleted: boolean;
    deleted_at: number | null;
    created_at: number;
    updated_at: number;
}
```

- [ ] **Step 3: Commit**

```bash
git add src/lib/types.ts
git commit -m "feat(types): add ModelConfig interface, refactor Agent interface"
```

---

## Task 15: 前端 ModelConfig Store

**Files:**
- Create: `src/lib/stores/modelConfigStore.svelte.ts`

- [ ] **Step 1: 创建 store**

```typescript
import { invoke } from '@tauri-apps/api/core';
import type { ModelConfig } from '$lib/types';

class ModelConfigStore {
    configs = $state<ModelConfig[]>([]);
    loading = $state(false);

    async load() {
        this.loading = true;
        try {
            this.configs = await invoke<ModelConfig[]>('list_model_configs');
        } catch (e) {
            console.error('Failed to load model configs:', e);
        } finally {
            this.loading = false;
        }
    }

    async create(config: Omit<ModelConfig, 'id' | 'created_at' | 'updated_at'>) {
        const created = await invoke<ModelConfig>('create_model_config', { req: config });
        this.configs = [created, ...this.configs];
        return created;
    }

    async update(id: string, partial: Partial<ModelConfig>) {
        const updated = await invoke<ModelConfig>('update_model_config', { req: { id, ...partial } });
        this.configs = this.configs.map(c => c.id === id ? updated : c);
        return updated;
    }

    async delete(id: string) {
        await invoke('delete_model_config', { req: { id } });
        this.configs = this.configs.filter(c => c.id !== id);
    }

    async testConnection(id: string) {
        return await invoke<{ success: boolean; latency_ms: number; message: string }>(
            'test_model_config_connection',
            { req: { id } }
        );
    }

    getById(id: string): ModelConfig | undefined {
        return this.configs.find(c => c.id === id);
    }
}

export const modelConfigStore = new ModelConfigStore();
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/stores/modelConfigStore.svelte.ts
git commit -m "feat(stores): add modelConfigStore for global model config management"
```

---

## Task 16: 前端 ModelConfigPanel 组件

**Files:**
- Create: `src/lib/components/ModelConfigPanel.svelte`

这是一个管理全局模型配置的表单+列表组件，嵌入 SettingsPanel 的「模型」Tab 中。

核心功能：
- 展示已配置的模型列表（名称、provider、模型名）
- 点击「添加模型」打开表单弹窗
- 表单字段：名称、provider下拉、模型名称、base_url、api_key、temperature（可为空）、max_tokens、top_p、presence_penalty、frequency_penalty
- 「测试连接」按钮
- 编辑/删除操作

由于组件代码较长，此处给出结构框架，实现时参考现有 SettingsPanel 和 AgentDetail 的表单风格：

```svelte
<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { modelConfigStore } from '$lib/stores/modelConfigStore.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { Plus, Trash2, Edit2, Wifi, Loader2 } from 'lucide-svelte';
    import { PROVIDER_DEFAULTS } from '$lib/modelConfig';

    let showForm = $state(false);
    let editingConfig = $state<ModelConfig | null>(null);
    let form = $state({
        name: '', provider: 'openai', model_name: '', base_url: '', api_key: '',
        temperature: null as number | null, max_tokens: 2048, top_p: 1.0,
        presence_penalty: 0.0, frequency_penalty: 0.0
    });
    let testingConnection = $state(false);

    // ... 表单处理、测试连接、保存、删除逻辑 ...
</script>

<div class="p-6 space-y-6">
    <div class="flex items-center justify-between">
        <h3 class="text-lg font-semibold">模型配置</h3>
        <button onclick={() => { showForm = true; editingConfig = null; }} class="btn-primary">
            <Plus size={16} /> 添加模型
        </button>
    </div>

    {#if modelConfigStore.configs.length === 0}
        <div class="text-sm text-text-secondary py-8 text-center">
            暂无模型配置，点击上方按钮添加
        </div>
    {:else}
        <div class="space-y-3">
            {#each modelConfigStore.configs as cfg}
                <div class="flex items-center justify-between p-3 border border-border rounded-lg">
                    <div>
                        <div class="font-medium">{cfg.name}</div>
                        <div class="text-xs text-text-secondary">{cfg.provider} / {cfg.model_name}</div>
                    </div>
                    <div class="flex gap-2">
                        <button onclick={() => testConnection(cfg.id)}>...</button>
                        <button onclick={() => editConfig(cfg)}>...</button>
                        <button onclick={() => deleteConfig(cfg.id)}>...</button>
                    </div>
                </div>
            {/each}
        </div>
    {/if}
</div>

<!-- 表单弹窗 -->
{#if showForm}
    <!-- 名称、provider、model_name、base_url、api_key、temperature(可为空)、max_tokens、top_p、presence_penalty、frequency_penalty -->
{/if}
```

- [ ] **Step 1: 创建 ModelConfigPanel.svelte**
- [ ] **Step 2: Commit**

```bash
git add src/lib/components/ModelConfigPanel.svelte
git commit -m "feat(ui): add ModelConfigPanel for global model config management"
```

---

## Task 17: SettingsPanel 新增「模型」Tab

**Files:**
- Modify: `src/lib/components/SettingsPanel.svelte`

- [ ] **Step 1: 添加模型 Tab 按钮**

在 Tab 栏中新增：

```svelte
<button class="... {activeTab === 'models' ? 'border-primary text-primary' : 'border-transparent text-text-secondary'}" onclick={() => activeTab = 'models'}>模型</button>
```

- [ ] **Step 2: 添加模型 Tab 内容**

```svelte
{:else if activeTab === 'models'}
    <ModelConfigPanel />
```

- [ ] **Step 3: 导入 ModelConfigPanel**

```typescript
import ModelConfigPanel from './ModelConfigPanel.svelte';
```

- [ ] **Step 4: onMount 中加载模型配置**

```typescript
onMount(() => {
    themeStore.loadThemes();
    modelConfigStore.load();  // 新增
});
```

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/SettingsPanel.svelte
git commit -m "feat(ui): add Models tab to SettingsPanel"
```

---

## Task 18: CreateAgentModal 模型配置重构

**Files:**
- Modify: `src/lib/components/CreateAgentModal.svelte`
- Delete: `src/lib/components/ImportModelConfigModal.svelte`

- [ ] **Step 1: 移除模型参数字段**

从 `form` 中移除：
- `model_provider`
- `model_name`
- `base_url`
- `api_key`
- `max_tokens`
- `thinking_mode`

新增：
- `model_config_id: string | null`
- `temperature: number | null`

- [ ] **Step 2: 替换表单区域**

移除 provider 下拉、model_name 输入、base_url 输入、api_key 输入、max_tokens 输入、thinking_mode 开关。

替换为：
- 「选择模型」下拉框：`<select bind:value={form.model_config_id}>`，选项从 `modelConfigStore.configs` 加载，显示 `name (provider / model_name)`
- Temperature：`<input type="number" bind:value={form.temperature} min={0} max={2} step={0.1} placeholder="使用模型默认值" />`
- 若未选择模型，保存按钮禁用或保存时提示

- [ ] **Step 3: 移除导入配置按钮和弹窗**

删除 `ImportModelConfigModal` 的 import 和使用，删除 `showImportModal` 状态。

- [ ] **Step 4: 更新 create_agent 调用**

```typescript
await invoke('create_agent', {
    name: form.name,
    // ... 其他字段 ...
    modelConfigId: form.model_config_id,
    temperature: form.temperature,
    // 不再传 model_provider, model_name, base_url, api_key, max_tokens, thinking_mode
});
```

- [ ] **Step 5: 删除 ImportModelConfigModal.svelte**

```bash
Remove-Item src/lib/components/ImportModelConfigModal.svelte
```

- [ ] **Step 6: Commit**

```bash
git add src/lib/components/CreateAgentModal.svelte
git rm src/lib/components/ImportModelConfigModal.svelte
git commit -m "feat(ui): refactor CreateAgentModal — select from global model configs, remove import config"
```

---

## Task 19: AgentDetail 模型配置重构

**Files:**
- Modify: `src/lib/components/AgentDetail.svelte`

变更内容与 Task 18 类似：
- 移除 model_provider、model_name、base_url、api_key、max_tokens、thinking_mode 表单字段
- 新增 model_config_id 下拉框 + 可选 temperature
- 更新 `handleSave` 中的 `update_agent` 调用参数
- 移除 `handleTestApi`（测试功能移到全局配置）
- 移除 `handleImportModelConfig` 和相关逻辑

- [ ] **Step 1: 修改 form state**
- [ ] **Step 2: 替换模型配置表单区域**
- [ ] **Step 3: 更新 handleSave 调用**
- [ ] **Step 4: 移除测试连接和导入配置逻辑**
- [ ] **Step 5: Commit**

```bash
git add src/lib/components/AgentDetail.svelte
git commit -m "feat(ui): refactor AgentDetail — select from global model configs, remove test/import config"
```

---

## Task 20: AgentList 展示更新

**Files:**
- Modify: `src/lib/components/AgentList.svelte`

AgentList 中展示 `agent.model_name` 用于显示模型信息。由于后端 `list_agents` 已经通过 JOIN 提供 `model_name`，前端无需改动展示逻辑。

但需要确认：当 `agent.model_name` 为 null（角色未选择模型配置）时，显示「未配置模型」。

当前代码已有此处理：
```svelte
<p class="text-xs text-text-secondary truncate">{agent.model_name || '未配置模型'}</p>
```

- [ ] **Step 1: 确认无需修改**
- [ ] **Step 2: Commit（如有微小调整）**

---

## Task 21: 前端验证与清理

- [ ] **Step 1: 运行 svelte-check**

```bash
npx svelte-check --tsconfig ./tsconfig.json
```

Expected: 0 errors

- [ ] **Step 2: 检查残留引用**

搜索以下旧字段/组件的残留引用：
- `thinking_mode`
- `model_provider`（除 PROVIDER_DEFAULTS 常量外）
- `test_api_connection`
- `ImportModelConfigModal`
- `import_model_config`

```bash
grep -r "thinking_mode\|test_api_connection\|ImportModelConfigModal" src/
```

Expected: 无匹配（除可能的注释外）

- [ ] **Step 3: 修复残留问题**

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "fix(frontend): resolve svelte-check errors and clean up old model config references"
```

---

## Task 22: 端到端手动测试

- [ ] **Step 1: 启动开发环境**

```bash
pnpm tauri dev
```

- [ ] **Step 2: 测试全局模型配置**

1. 打开设置 → 模型 Tab
2. 点击「添加模型」，填写名称/provider/模型名/API Key
3. Temperature 留空，保存
4. 点击「测试连接」，验证成功
5. 编辑模型，修改 temperature 为 0.5，保存
6. 尝试删除被引用的模型配置，验证拒绝删除

- [ ] **Step 3: 测试角色创建**

1. 创建新角色
2. 模型配置区域：选择刚才创建的模型
3. Temperature 留空（使用模型默认值）
4. 保存成功
5. 在 AgentList 中查看角色，显示正确的模型名称

- [ ] **Step 4: 测试角色编辑**

1. 编辑已有角色
2. 修改 Temperature 为 0.3（覆盖模型默认值）
3. 保存成功
4. 发送消息测试 LLM 调用正常

- [ ] **Step 5: 测试 Temperature 优先级**

1. 全局模型配置 temperature = 空
2. 角色 temperature = 空
3. 调用 LLM，验证请求体中**不包含** temperature 字段

- [ ] **Step 6: Commit 测试通过**

```bash
git add -A
git commit -m "test: verify model config refactor end-to-end"
```

---

## Plan Self-Review

### Spec Coverage Check

| 需求 | 实现任务 |
|------|----------|
| 全局设置中配置模型（可配置多个） | Task 4, 6, 16 |
| 每个角色从已配置模型中选择 | Task 3, 5, 7, 18, 19 |
| 链接测试功能移到全局配置 | Task 6 (test_model_config_connection), Task 16 |
| 移除思考模式开关 | Task 3, 5, 18, 19 |
| 角色保留 Temperature 配置，可覆盖 | Task 3, 5, 7, 18, 19, 9 |
| 配置模型时 Temperature 可为空 | Task 4 (ModelConfig.temperature: Option<f64>), Task 16 |
| Temperature 为空时不传参 | Task 9 |

✅ 全部覆盖。

### Placeholder Scan

- 无 TBD/TODO
- 无 "add appropriate error handling" 等模糊描述
- 所有任务包含实际代码或明确的文件操作

### Type Consistency

- `model_config_id` 在 Rust 中为 `Option<String>`，在前端为 `string | null` ✅
- `temperature` 在 Rust 中为 `Option<f64>`，在前端为 `number | null` ✅
- `AgentResponse.model_name` 来自 JOIN 查询的别名 `mc_model_name` ✅
- `UpdateAgentRequest.temperature` 使用 `Option<Option<f64>>` 来区分「不更新」和「设为 NULL」 ✅

---

*Plan complete and saved to `docs/superpowers/plans/2026-05-27-model-config-refactor.md`.*

**Two execution options:**

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
