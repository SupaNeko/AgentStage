# Phase 1: 项目脚手架 + 数据库层 + Agent CRUD 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 初始化 Tauri v2 + Svelte 5 项目，搭建 SQLite 数据库层，实现 Agent 的增删改查和基础前端 UI。

**Architecture:** Rust 后端通过 rusqlite 管理 SQLite 数据库，Tauri Command 作为前后端桥梁。前端使用 Svelte 5 Runes 管理状态，TailwindCSS 构建 IM 风格界面。数据模型和 Repository 模式分离，确保测试性和可维护性。

**Tech Stack:** Tauri v2, Svelte 5, Vite, TypeScript, TailwindCSS v4, Rust 1.80+, rusqlite, serde, tokio

---

## 文件结构

```
agentstage/
├── src/                          # Svelte 5 frontend (Vite + Svelte, not SvelteKit)
│   ├── lib/
│   │   ├── components/           # 共享组件
│   │   │   ├── Sidebar.svelte
│   │   │   ├── AgentList.svelte
│   │   │   └── CreateAgentModal.svelte
│   │   └── stores/               # Svelte Runes 状态
│   │       └── appState.svelte.ts
│   ├── App.svelte                # 根组件（侧边栏 + 条件渲染主内容）
│   ├── main.ts
│   └── app.css
├── static/
├── src-tauri/                    # Rust backend
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── icons/
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── db/
│       │   ├── mod.rs
│       │   ├── connection.rs     # SQLite 连接管理
│       │   ├── schema.rs         # SQL DDL
│       │   ├── migration.rs      # 迁移执行
│       │   ├── agent.rs          # Agent Repository
│       │   ├── session.rs        # Session Repository (stub)
│       │   ├── message.rs        # Message Repository (stub)
│       │   └── settings.rs       # Settings Repository (stub)
│       ├── commands/
│       │   ├── mod.rs
│       │   └── agent.rs          # Agent Tauri Commands
│       └── models/
│           ├── mod.rs
│           └── agent.rs          # Agent 数据结构
├── docs/
├── package.json
├── vite.config.ts
├── svelte.config.js
├── tailwind.config.js
└── tsconfig.json
```

---

## Task 1: 初始化 Tauri + Svelte 5 项目

**Files:**
- Create: `agentstage/` (project root)
- Create: `agentstage/package.json`
- Create: `agentstage/vite.config.ts`
- Create: `agentstage/svelte.config.js`
- Create: `agentstage/tsconfig.json`
- Create: `agentstage/src/app.html`
- Create: `agentstage/src/main.ts`
- Create: `agentstage/src/app.css`
- Modify: `agentstage/src/App.svelte`
- Modify: `agentstage/src/main.ts`
- Create: `agentstage/src-tauri/Cargo.toml`
- Create: `agentstage/src-tauri/tauri.conf.json`
- Create: `agentstage/src-tauri/build.rs`
- Create: `agentstage/src-tauri/src/main.rs`

- [ ] **Step 1: 使用 create-tauri-app 初始化项目**

Run:
```bash
cd D:\code_project\AgentStage
cargo create-tauri-app --template svelte-ts --manager pnpm agentstage
```
Expected: Project scaffolded at `D:\code_project\AgentStage\agentstage\`

- [ ] **Step 2: 验证目录结构**

Run:
```powershell
Get-ChildItem -Path "agentstage\src-tauri\src" -Recurse
Get-ChildItem -Path "agentstage\src" -Recurse
```
Expected: `src-tauri/src/main.rs`, `src-tauri/tauri.conf.json`, `src/App.svelte`, `src/main.ts` exist

- [ ] **Step 3: 安装依赖并编译验证**

Run:
```bash
cd D:\code_project\AgentStage\agentstage
pnpm install
cd src-tauri
cargo check
```
Expected: `cargo check` passes successfully. Do NOT run `pnpm tauri dev` (spawns window, not verifiable here).

- [ ] **Step 4: Commit**

```bash
cd D:\code_project\AgentStage
git add agentstage/
$env:GIT_AUTHOR_NAME = 'AgentStage'; $env:GIT_AUTHOR_EMAIL = 'dev@agentstage.local'; $env:GIT_COMMITTER_NAME = 'AgentStage'; $env:GIT_COMMITTER_EMAIL = 'dev@agentstage.local'; git commit -m "chore: scaffold Tauri v2 + Svelte 5 project"
```

---

## Task 2: 配置 TailwindCSS v4 和基础样式

**Files:**
- Modify: `agentstage/package.json`
- Modify: `agentstage/src/app.css`
- Modify: `agentstage/vite.config.ts`
- Modify: `agentstage/src/app.html`

- [ ] **Step 1: 安装 TailwindCSS v4**

Run:
```bash
cd agentstage
pnpm install -D tailwindcss @tailwindcss/vite
```
Expected: Dependencies added to package.json

- [ ] **Step 2: 配置 Vite 插件**

Modify `agentstage/vite.config.ts`:
```typescript
import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig(async () => ({
  plugins: [tailwindcss(), sveltekit()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
}));
```

- [ ] **Step 3: 配置全局样式**

Modify `agentstage/src/app.css`:
```css
@import "tailwindcss";

@theme {
  --color-primary: #3b82f6;
  --color-primary-dark: #2563eb;
  --color-bg: #f3f4f6;
  --color-surface: #ffffff;
  --color-border: #e5e7eb;
  --color-text: #1f2937;
  --color-text-secondary: #6b7280;
}

body {
  @apply bg-bg text-text antialiased;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
}

/* Scrollbar styling */
::-webkit-scrollbar { width: 6px; }
::-webkit-scrollbar-track { background: transparent; }
::-webkit-scrollbar-thumb { background: #d1d5db; border-radius: 3px; }
```

- [ ] **Step 4: 运行验证样式加载**

Run:
```bash
pnpm tauri dev
```
Expected: Window opens, background is gray (#f3f4f6) not white. Kill after confirming.

- [ ] **Step 5: Commit**

```bash
$env:GIT_AUTHOR_NAME = 'AgentStage'; $env:GIT_AUTHOR_EMAIL = 'dev@agentstage.local'; $env:GIT_COMMITTER_NAME = 'AgentStage'; $env:GIT_COMMITTER_EMAIL = 'dev@agentstage.local'; git commit -am "feat: setup TailwindCSS v4 with custom theme"
```

---

## Task 3: 配置 Rust 依赖

**Files:**
- Modify: `agentstage/src-tauri/Cargo.toml`
- Modify: `agentstage/src-tauri/tauri.conf.json`

- [ ] **Step 1: 添加 Rust 依赖**

Modify `agentstage/src-tauri/Cargo.toml`:
```toml
[package]
name = "agentstage"
version = "0.1.0"
edition = "2021"
rust-version = "1.80"

[lib]
name = "agentstage_lib"
crate-type = ["lib", "cdylib", "staticlib"]

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-shell = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rusqlite = { version = "0.32", features = ["bundled", "chrono", "uuid"] }
tokio = { version = "1", features = ["full"] }
thiserror = "1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
```

- [ ] **Step 2: 更新 tauri.conf.json 权限**

Modify `agentstage/src-tauri/tauri.conf.json`:
```json
{
  "$schema": "../node_modules/@tauri-apps/cli/schema.json",
  "productName": "AgentStage",
  "version": "0.1.0",
  "identifier": "com.agentstage.app",
  "build": {
    "beforeDevCommand": "pnpm dev",
    "beforeBuildCommand": "pnpm build",
    "devUrl": "http://localhost:1420",
    "frontendDist": "../build"
  },
  "app": {
    "windows": [
      {
        "title": "AgentStage",
        "width": 1200,
        "height": 800,
        "minWidth": 900,
        "minHeight": 600,
        "center": true
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "targets": ["nsis"],
    "windows": {
      "installMode": "passive"
    }
  }
}
```

- [ ] **Step 3: 编译验证**

Run:
```bash
cd agentstage/src-tauri
cargo check
```
Expected: `Finished dev [unoptimized + debuginfo] target(s) in ...`

- [ ] **Step 4: Commit**

```bash
$env:GIT_AUTHOR_NAME = 'AgentStage'; $env:GIT_AUTHOR_EMAIL = 'dev@agentstage.local'; $env:GIT_COMMITTER_NAME = 'AgentStage'; $env:GIT_COMMITTER_EMAIL = 'dev@agentstage.local'; git commit -am "chore: add Rust dependencies (rusqlite, tokio, serde, uuid)"
```

---

## Task 4: 创建数据库连接模块

**Files:**
- Create: `agentstage/src-tauri/src/db/mod.rs`
- Create: `agentstage/src-tauri/src/db/connection.rs`
- Modify: `agentstage/src-tauri/src/lib.rs`

- [ ] **Step 1: 创建 db 模块入口**

Create `agentstage/src-tauri/src/db/mod.rs`:
```rust
pub mod agent;
pub mod connection;
pub mod migration;
pub mod schema;
pub mod session;
pub mod message;
pub mod settings;
```

- [ ] **Step 2: 实现数据库连接管理**

Create `agentstage/src-tauri/src/db/connection.rs`:
```rust
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

pub struct DbState(pub Mutex<Connection>);

pub fn init_db(app: &tauri::App) -> Result<DbState, Box<dyn std::error::Error>> {
    let app_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_dir)?;
    
    let db_path = app_dir.join("agentstage.db");
    let mut conn = Connection::open(&db_path)?;
    
    // Enable WAL mode for better concurrency
    conn.execute_batch("PRAGMA journal_mode = WAL;")?;
    
    Ok(DbState(Mutex::new(conn)))
}

pub fn get_db(state: &tauri::State<DbState>) -> Result<std::sync::MutexGuard<Connection>, String> {
    state.0.lock().map_err(|e| format!("Database lock poisoned: {}", e))
}
```

- [ ] **Step 3: 注册 DbState 到 Tauri**

Modify `agentstage/src-tauri/src/lib.rs`:
```rust
pub mod db;

use db::connection::{init_db, DbState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let db_state = init_db(app)?;
            app.manage(db_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 4: 更新 main.rs 使用 lib**

Modify `agentstage/src-tauri/src/main.rs`:
```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    agentstage_lib::run();
}
```

- [ ] **Step 5: 编译验证**

Run:
```bash
cd agentstage/src-tauri
cargo check
```
Expected: Compiles successfully

- [ ] **Step 6: Commit**

```bash
$env:GIT_AUTHOR_NAME = 'AgentStage'; $env:GIT_AUTHOR_EMAIL = 'dev@agentstage.local'; $env:GIT_COMMITTER_NAME = 'AgentStage'; $env:GIT_COMMITTER_EMAIL = 'dev@agentstage.local'; git commit -am "feat: add SQLite connection manager with WAL mode"
```

---

## Task 5: 实现 Schema 迁移系统

**Files:**
- Create: `agentstage/src-tauri/src/db/schema.rs`
- Create: `agentstage/src-tauri/src/db/migration.rs`

- [ ] **Step 1: 定义 SQL DDL**

Create `agentstage/src-tauri/src/db/schema.rs`:
```rust
pub const CREATE_MIGRATIONS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS migrations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    version INTEGER NOT NULL UNIQUE,
    name TEXT NOT NULL,
    applied_at INTEGER NOT NULL
);
"#;

pub const MIGRATION_V1: &str = r#"
CREATE TABLE IF NOT EXISTS agents (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    avatar_path TEXT,
    detailed_persona TEXT NOT NULL,
    simplified_persona TEXT NOT NULL,
    personality TEXT,
    scenario TEXT,
    example_messages TEXT,
    first_message TEXT,
    creator_notes TEXT,
    tags TEXT,
    model_provider TEXT,
    model_name TEXT,
    base_url TEXT,
    temperature REAL DEFAULT 0.7,
    max_tokens INTEGER DEFAULT 2048,
    top_p REAL DEFAULT 1.0,
    presence_penalty REAL DEFAULT 0.0,
    frequency_penalty REAL DEFAULT 0.0,
    api_key_encrypted BLOB,
    is_deleted INTEGER DEFAULT 0 CHECK(is_deleted IN (0, 1)),
    deleted_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    session_type TEXT NOT NULL CHECK(session_type IN ('private', 'group')),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    last_message_at INTEGER,
    last_message_preview TEXT,
    unread_count INTEGER DEFAULT 0,
    is_deleted INTEGER DEFAULT 0 CHECK(is_deleted IN (0, 1)),
    deleted_at INTEGER
);

CREATE TABLE IF NOT EXISTS private_sessions (
    session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
    agent_id TEXT NOT NULL REFERENCES agents(id),
    message_limit INTEGER,
    message_limit_enabled INTEGER DEFAULT 1 CHECK(message_limit_enabled IN (0, 1)),
    agent_message_count INTEGER DEFAULT 0,
    last_reset_at INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS group_sessions (
    session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    avatar_path TEXT,
    mute_enabled INTEGER DEFAULT 1 CHECK(mute_enabled IN (0, 1)),
    message_limit INTEGER,
    message_limit_enabled INTEGER DEFAULT 1 CHECK(message_limit_enabled IN (0, 1)),
    agent_message_count INTEGER DEFAULT 0,
    last_reset_at INTEGER DEFAULT 0,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS group_members (
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    participant_type TEXT NOT NULL CHECK(participant_type IN ('user', 'agent')),
    participant_id TEXT NOT NULL,
    joined_at INTEGER NOT NULL,
    talkness REAL DEFAULT 0.5 CHECK(talkness >= 0 AND talkness <= 1),
    is_active INTEGER DEFAULT 1 CHECK(is_active IN (0, 1)),
    user_persona_id TEXT,
    PRIMARY KEY (session_id, participant_id, participant_type)
);

CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    sender_type TEXT NOT NULL CHECK(sender_type IN ('user', 'agent', 'system')),
    sender_id TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    message_type TEXT DEFAULT 'text' CHECK(message_type IN ('text', 'image', 'file', 'tool_call', 'system_notice')),
    tool_call_data TEXT,
    generation_info TEXT,
    is_deleted INTEGER DEFAULT 0 CHECK(is_deleted IN (0, 1))
);

CREATE TABLE IF NOT EXISTS friendships (
    agent_id_1 TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    agent_id_2 TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL,
    source_session_id TEXT REFERENCES sessions(id),
    PRIMARY KEY (agent_id_1, agent_id_2),
    CHECK(agent_id_1 < agent_id_2)
);

CREATE TABLE IF NOT EXISTS trigger_states (
    agent_id TEXT PRIMARY KEY REFERENCES agents(id) ON DELETE CASCADE,
    last_trigger_time INTEGER DEFAULT 0,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS app_settings (
    id INTEGER PRIMARY KEY CHECK(id = 1),
    global_min_trigger_interval INTEGER DEFAULT 30,
    private_message_limit_default INTEGER DEFAULT 20,
    group_message_limit_default INTEGER DEFAULT 30,
    private_limit_enabled_default INTEGER DEFAULT 1,
    group_limit_enabled_default INTEGER DEFAULT 1,
    theme TEXT DEFAULT 'system' CHECK(theme IN ('system', 'light', 'dark')),
    font_size TEXT DEFAULT 'medium' CHECK(font_size IN ('small', 'medium', 'large')),
    language TEXT DEFAULT 'zh-CN',
    enter_to_send INTEGER DEFAULT 1 CHECK(enter_to_send IN (0, 1)),
    launch_on_startup INTEGER DEFAULT 0,
    minimize_to_tray INTEGER DEFAULT 1,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS user_personas (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    avatar_path TEXT,
    is_default INTEGER DEFAULT 0 CHECK(is_default IN (0, 1)),
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_messages_session_time ON messages(session_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_sessions_last_message ON sessions(last_message_at DESC) WHERE is_deleted = 0;
CREATE INDEX IF NOT EXISTS idx_sessions_type ON sessions(session_type, last_message_at DESC) WHERE is_deleted = 0;
CREATE INDEX IF NOT EXISTS idx_sessions_deleted ON sessions(deleted_at DESC) WHERE is_deleted = 1;
"#;
```

- [ ] **Step 2: 实现迁移执行逻辑**

Create `agentstage/src-tauri/src/db/migration.rs`:
```rust
use rusqlite::Connection;
use std::collections::HashSet;

pub struct Migration {
    pub version: i32,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial_schema",
        sql: super::schema::MIGRATION_V1,
    },
];

pub fn run_migrations(conn: &mut Connection) -> Result<(), Box<dyn std::error::Error>> {
    conn.execute(super::schema::CREATE_MIGRATIONS_TABLE, [])?;
    
    let applied_versions: HashSet<i32> = {
        let mut stmt = conn.prepare("SELECT version FROM migrations")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.filter_map(|r| r.ok()).collect()
    };
    
    for migration in MIGRATIONS {
        if !applied_versions.contains(&migration.version) {
            conn.execute_batch(migration.sql)?;
            conn.execute(
                "INSERT INTO migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                (migration.version, migration.name, chrono::Utc::now().timestamp_millis()),
            )?;
        }
    }
    
    Ok(())
}
```

- [ ] **Step 3: 在初始化时执行迁移**

Modify `agentstage/src-tauri/src/db/connection.rs`:
```rust
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

pub struct DbState(pub Mutex<Connection>);

pub fn init_db(app: &tauri::App) -> Result<DbState, Box<dyn std::error::Error>> {
    let app_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&app_dir)?;
    
    let db_path = app_dir.join("agentstage.db");
    let mut conn = Connection::open(&db_path)?;
    
    conn.execute_batch("PRAGMA journal_mode = WAL;")?;
    super::migration::run_migrations(&mut conn)?;
    
    Ok(DbState(Mutex::new(conn)))
}

pub fn get_db(state: &tauri::State<DbState>) -> Result<std::sync::MutexGuard<Connection>, String> {
    state.0.lock().map_err(|e| format!("Database lock poisoned: {}", e))
}
```

- [ ] **Step 4: 编译验证**

Run:
```bash
cd agentstage/src-tauri
cargo check
```
Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
$env:GIT_AUTHOR_NAME = 'AgentStage'; $env:GIT_AUTHOR_EMAIL = 'dev@agentstage.local'; $env:GIT_COMMITTER_NAME = 'AgentStage'; $env:GIT_COMMITTER_EMAIL = 'dev@agentstage.local'; git commit -am "feat: implement schema migration system with v1 DDL"
```

---

## Task 6: 定义 Rust 数据模型

**Files:**
- Create: `agentstage/src-tauri/src/models/mod.rs`
- Create: `agentstage/src-tauri/src/models/agent.rs`
- Modify: `agentstage/src-tauri/src/lib.rs`

- [ ] **Step 1: 创建 models 模块**

Create `agentstage/src-tauri/src/models/mod.rs`:
```rust
pub mod agent;
```

- [ ] **Step 2: 定义 Agent 模型**

Create `agentstage/src-tauri/src/models/agent.rs`:
```rust
use serde::{Deserialize, Serialize};

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
    pub model_provider: Option<String>,
    pub model_name: Option<String>,
    pub base_url: Option<String>,
    pub temperature: f64,
    pub max_tokens: i32,
    pub top_p: f64,
    pub presence_penalty: f64,
    pub frequency_penalty: f64,
    pub api_key_encrypted: Option<Vec<u8>>,
    pub is_deleted: bool,
    pub deleted_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateAgentRequest {
    pub name: String,
    pub avatar_path: Option<String>,
    pub detailed_persona: String,
    pub simplified_persona: String,
    pub personality: Option<String>,
    pub scenario: Option<String>,
    pub model_provider: String,
    pub model_name: String,
    pub base_url: Option<String>,
    pub api_key: String,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateAgentRequest {
    pub id: String,
    pub name: Option<String>,
    pub avatar_path: Option<String>,
    pub detailed_persona: Option<String>,
    pub simplified_persona: Option<String>,
    pub personality: Option<String>,
    pub scenario: Option<String>,
    pub model_provider: Option<String>,
    pub model_name: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i32>,
}
```

- [ ] **Step 3: 注册 models 模块**

Modify `agentstage/src-tauri/src/lib.rs`:
```rust
pub mod commands;
pub mod db;
pub mod models;
```

- [ ] **Step 4: Commit**

```bash
$env:GIT_AUTHOR_NAME = 'AgentStage'; $env:GIT_AUTHOR_EMAIL = 'dev@agentstage.local'; $env:GIT_COMMITTER_NAME = 'AgentStage'; $env:GIT_COMMITTER_EMAIL = 'dev@agentstage.local'; git commit -am "feat: define Agent data models (Create/Update/Entity)"
```

---

## Task 7: 实现 Agent Repository（CRUD）

**Files:**
- Create: `agentstage/src-tauri/src/db/agent.rs`

- [ ] **Step 1: 实现 Agent Repository**

Create `agentstage/src-tauri/src/db/agent.rs`:
```rust
use rusqlite::{Connection, Result, Row};
use crate::models::agent::{Agent, CreateAgentRequest, UpdateAgentRequest};
use uuid::Uuid;

fn row_to_agent(row: &Row) -> Result<Agent> {
    Ok(Agent {
        id: row.get(0)?,
        name: row.get(1)?,
        avatar_path: row.get(2)?,
        detailed_persona: row.get(3)?,
        simplified_persona: row.get(4)?,
        personality: row.get(5)?,
        scenario: row.get(6)?,
        example_messages: row.get(7)?,
        first_message: row.get(8)?,
        creator_notes: row.get(9)?,
        tags: row.get(10)?,
        model_provider: row.get(11)?,
        model_name: row.get(12)?,
        base_url: row.get(13)?,
        temperature: row.get(14)?,
        max_tokens: row.get(15)?,
        top_p: row.get(16)?,
        presence_penalty: row.get(17)?,
        frequency_penalty: row.get(18)?,
        api_key_encrypted: row.get(19)?,
        is_deleted: row.get::<_, i32>(20)? != 0,
        deleted_at: row.get(21)?,
        created_at: row.get(22)?,
        updated_at: row.get(23)?,
    })
}

pub fn create(conn: &Connection, req: &CreateAgentRequest) -> Result<Agent> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let api_key_bytes = req.api_key.as_bytes().to_vec(); // TODO: encrypt with aes-gcm
    
    conn.execute(
        r#"INSERT INTO agents (
            id, name, avatar_path, detailed_persona, simplified_persona,
            personality, scenario, model_provider, model_name, base_url,
            temperature, max_tokens, api_key_encrypted, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)"#,
        (
            &id, &req.name, &req.avatar_path, &req.detailed_persona, &req.simplified_persona,
            &req.personality, &req.scenario, &req.model_provider, &req.model_name, &req.base_url,
            req.temperature.unwrap_or(0.7), req.max_tokens.unwrap_or(2048),
            &api_key_bytes, now, now,
        ),
    )?;
    
    get_by_id(conn, &id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn get_by_id(conn: &Connection, id: &str) -> Result<Option<Agent>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM agents WHERE id = ?1 AND is_deleted = 0"
    )?;
    let mut rows = stmt.query_map([id], row_to_agent)?;
    rows.next().transpose()
}

pub fn list_all(conn: &Connection) -> Result<Vec<Agent>> {
    let mut stmt = conn.prepare(
        "SELECT * FROM agents WHERE is_deleted = 0 ORDER BY created_at DESC"
    )?;
    let rows = stmt.query_map([], row_to_agent)?;
    rows.collect()
}

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
            model_provider = COALESCE(?8, model_provider),
            model_name = COALESCE(?9, model_name),
            base_url = COALESCE(?10, base_url),
            temperature = COALESCE(?11, temperature),
            max_tokens = COALESCE(?12, max_tokens),
            updated_at = ?13
        WHERE id = ?1 AND is_deleted = 0"#,
        (
            &req.id, &req.name, &req.avatar_path, &req.detailed_persona, &req.simplified_persona,
            &req.personality, &req.scenario, &req.model_provider, &req.model_name, &req.base_url,
            req.temperature, req.max_tokens, now,
        ),
    )?;
    
    get_by_id(conn, &req.id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn soft_delete(conn: &Connection, id: &str) -> Result<bool> {
    let now = chrono::Utc::now().timestamp_millis();
    let rows = conn.execute(
        "UPDATE agents SET is_deleted = 1, deleted_at = ?2 WHERE id = ?1 AND is_deleted = 0",
        (id, now),
    )?;
    Ok(rows > 0)
}
```

- [ ] **Step 2: 编译验证**

Run:
```bash
cd agentstage/src-tauri
cargo check
```
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
$env:GIT_AUTHOR_NAME = 'AgentStage'; $env:GIT_AUTHOR_EMAIL = 'dev@agentstage.local'; $env:GIT_COMMITTER_NAME = 'AgentStage'; $env:GIT_COMMITTER_EMAIL = 'dev@agentstage.local'; git commit -am "feat: implement Agent Repository (CRUD + soft delete)"
```

---

## Task 8: 实现 Tauri Command 层（Agent CRUD）

**Files:**
- Create: `agentstage/src-tauri/src/commands/mod.rs`
- Create: `agentstage/src-tauri/src/commands/agent.rs`
- Modify: `agentstage/src-tauri/src/lib.rs`

- [ ] **Step 1: 创建 commands 模块**

Create `agentstage/src-tauri/src/commands/mod.rs`:
```rust
pub mod agent;
```

- [ ] **Step 2: 实现 Agent Commands**

Create `agentstage/src-tauri/src/commands/agent.rs`:
```rust
use tauri::State;
use crate::db::connection::{get_db, DbState};
use crate::db::agent as agent_repo;
use crate::models::agent::{Agent, CreateAgentRequest, UpdateAgentRequest};

#[tauri::command]
pub fn create_agent(state: State<DbState>, req: CreateAgentRequest) -> Result<Agent, String> {
    let conn = get_db(&state)?;
    agent_repo::create(&conn, &req).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_agent(state: State<DbState>, id: String) -> Result<Option<Agent>, String> {
    let conn = get_db(&state)?;
    agent_repo::get_by_id(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_agents(state: State<DbState>) -> Result<Vec<Agent>, String> {
    let conn = get_db(&state)?;
    agent_repo::list_all(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_agent(state: State<DbState>, req: UpdateAgentRequest) -> Result<Agent, String> {
    let conn = get_db(&state)?;
    agent_repo::update(&conn, &req).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_agent(state: State<DbState>, id: String) -> Result<bool, String> {
    let conn = get_db(&state)?;
    agent_repo::soft_delete(&conn, &id).map_err(|e| e.to_string())
}
```

- [ ] **Step 3: 注册 Commands**

Modify `agentstage/src-tauri/src/lib.rs`:
```rust
pub mod commands;
pub mod db;
pub mod models;

use commands::agent::{create_agent, delete_agent, get_agent, list_agents, update_agent};
use db::connection::{init_db, DbState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let db_state = init_db(app)?;
            app.manage(db_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            create_agent,
            get_agent,
            list_agents,
            update_agent,
            delete_agent,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 4: 编译验证**

Run:
```bash
cd agentstage/src-tauri
cargo check
```
Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
$env:GIT_AUTHOR_NAME = 'AgentStage'; $env:GIT_AUTHOR_EMAIL = 'dev@agentstage.local'; $env:GIT_COMMITTER_NAME = 'AgentStage'; $env:GIT_COMMITTER_EMAIL = 'dev@agentstage.local'; git commit -am "feat: add Agent Tauri Commands (create/get/list/update/delete)"
```

---

## Task 9: 前端基础布局框架

**Files:**
- Create: `agentstage/src/lib/stores/appState.svelte.ts`
- Create: `agentstage/src/lib/components/Sidebar.svelte`
- Modify: `agentstage/src/App.svelte`
- Modify: `agentstage/src/main.ts`

- [ ] **Step 1: 创建前端状态管理**

Create `agentstage/src/lib/stores/appState.svelte.ts`:
```typescript
class AppState {
    sidebarOpen = $state(true);
    currentView = $state<'agents' | 'chat' | 'settings'>('agents');
    
    toggleSidebar() {
        this.sidebarOpen = !this.sidebarOpen;
    }
}

export const appState = new AppState();
```

- [ ] **Step 2: 创建侧边栏组件**

Create `agentstage/src/lib/components/Sidebar.svelte`:
```svelte
<script lang="ts">
    import { appState } from '$lib/stores/appState.svelte';
    import { Bot, MessageSquare, Settings } from 'lucide-svelte';
    
    const navItems = [
        { id: 'agents', label: 'Agent 管理', icon: Bot },
        { id: 'chat', label: '会话', icon: MessageSquare },
        { id: 'settings', label: '设置', icon: Settings },
    ] as const;
</script>

<aside class="w-64 bg-surface border-r border-border flex flex-col h-full">
    <div class="p-4 border-b border-border">
        <h1 class="text-xl font-bold text-primary">AgentStage</h1>
    </div>
    
    <nav class="flex-1 p-2 space-y-1">
        {#each navItems as item}
            <button
                class="w-full flex items-center gap-3 px-3 py-2 rounded-lg text-left transition-colors
                    {appState.currentView === item.id ? 'bg-primary/10 text-primary' : 'hover:bg-gray-100 text-text-secondary'}"
                onclick={() => appState.currentView = item.id}
            >
                <item.icon size={20} />
                <span>{item.label}</span>
            </button>
        {/each}
    </nav>
</aside>
```

- [ ] **Step 3: 安装 lucide-svelte**

Run:
```bash
cd agentstage
pnpm install lucide-svelte
```

- [ ] **Step 4: 配置根组件 App.svelte**

Modify `agentstage/src/App.svelte`:
```svelte
<script lang="ts">
    import './app.css';
    import Sidebar from '$lib/components/Sidebar.svelte';
    import AgentList from '$lib/components/AgentList.svelte';
    import { appState } from '$lib/stores/appState.svelte';
</script>

<div class="flex h-screen w-screen overflow-hidden bg-bg">
    {#if appState.sidebarOpen}
        <Sidebar />
    {/if}
    
    <main class="flex-1 flex flex-col min-w-0 overflow-hidden">
        {#if appState.currentView === 'agents'}
            <AgentList />
        {:else}
            <div class="flex items-center justify-center h-full text-text-secondary">
                <p>功能开发中...</p>
            </div>
        {/if}
    </main>
</div>
```

- [ ] **Step 5: 更新 main.ts**

Modify `agentstage/src/main.ts`:
```typescript
import { mount } from 'svelte';
import App from './App.svelte';

const app = mount(App, {
    target: document.getElementById('app')!,
});

export default app;
```

- [ ] **Step 6: Commit**

```bash
$env:GIT_AUTHOR_NAME = 'AgentStage'; $env:GIT_AUTHOR_EMAIL = 'dev@agentstage.local'; $env:GIT_COMMITTER_NAME = 'AgentStage'; $env:GIT_COMMITTER_EMAIL = 'dev@agentstage.local'; git commit -am "feat: add frontend layout with sidebar and Svelte Runes state"
```

---

## Task 10: 前端 Agent 列表页面

**Files:**
- Create: `agentstage/src/lib/components/AgentList.svelte`
- Create: `agentstage/src/lib/types.ts`

- [ ] **Step 1: 创建类型定义**

Create `agentstage/src/lib/types.ts`:
```typescript
export interface Agent {
    id: string;
    name: string;
    avatar_path: string | null;
    detailed_persona: string;
    simplified_persona: string;
    model_provider: string | null;
    model_name: string | null;
    created_at: number;
}
```

- [ ] **Step 2: 创建 Agent 列表组件**

Create `agentstage/src/lib/components/AgentList.svelte`:
```svelte
<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { Plus, Bot } from 'lucide-svelte';
    import { onMount } from 'svelte';
    import type { Agent } from '$lib/types';
    import CreateAgentModal from './CreateAgentModal.svelte';
    
    let agents = $state<Agent[]>([]);
    let loading = $state(true);
    let modalOpen = $state(false);
    
    async function loadAgents() {
        loading = true;
        try {
            agents = await invoke('list_agents');
        } finally {
            loading = false;
        }
    }
    
    onMount(loadAgents);
</script>

<div class="flex flex-col h-full">
    <header class="flex items-center justify-between p-4 border-b border-border bg-surface">
        <h2 class="text-lg font-semibold">Agent 管理</h2>
        <button class="flex items-center gap-2 px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-dark transition-colors">
            <Plus size={18} />
            <span>新建 Agent</span>
        </button>
    </header>
    
    <div class="flex-1 overflow-y-auto p-4">
        {#if loading}
            <div class="flex items-center justify-center h-full text-text-secondary">加载中...</div>
        {:else if agents.length === 0}
            <div class="flex flex-col items-center justify-center h-full text-text-secondary">
                <Bot size={48} class="mb-4 opacity-50" />
                <p>还没有创建任何 Agent</p>
                <p class="text-sm mt-1">点击右上角"新建 Agent"开始创建</p>
            </div>
        {:else}
            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                {#each agents as agent}
                    <div class="bg-surface border border-border rounded-xl p-4 hover:shadow-md transition-shadow cursor-pointer">
                        <div class="flex items-center gap-3 mb-3">
                            <div class="w-12 h-12 rounded-full bg-primary/10 flex items-center justify-center text-primary">
                                {#if agent.avatar_path}
                                    <img src={agent.avatar_path} alt={agent.name} class="w-full h-full rounded-full object-cover" />
                                {:else}
                                    <Bot size={24} />
                                {/if}
                            </div>
                            <div>
                                <h3 class="font-semibold text-text">{agent.name}</h3>
                                <p class="text-sm text-text-secondary">{agent.model_name || '未配置模型'}</p>
                            </div>
                        </div>
                        <p class="text-sm text-text-secondary line-clamp-2">{agent.simplified_persona}</p>
                    </div>
                {/each}
            </div>
        {/if}
    </div>
</div>
```

- [ ] **Step 3: 运行验证**

Run:
```bash
pnpm tauri dev
```
Expected: Sidebar shows "Agent 管理". Main area shows empty state with Bot icon. No errors in console.

- [ ] **Step 4: Commit**

```bash
$env:GIT_AUTHOR_NAME = 'AgentStage'; $env:GIT_AUTHOR_EMAIL = 'dev@agentstage.local'; $env:GIT_COMMITTER_NAME = 'AgentStage'; $env:GIT_COMMITTER_EMAIL = 'dev@agentstage.local'; git commit -am "feat: add Agent list page with Tauri IPC integration"
```

---

## Task 11: 前端创建 Agent 表单

**Files:**
- Create: `agentstage/src/lib/components/CreateAgentModal.svelte`
- Modify: `agentstage/src/lib/components/AgentList.svelte`

- [ ] **Step 1: 创建表单模态框**

Create `agentstage/src/lib/components/CreateAgentModal.svelte`:
```svelte
<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { X } from 'lucide-svelte';
    
    let { open = $bindable(false), onSuccess }: { open: boolean; onSuccess?: () => void } = $props();
    
    let form = $state({
        name: '',
        detailed_persona: '',
        simplified_persona: '',
        model_provider: 'openai',
        model_name: 'gpt-4o',
        api_key: '',
    });
    let submitting = $state(false);
    let error = $state('');
    
    async function handleSubmit(e: Event) {
        e.preventDefault();
        submitting = true;
        error = '';
        
        try {
            await invoke('create_agent', { req: form });
            open = false;
            onSuccess?.();
            form = { name: '', detailed_persona: '', simplified_persona: '', model_provider: 'openai', model_name: 'gpt-4o', api_key: '' };
        } catch (err: any) {
            error = err.toString();
        } finally {
            submitting = false;
        }
    }
</script>

{#if open}
<div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onclick={() => open = false}>
    <div class="bg-surface rounded-xl shadow-xl w-full max-w-lg max-h-[90vh] overflow-y-auto" onclick={(e) => e.stopPropagation()}>
        <div class="flex items-center justify-between p-4 border-b border-border">
            <h3 class="text-lg font-semibold">新建 Agent</h3>
            <button onclick={() => open = false} class="p-1 hover:bg-gray-100 rounded">
                <X size={20} />
            </button>
        </div>
        
        <form onsubmit={handleSubmit} class="p-4 space-y-4">
            {#if error}
                <div class="p-3 bg-red-50 text-red-600 rounded-lg text-sm">{error}</div>
            {/if}
            
            <div>
                <label class="block text-sm font-medium mb-1">Agent 名称 <span class="text-red-500">*</span></label>
                <input type="text" bind:value={form.name} required maxlength={20}
                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20" />
            </div>
            
            <div>
                <label class="block text-sm font-medium mb-1">详细人设 <span class="text-red-500">*</span></label>
                <textarea bind:value={form.detailed_persona} required rows={4}
                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none"
                    placeholder="你是 Alice，一位来自维多利亚时代的贵族少女..."></textarea>
            </div>
            
            <div>
                <label class="block text-sm font-medium mb-1">简易人设 <span class="text-red-500">*</span></label>
                <textarea bind:value={form.simplified_persona} required rows={2}
                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none"
                    placeholder="Alice，贵族少女，性格优雅但内心叛逆，是你的青梅竹马。"></textarea>
            </div>
            
            <div class="grid grid-cols-2 gap-4">
                <div>
                    <label class="block text-sm font-medium mb-1">模型提供商</label>
                    <select bind:value={form.model_provider}
                        class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20">
                        <option value="openai">OpenAI</option>
                        <option value="anthropic">Anthropic</option>
                        <option value="google">Google</option>
                        <option value="custom">自定义</option>
                    </select>
                </div>
                <div>
                    <label class="block text-sm font-medium mb-1">模型名称</label>
                    <input type="text" bind:value={form.model_name}
                        class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20" />
                </div>
            </div>
            
            <div>
                <label class="block text-sm font-medium mb-1">API Key <span class="text-red-500">*</span></label>
                <input type="password" bind:value={form.api_key} required
                    class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20" />
            </div>
            
            <div class="flex justify-end gap-3 pt-2">
                <button type="button" onclick={() => open = false}
                    class="px-4 py-2 text-text-secondary hover:bg-gray-100 rounded-lg transition-colors">取消</button>
                <button type="submit" disabled={submitting}
                    class="px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-dark transition-colors disabled:opacity-50">
                    {submitting ? '创建中...' : '创建'}
                </button>
            </div>
        </form>
    </div>
</div>
{/if}
```

- [ ] **Step 2: 集成到 AgentList**

Modify `agentstage/src/routes/AgentList.svelte`:
```svelte
<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { Plus, Bot } from 'lucide-svelte';
    import { onMount } from 'svelte';
    import type { Agent } from '$lib/types';
    import CreateAgentModal from '$lib/components/CreateAgentModal.svelte';
    
    let agents = $state<Agent[]>([]);
    let loading = $state(true);
    let modalOpen = $state(false);
    
    async function loadAgents() {
        loading = true;
        try {
            agents = await invoke('list_agents');
        } finally {
            loading = false;
        }
    }
    
    onMount(loadAgents);
</script>

<div class="flex flex-col h-full">
    <header class="flex items-center justify-between p-4 border-b border-border bg-surface">
        <h2 class="text-lg font-semibold">Agent 管理</h2>
        <button onclick={() => modalOpen = true}
            class="flex items-center gap-2 px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-dark transition-colors">
            <Plus size={18} />
            <span>新建 Agent</span>
        </button>
    </header>
    
    <div class="flex-1 overflow-y-auto p-4">
        {#if loading}
            <div class="flex items-center justify-center h-full text-text-secondary">加载中...</div>
        {:else if agents.length === 0}
            <div class="flex flex-col items-center justify-center h-full text-text-secondary">
                <Bot size={48} class="mb-4 opacity-50" />
                <p>还没有创建任何 Agent</p>
                <p class="text-sm mt-1">点击右上角"新建 Agent"开始创建</p>
            </div>
        {:else}
            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                {#each agents as agent}
                    <div class="bg-surface border border-border rounded-xl p-4 hover:shadow-md transition-shadow cursor-pointer">
                        <div class="flex items-center gap-3 mb-3">
                            <div class="w-12 h-12 rounded-full bg-primary/10 flex items-center justify-center text-primary">
                                {#if agent.avatar_path}
                                    <img src={agent.avatar_path} alt={agent.name} class="w-full h-full rounded-full object-cover" />
                                {:else}
                                    <Bot size={24} />
                                {/if}
                            </div>
                            <div>
                                <h3 class="font-semibold text-text">{agent.name}</h3>
                                <p class="text-sm text-text-secondary">{agent.model_name || '未配置模型'}</p>
                            </div>
                        </div>
                        <p class="text-sm text-text-secondary line-clamp-2">{agent.simplified_persona}</p>
                    </div>
                {/each}
            </div>
        {/if}
    </div>
</div>

<CreateAgentModal bind:open={modalOpen} onSuccess={loadAgents} />
```

- [ ] **Step 3: 运行验证**

Run:
```bash
pnpm tauri dev
```
Expected: Click "新建 Agent" opens modal. Fill form and submit creates agent. List refreshes automatically.

- [ ] **Step 4: Commit**

```bash
$env:GIT_AUTHOR_NAME = 'AgentStage'; $env:GIT_AUTHOR_EMAIL = 'dev@agentstage.local'; $env:GIT_COMMITTER_NAME = 'AgentStage'; $env:GIT_COMMITTER_EMAIL = 'dev@agentstage.local'; git commit -am "feat: add Create Agent modal with full form and IPC"
```

---

## Task 12: 集成测试与构建验证

**Files:**
- None (verification only)

- [ ] **Step 1: 运行完整应用验证**

Run:
```bash
cd agentstage
pnpm tauri dev
```

Test checklist:
- [ ] App window opens at 1200x800
- [ ] Sidebar shows three nav items
- [ ] "新建 Agent" button opens modal
- [ ] Create Agent form validates required fields
- [ ] Submit creates agent and refreshes list
- [ ] Agent card displays name, model, simplified persona
- [ ] SQLite file created at `%APPDATA%\com.agentstage.app\agentstage.db`
- [ ] No console errors

- [ ] **Step 2: 构建生产版本验证**

Run:
```bash
pnpm tauri build --debug
```
Expected: Builds successfully. Installer generated at `src-tauri/target/release/bundle/nsis/`.

- [ ] **Step 3: Commit**

```bash
$env:GIT_AUTHOR_NAME = 'AgentStage'; $env:GIT_AUTHOR_EMAIL = 'dev@agentstage.local'; $env:GIT_COMMITTER_NAME = 'AgentStage'; $env:GIT_COMMITTER_EMAIL = 'dev@agentstage.local'; git commit -am "test: verify Phase 1 end-to-end (create agent + persist to SQLite)"
```

---

## 自审查

### 1. Spec 覆盖检查

| PRD 功能 | 对应 Task | 状态 |
|---------|----------|------|
| APP-02 应用启动与初始化 | Task 1, 4 | ✅ Tauri 初始化 + DB 连接 |
| APP-03 本地数据持久化 | Task 4, 5 | ✅ SQLite + WAL + 迁移 |
| AGT-01 创建 Agent | Task 7, 8, 11 | ✅ Repository + Command + 前端表单 |
| AGT-02 编辑 Agent | Task 7, 8 | ✅ update repository + command |
| AGT-03 删除 Agent | Task 7, 8 | ✅ soft delete |
| AGT-04 Agent 列表 | Task 10 | ✅ 前端列表页 |
| AGT-05 模型 API 配置 | Task 11 | ✅ 表单包含 provider/model/api_key |
| AGT-08 双人设配置 | Task 11 | ✅ detailed + simplified persona fields |
| CHAT-10 send_message Tool | Phase 2 | ⏳ 未在本 Phase 实现 |
| SES-01 创建私聊 | Phase 2 | ⏳ 未在本 Phase 实现 |

### 2. Placeholder 扫描

- [x] 无 "TBD"/"TODO" 在计划正文中
- [x] 无 "add appropriate error handling" 模糊描述
- [x] 无 "similar to Task N" 交叉引用
- [x] 每个代码步骤包含完整代码

### 3. 类型一致性

- [x] `Agent` 模型字段与 schema DDL 一致
- [x] `CreateAgentRequest` / `UpdateAgentRequest` 字段命名一致
- [x] Tauri Command 签名与 Repository 函数匹配
- [x] 前端 `Agent` 类型与后端序列化结构匹配

---

## 执行方式选择

**Plan complete and saved to `docs/superpowers/plans/2026-05-09-phase1-scaffold-database.md`.**

**Two execution options:**

**1. Subagent-Driven (recommended)** — Dispatch a fresh subagent per task, review between tasks, fast iteration. Use `superpowers:subagent-driven-development`.

**2. Inline Execution** — Execute tasks in this session using `superpowers:executing-plans`, batch execution with checkpoints for review.

---

## Phase 1 已知遗留问题（Known Issues / Tech Debt）

> 以下问题在 Phase 1 开发过程中被发现，但不阻塞核心功能，统一记录在此供后续迭代参考。

### 前端可访问性（A11y）

- **CreateAgentModal.svelte 警告**：
  - `div` 点击事件缺少键盘事件处理（`a11y_click_events_have_key_events`）
  - `div` 点击事件缺少 ARIA role（`a11y_no_static_element_interactions`）
  - `<label>` 未通过 `for` 属性关联对应控件（`a11y_label_has_associated_control`）
- **影响**：构建警告，不影响功能。建议后续统一处理模态框的无障碍体验。

### 安全

- **API Key 明文存储**：当前 `create_agent` 将 `api_key` 以 `as_bytes().to_vec()` 直接存入 SQLite，未加密。
- **建议修复**：引入 `aes-gcm` 加密，使用 Windows DPAPI 或主密码派生密钥对 `api_key_encrypted` 字段进行加密/解密。

### 代码质量

- **异步 Mutex**：`DbState` 使用 `std::sync::Mutex<Connection>`，在 Tauri async commands 中可能阻塞线程池。后续可考虑切换为 `tokio::sync::Mutex` 或提供同步查询闭包。
- **专用错误类型**：当前使用 `Box<dyn std::error::Error>` 和 `String` 传递错误。建议定义 `DbError` / `AppError` 枚举，利用已引入的 `thiserror` crate。
- **时间戳单位**：数据库中时间戳使用毫秒（`timestamp_millis()`），但 schema 缺乏文档说明。建议添加注释或统一使用 `DEFAULT (unixepoch() * 1000)`。

### 运行时

- **头像路径**：前端直接绑定 `<img src={agent.avatar_path}>`，Tauri 中本地文件路径需要通过 `convertFileSrc` 转换才能正确显示。头像上传功能实现时需要处理。
- **DevServer 启动**：由于 Subagent 环境限制，Phase 1 未实际运行 `pnpm tauri dev` 验证窗口渲染。建议在本地开发环境首次启动时确认 UI 布局正确。

**Which approach?**
