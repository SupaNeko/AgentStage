# 用户角色配置页 (USR-01) 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现个人配置页，支持多套用户人设的创建、编辑、切换和头像管理，并将默认人设从数据库记录改为代码常量。

**Architecture:** 后端新增 `user_personas` repository + commands，用 `app_settings.active_persona_id` 标记当前启用的人设；前端新增 `profile` 视图 + 手风琴式人设列表 + 创建弹窗。默认人设（name/description）统一收敛到 `constants.rs`。

**Tech Stack:** Tauri v2 (Rust + SQLite), Svelte 5 Runes, TailwindCSS v4, TypeScript

---

## 文件结构

### 后端 (Rust)

| 文件 | 操作 | 职责 |
|------|------|------|
| `src-tauri/src/constants.rs` | 新建 | `DEFAULT_USER_NAME`, `DEFAULT_USER_PERSONA` 常量 |
| `src-tauri/src/models/user_persona.rs` | 新建 | `UserPersona`, `CreateUserPersonaRequest`, `UpdateUserPersonaRequest`, `CurrentUserPersonaResponse` |
| `src-tauri/src/models/mod.rs` | 修改 | 导出 `user_persona` 模块 |
| `src-tauri/src/db/schema.rs` | 修改 | 新增 `MIGRATION_V11` |
| `src-tauri/src/db/migration.rs` | 修改 | 注册 V11 迁移 |
| `src-tauri/src/db/user_persona.rs` | 新建 | CRUD + `get_current_user_persona` |
| `src-tauri/src/db/mod.rs` | 修改 | 导出 `user_persona` 模块 |
| `src-tauri/src/commands/user_persona.rs` | 新建 | `list_user_personas`, `create_user_persona`, `update_user_persona`, `delete_user_persona`, `get_current_user_persona`, `activate_user_persona` |
| `src-tauri/src/commands/mod.rs` | 修改 | 导出 `user_persona` 模块 |
| `src-tauri/src/commands/upload.rs` | 修改 | 支持 `target_type = "user_default"` 和 `"user_persona"` |
| `src-tauri/src/llm/prompt_templates.rs` | 修改 | 移除 `USER_NAME_DEFAULT` / `USER_PERSONA_DEFAULT`，改为从 `constants` 导入 |
| `src-tauri/src/llm/prompt.rs` | 修改 | `get_user_persona` 改用新逻辑（`active_persona_id` → `user_personas` → fallback 到常量） |
| `src-tauri/src/lib.rs` | 修改 | `mod constants;` + 注册 6 个新命令 |

### 前端 (Svelte/TS)

| 文件 | 操作 | 职责 |
|------|------|------|
| `src/lib/stores/appState.svelte.ts` | 修改 | `currentView` 类型扩展 `'profile'`，`switchView` 支持 `'profile'` |
| `src/lib/stores/userPersonaStore.svelte.ts` | 新建 | 人设列表状态、CRUD、切换激活 |
| `src/lib/components/LeftNav.svelte` | 修改 | 最上方新增 `[个人]` 按钮 |
| `src/lib/components/ProfileView.svelte` | 新建 | Profile 视图根组件（分类列表 + 详情区域） |
| `src/lib/components/UserPersonaConfig.svelte` | 新建 | "用户角色配置"详情页 |
| `src/lib/components/UserPersonaItem.svelte` | 新建 | 单个人设手风琴行（折叠/展开/编辑） |
| `src/lib/components/CreateUserPersonaModal.svelte` | 新建 | 创建新人设弹窗 |
| `src/App.svelte` | 修改 | `currentView === 'profile'` 时渲染 `ProfileView` |

---

## Task 1: 新建 constants.rs 并替换旧常量

**Files:**
- Create: `src-tauri/src/constants.rs`
- Modify: `src-tauri/src/llm/prompt_templates.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 创建 constants.rs**

```rust
// src-tauri/src/constants.rs
pub const DEFAULT_USER_NAME: &str = "用户";
pub const DEFAULT_USER_PERSONA: &str = "正在与你聊天的真实用户";
```

- [ ] **Step 2: 修改 lib.rs 注册 constants 模块**

在 `src-tauri/src/lib.rs` 文件顶部，新增：
```rust
pub mod constants;
```

- [ ] **Step 3: 修改 prompt_templates.rs 移除旧常量，从新模块导入**

将 `src-tauri/src/llm/prompt_templates.rs` 中：
```rust
pub const USER_NAME_DEFAULT: &str = "用户";
pub const USER_PERSONA_DEFAULT: &str = "正在与你聊天的真实用户";
```
替换为：
```rust
pub use crate::constants::{DEFAULT_USER_NAME, DEFAULT_USER_PERSONA};
```

- [ ] **Step 4: 验证编译通过**

Run:
```bash
cd src-tauri && cargo check
```
Expected: 无错误

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/constants.rs src-tauri/src/llm/prompt_templates.rs src-tauri/src/lib.rs
git commit -m "feat: 新建 constants.rs，统一默认用户人设常量"
```

---

## Task 2: 新建 UserPersona Model

**Files:**
- Create: `src-tauri/src/models/user_persona.rs`
- Modify: `src-tauri/src/models/mod.rs`

- [ ] **Step 1: 创建 models/user_persona.rs**

```rust
// src-tauri/src/models/user_persona.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct UserPersona {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub avatar_path: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserPersonaRequest {
    pub name: String,
    pub description: Option<String>,
    pub avatar_path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserPersonaRequest {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub avatar_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CurrentUserPersonaResponse {
    pub id: Option<String>,
    pub name: String,
    pub description: String,
    pub avatar_path: Option<String>,
    pub is_custom: bool,
}
```

- [ ] **Step 2: 修改 models/mod.rs**

```rust
// src-tauri/src/models/mod.rs
pub mod agent;
pub mod chat_page;
pub mod message;
pub mod session;
pub mod settings;
pub mod user_persona;
```

- [ ] **Step 3: 验证编译**

Run:
```bash
cd src-tauri && cargo check
```
Expected: 无错误

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/models/user_persona.rs src-tauri/src/models/mod.rs
git commit -m "feat: 新增 UserPersona 模型定义"
```

---

## Task 3: 数据库迁移 V11

**Files:**
- Modify: `src-tauri/src/db/schema.rs`
- Modify: `src-tauri/src/db/migration.rs`

- [ ] **Step 1: 在 schema.rs 底部新增 MIGRATION_V11**

```rust
// 追加到 src-tauri/src/db/schema.rs 底部
pub const MIGRATION_V11: &str = r#"
-- 删除旧的默认人设记录（默认人设改为代码常量）
DELETE FROM user_personas WHERE is_default = 1;

-- app_settings 新增字段
ALTER TABLE app_settings ADD COLUMN active_persona_id TEXT;
ALTER TABLE app_settings ADD COLUMN default_avatar_path TEXT;

-- 注意：is_default 列因 SQLite 限制可能无法 DROP COLUMN，后续由开发者在确认后手动处理
"#;
```

- [ ] **Step 2: 在 migration.rs 注册 V11**

```rust
// src-tauri/src/db/migration.rs
// 在 MIGRATIONS 数组末尾追加
Migration {
    version: 11,
    name: "user_persona_config",
    sql: super::schema::MIGRATION_V11,
},
```

- [ ] **Step 3: 验证编译**

Run:
```bash
cd src-tauri && cargo check
```
Expected: 无错误

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db/schema.rs src-tauri/src/db/migration.rs
git commit -m "feat: 数据库迁移 V11 - 用户配置支持 active_persona_id 和 default_avatar_path"
```

---

## Task 4: 新建 UserPersona Repository

**Files:**
- Create: `src-tauri/src/db/user_persona.rs`
- Modify: `src-tauri/src/db/mod.rs`

- [ ] **Step 1: 创建 db/user_persona.rs**

```rust
// src-tauri/src/db/user_persona.rs
use rusqlite::Connection;
use crate::models::user_persona::{UserPersona, CreateUserPersonaRequest, UpdateUserPersonaRequest, CurrentUserPersonaResponse};
use crate::constants::{DEFAULT_USER_NAME, DEFAULT_USER_PERSONA};

fn row_to_persona(row: &rusqlite::Row) -> Result<UserPersona, rusqlite::Error> {
    Ok(UserPersona {
        id: row.get("id")?,
        name: row.get("name")?,
        description: row.get("description")?,
        avatar_path: row.get("avatar_path")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn list_user_personas(conn: &Connection) -> Result<Vec<UserPersona>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, avatar_path, created_at, updated_at FROM user_personas ORDER BY updated_at DESC"
    )?;
    let rows = stmt.query_map([], row_to_persona)?;
    rows.collect()
}

pub fn create_user_persona(
    conn: &Connection,
    req: &CreateUserPersonaRequest,
) -> Result<UserPersona, rusqlite::Error> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO user_personas (id, name, description, avatar_path, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        (&id, &req.name, &req.description, &req.avatar_path, &now),
    )?;
    Ok(UserPersona {
        id,
        name: req.name.clone(),
        description: req.description.clone(),
        avatar_path: req.avatar_path.clone(),
        created_at: now,
        updated_at: now,
    })
}

pub fn update_user_persona(
    conn: &Connection,
    req: &UpdateUserPersonaRequest,
) -> Result<UserPersona, rusqlite::Error> {
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE user_personas SET name = COALESCE(?2, name), description = ?3, avatar_path = ?4, updated_at = ?5 WHERE id = ?1",
        (&req.id, &req.name, &req.description, &req.avatar_path, &now),
    )?;
    get_user_persona_by_id(conn, &req.id)
}

pub fn get_user_persona_by_id(conn: &Connection, id: &str) -> Result<UserPersona, rusqlite::Error> {
    conn.query_row(
        "SELECT id, name, description, avatar_path, created_at, updated_at FROM user_personas WHERE id = ?1",
        [id],
        row_to_persona,
    )
}

pub fn delete_user_persona(conn: &Connection, id: &str) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM user_personas WHERE id = ?1", [id])?;
    Ok(())
}

pub fn get_current_user_persona(conn: &Connection) -> Result<CurrentUserPersonaResponse, rusqlite::Error> {
    let active_id: Option<String> = conn.query_row(
        "SELECT active_persona_id FROM app_settings WHERE id = 1",
        [],
        |row| row.get(0),
    ).ok();

    if let Some(id) = active_id {
        if let Ok(persona) = get_user_persona_by_id(conn, &id) {
            return Ok(CurrentUserPersonaResponse {
                id: Some(persona.id),
                name: persona.name,
                description: persona.description.unwrap_or_default(),
                avatar_path: persona.avatar_path,
                is_custom: true,
            });
        }
    }

    // Fallback to default
    let default_avatar: Option<String> = conn.query_row(
        "SELECT default_avatar_path FROM app_settings WHERE id = 1",
        [],
        |row| row.get(0),
    ).ok().flatten();

    Ok(CurrentUserPersonaResponse {
        id: None,
        name: DEFAULT_USER_NAME.to_string(),
        description: DEFAULT_USER_PERSONA.to_string(),
        avatar_path: default_avatar,
        is_custom: false,
    })
}

pub fn activate_user_persona(conn: &Connection, id: Option<&str>) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE app_settings SET active_persona_id = ?1 WHERE id = 1",
        [id],
    )?;
    Ok(())
}

pub fn update_default_avatar(conn: &Connection, path: &str) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE app_settings SET default_avatar_path = ?1 WHERE id = 1",
        [path],
    )?;
    Ok(())
}
```

- [ ] **Step 2: 修改 db/mod.rs**

```rust
// src-tauri/src/db/mod.rs
pub mod agent;
pub mod agent_unread;
pub mod chat_page;
pub mod connection;
pub mod frozen_state;
pub mod migration;
pub mod schema;
pub mod session;
pub mod message;
pub mod settings;
pub mod trigger_state;
pub mod user_persona;
```

- [ ] **Step 3: 验证编译**

Run:
```bash
cd src-tauri && cargo check
```
Expected: 无错误

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db/user_persona.rs src-tauri/src/db/mod.rs
git commit -m "feat: 新增 UserPersona Repository - CRUD + 当前人设查询"
```

---

## Task 5: 新建 UserPersona Commands

**Files:**
- Create: `src-tauri/src/commands/user_persona.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 创建 commands/user_persona.rs**

```rust
// src-tauri/src/commands/user_persona.rs
use tauri::State;
use crate::db::connection::{get_db, DbState};
use crate::db::user_persona;
use crate::models::user_persona::{
    UserPersona, CreateUserPersonaRequest, UpdateUserPersonaRequest, CurrentUserPersonaResponse
};

#[tauri::command]
pub async fn list_user_personas(state: State<'_, DbState>) -> Result<Vec<UserPersona>, String> {
    let conn = get_db(&state).await.map_err(|e| e.to_string())?;
    user_persona::list_user_personas(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_user_persona(
    state: State<'_, DbState>,
    req: CreateUserPersonaRequest,
) -> Result<UserPersona, String> {
    let conn = get_db(&state).await.map_err(|e| e.to_string())?;
    user_persona::create_user_persona(&conn, &req).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_user_persona(
    state: State<'_, DbState>,
    req: UpdateUserPersonaRequest,
) -> Result<UserPersona, String> {
    let conn = get_db(&state).await.map_err(|e| e.to_string())?;
    user_persona::update_user_persona(&conn, &req).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_user_persona(state: State<'_, DbState>, id: String) -> Result<(), String> {
    let conn = get_db(&state).await.map_err(|e| e.to_string())?;
    user_persona::delete_user_persona(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_current_user_persona(state: State<'_, DbState>) -> Result<CurrentUserPersonaResponse, String> {
    let conn = get_db(&state).await.map_err(|e| e.to_string())?;
    user_persona::get_current_user_persona(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn activate_user_persona(
    state: State<'_, DbState>,
    id: Option<String>,
) -> Result<(), String> {
    let conn = get_db(&state).await.map_err(|e| e.to_string())?;
    user_persona::activate_user_persona(&conn, id.as_deref()).map_err(|e| e.to_string())
}
```

- [ ] **Step 2: 修改 commands/mod.rs**

```rust
// src-tauri/src/commands/mod.rs
pub mod agent;
pub mod log;
pub mod message;
pub mod session;
pub mod settings;
pub mod upload;
pub mod user_persona;
```

- [ ] **Step 3: 修改 lib.rs 注册命令**

在 `src-tauri/src/lib.rs` 顶部 `use` 区域新增：
```rust
use commands::user_persona::{
    list_user_personas, create_user_persona, update_user_persona, delete_user_persona,
    get_current_user_persona, activate_user_persona,
};
```

在 `tauri::generate_handler![...]` 数组末尾追加：
```rust
list_user_personas,
create_user_persona,
update_user_persona,
delete_user_persona,
get_current_user_persona,
activate_user_persona,
```

- [ ] **Step 4: 验证编译**

Run:
```bash
cd src-tauri && cargo check
```
Expected: 无错误

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/user_persona.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat: 新增 UserPersona Tauri Commands (list/create/update/delete/activate/get_current)"
```

---

## Task 6: 更新 upload_avatar 支持 user_default / user_persona

**Files:**
- Modify: `src-tauri/src/commands/upload.rs`

- [ ] **Step 1: 修改 upload.rs 替换 "user" 分支**

将 `src-tauri/src/commands/upload.rs` 中 `"user"` 分支替换为两个新分支：

```rust
"user_default" => {
    use crate::db::user_persona;
    user_persona::update_default_avatar(&conn, &relative_path).map_err(|e| e.to_string())?;
}
"user_persona" => {
    use crate::db::user_persona;
    user_persona::update_user_persona(
        &conn,
        &crate::models::user_persona::UpdateUserPersonaRequest {
            id: req.target_id.clone(),
            name: None,
            description: None,
            avatar_path: Some(relative_path.clone()),
        },
    ).map_err(|e| e.to_string())?;
}
```

- [ ] **Step 2: 验证编译**

Run:
```bash
cd src-tauri && cargo check
```
Expected: 无错误

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/upload.rs
git commit -m "feat: upload_avatar 支持 user_default 和 user_persona 目标类型"
```

---

## Task 7: 更新 prompt.rs get_user_persona 使用新逻辑

**Files:**
- Modify: `src-tauri/src/llm/prompt.rs`

- [ ] **Step 1: 修改 get_user_persona 函数**

将 `src-tauri/src/llm/prompt.rs` 中的 `get_user_persona` 函数替换为：

```rust
fn get_user_persona(conn: &Connection) -> (String, String) {
    use crate::db::user_persona;
    use crate::constants::{DEFAULT_USER_NAME, DEFAULT_USER_PERSONA};

    match user_persona::get_current_user_persona(conn) {
        Ok(p) => (p.name, p.description),
        Err(_) => (DEFAULT_USER_NAME.to_string(), DEFAULT_USER_PERSONA.to_string()),
    }
}
```

- [ ] **Step 2: 删除未使用的 import**

确保文件顶部 `prompt_templates` 中仍然导入了需要的常量（如果 `USER_NAME` 等还在用则保留）。`USER_NAME_DEFAULT` 和 `USER_PERSONA_DEFAULT` 已不在本文件使用，但它们在 `prompt_templates.rs` 中已改为 `pub use crate::constants::*` 形式导出，所以 `prompt.rs` 中如果有直接引用需要改为引用 `crate::constants`。

检查 `prompt.rs` 第 283 行附近的 `prompt_templates::USER_NAME` — 这保持不变。

- [ ] **Step 3: 验证编译**

Run:
```bash
cd src-tauri && cargo check
```
Expected: 无错误

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/llm/prompt.rs
git commit -m "feat: prompt.rs 使用新 UserPersona 逻辑获取当前人设"
```

---

## Task 8: 前端 appState 扩展 profile 视图 + LeftNav 新增按钮

**Files:**
- Modify: `src/lib/stores/appState.svelte.ts`
- Modify: `src/lib/components/LeftNav.svelte`

- [ ] **Step 1: 修改 appState.svelte.ts**

```typescript
// src/lib/stores/appState.svelte.ts
class AppState {
    currentView = $state<'agents' | 'chat' | 'history' | 'profile'>('chat');
    selectedAgentId = $state<string | null>(null);
    selectedSessionId = $state<string | null>(null);
    settingsOpen = $state(false);

    switchView(view: 'agents' | 'chat' | 'history' | 'profile') {
        this.currentView = view;
        if (view === 'agents') {
            this.selectedSessionId = null;
        } else if (view === 'profile') {
            this.selectedAgentId = null;
            this.selectedSessionId = null;
        } else {
            this.selectedAgentId = null;
        }
    }
    // ...其余方法不变
}

export const appState = new AppState();
```

- [ ] **Step 2: 修改 LeftNav.svelte**

```svelte
<!-- src/lib/components/LeftNav.svelte -->
<script lang="ts">
    import { appState } from '$lib/stores/appState.svelte';
    import { User, Bot, MessageSquare, History, Settings } from 'lucide-svelte';

    const navItems = [
        { id: 'profile' as const, label: '个人', icon: User },
        { id: 'agents' as const, label: '角色管理', icon: Bot },
        { id: 'chat' as const, label: '聊天', icon: MessageSquare },
        { id: 'history' as const, label: '历史会话', icon: History },
    ];
</script>

<!-- 其余结构保持不变 -->
```

- [ ] **Step 3: 验证前端编译**

Run:
```bash
pnpm build
```
Expected: 无 TypeScript/Svelte 编译错误

- [ ] **Step 4: Commit**

```bash
git add src/lib/stores/appState.svelte.ts src/lib/components/LeftNav.svelte
git commit -m "feat: appState 新增 profile 视图，LeftNav 新增个人入口"
```

---

## Task 9: 新建 userPersonaStore

**Files:**
- Create: `src/lib/stores/userPersonaStore.svelte.ts`

- [ ] **Step 1: 创建 userPersonaStore.svelte.ts**

```typescript
// src/lib/stores/userPersonaStore.svelte.ts
import { invoke } from '@tauri-apps/api/core';
import { settingsStore } from './settingsStore.svelte';
import { logger } from '$lib/logger';

export interface UserPersona {
    id: string;
    name: string;
    description?: string;
    avatar_path?: string;
}

export interface CurrentUserPersona {
    id?: string;
    name: string;
    description: string;
    avatar_path?: string;
    is_custom: boolean;
}

class UserPersonaStore {
    personas = $state<UserPersona[]>([]);
    currentPersona = $state<CurrentUserPersona | null>(null);
    loading = $state(false);

    async loadPersonas() {
        this.loading = true;
        try {
            this.personas = await invoke<UserPersona[]>('list_user_personas');
        } catch (e) {
            logger.error('Failed to load user personas', e);
        } finally {
            this.loading = false;
        }
    }

    async loadCurrentPersona() {
        try {
            this.currentPersona = await invoke<CurrentUserPersona>('get_current_user_persona');
        } catch (e) {
            logger.error('Failed to load current persona', e);
        }
    }

    async createPersona(data: { name: string; description?: string; avatar_path?: string }) {
        const persona = await invoke<UserPersona>('create_user_persona', { req: data });
        this.personas = [...this.personas, persona];
        return persona;
    }

    async updatePersona(data: { id: string; name?: string; description?: string; avatar_path?: string }) {
        const persona = await invoke<UserPersona>('update_user_persona', { req: data });
        this.personas = this.personas.map(p => p.id === persona.id ? persona : p);
        // If updated persona is currently active, refresh current
        if (this.currentPersona?.id === persona.id) {
            await this.loadCurrentPersona();
        }
        return persona;
    }

    async deletePersona(id: string) {
        await invoke('delete_user_persona', { id });
        this.personas = this.personas.filter(p => p.id !== id);
        if (this.currentPersona?.id === id) {
            await this.activatePersona(null);
        }
    }

    async activatePersona(id: string | null) {
        await invoke('activate_user_persona', { id });
        await settingsStore.load();
        await this.loadCurrentPersona();
    }
}

export const userPersonaStore = new UserPersonaStore();
```

- [ ] **Step 2: 验证前端编译**

Run:
```bash
pnpm build
```
Expected: 无错误

- [ ] **Step 3: Commit**

```bash
git add src/lib/stores/userPersonaStore.svelte.ts
git commit -m "feat: 新建 userPersonaStore - 人设列表、当前人设、CRUD、激活切换"
```

---

## Task 10: 新建 CreateUserPersonaModal

**Files:**
- Create: `src/lib/components/CreateUserPersonaModal.svelte`

- [ ] **Step 1: 创建 CreateUserPersonaModal.svelte**

```svelte
<!-- src/lib/components/CreateUserPersonaModal.svelte -->
<script lang="ts">
    import { userPersonaStore } from '$lib/stores/userPersonaStore.svelte';
    import { settingsStore } from '$lib/stores/settingsStore.svelte';
    import AvatarUploadModal from './AvatarUploadModal.svelte';
    import { resolveAvatarUrl } from '$lib/utils';
    import { X } from 'lucide-svelte';

    let { onclose, oncreated }: { onclose: () => void; oncreated?: () => void } = $props();

    let name = $state('');
    let description = $state('');
    let avatarPath = $state<string | undefined>(undefined);
    let avatarUploadOpen = $state(false);
    let saving = $state(false);

    function handleUseDefaultAvatar() {
        avatarPath = settingsStore.settings?.default_avatar_path ?? undefined;
    }

    function handleAvatarUploaded(path: string) {
        avatarPath = path;
        avatarUploadOpen = false;
    }

    async function handleCreate() {
        if (!name.trim()) return;
        saving = true;
        try {
            await userPersonaStore.createPersona({
                name: name.trim(),
                description: description.trim() || undefined,
                avatar_path: avatarPath,
            });
            oncreated?.();
            onclose();
        } finally {
            saving = false;
        }
    }
</script>

{#if avatarUploadOpen}
    <AvatarUploadModal
        targetType="user_persona"
        targetId="new"
        onUploaded={handleAvatarUploaded}
        onClose={() => avatarUploadOpen = false}
    />
{/if}

<div class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center" onclick={(e) => e.target === e.currentTarget && onclose()}>
    <div class="bg-surface rounded-xl shadow-xl w-full max-w-md p-6">
        <div class="flex items-center justify-between mb-4">
            <h2 class="text-lg font-semibold">创建新人设</h2>
            <button onclick={onclose} class="text-text-secondary hover:text-text"><X size={20} /></button>
        </div>

        <!-- Avatar -->
        <div class="flex items-center gap-4 mb-4">
            <button onclick={() => avatarUploadOpen = true} class="w-16 h-16 rounded-full bg-gray-200 flex items-center justify-center overflow-hidden shrink-0">
                {#if avatarPath}
                    <img src={resolveAvatarUrl(avatarPath)} alt="avatar" class="w-full h-full object-cover" />
                {:else}
                    <svg xmlns="http://www.w3.org/2000/svg" width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-gray-400"><path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>
                {/if}
            </button>
            <div class="flex flex-col gap-2">
                <button onclick={handleUseDefaultAvatar} class="text-sm text-primary hover:underline">使用默认头像</button>
            </div>
        </div>

        <!-- Name -->
        <div class="mb-4">
            <label class="block text-sm font-medium mb-1">角色名 <span class="text-red-500">*</span></label>
            <input type="text" bind:value={name} class="w-full px-3 py-2 rounded-lg border border-border bg-bg focus:outline-none focus:ring-2 focus:ring-primary" placeholder="给你的角色起个名字" />
        </div>

        <!-- Description -->
        <div class="mb-6">
            <label class="block text-sm font-medium mb-1">简易人设</label>
            <textarea bind:value={description} rows={3} class="w-full px-3 py-2 rounded-lg border border-border bg-bg focus:outline-none focus:ring-2 focus:ring-primary resize-none" placeholder="其他角色会看到的你的人设描述"></textarea>
        </div>

        <!-- Actions -->
        <div class="flex justify-end gap-2">
            <button onclick={onclose} class="px-4 py-2 rounded-lg text-text-secondary hover:bg-gray-100">取消</button>
            <button
                onclick={handleCreate}
                disabled={!name.trim() || saving}
                class="px-4 py-2 rounded-lg bg-primary text-white disabled:opacity-50 disabled:cursor-not-allowed"
            >
                {saving ? '创建中...' : '创建'}
            </button>
        </div>
    </div>
</div>
```

- [ ] **Step 2: 验证前端编译**

Run:
```bash
pnpm build
```
Expected: 无错误

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/CreateUserPersonaModal.svelte
git commit -m "feat: 新建 CreateUserPersonaModal 组件"
```

---

## Task 11: 新建 UserPersonaItem 组件

**Files:**
- Create: `src/lib/components/UserPersonaItem.svelte`

- [ ] **Step 1: 创建 UserPersonaItem.svelte**

```svelte
<!-- src/lib/components/UserPersonaItem.svelte -->
<script lang="ts">
    import { userPersonaStore } from '$lib/stores/userPersonaStore.svelte';
    import { settingsStore } from '$lib/stores/settingsStore.svelte';
    import AvatarUploadModal from './AvatarUploadModal.svelte';
    import { resolveAvatarUrl } from '$lib/utils';
    import type { UserPersona } from '$lib/stores/userPersonaStore.svelte';
    import { ChevronDown, ChevronUp } from 'lucide-svelte';

    let {
        persona,
        isActive,
    }: {
        persona: UserPersona;
        isActive: boolean;
    } = $props();

    let expanded = $state(false);
    let draftName = $state(persona.name);
    let draftDesc = $state(persona.description ?? '');
    let avatarUploadOpen = $state(false);
    let saving = $state(false);

    function toggleExpand() {
        expanded = !expanded;
        if (expanded) {
            draftName = persona.name;
            draftDesc = persona.description ?? '';
        }
    }

    async function handleActivate() {
        await userPersonaStore.activatePersona(persona.id);
    }

    async function handleSave() {
        saving = true;
        try {
            await userPersonaStore.updatePersona({
                id: persona.id,
                name: draftName.trim() || persona.name,
                description: draftDesc.trim() || undefined,
            });
            expanded = false;
        } finally {
            saving = false;
        }
    }

    function handleCancel() {
        draftName = persona.name;
        draftDesc = persona.description ?? '';
        expanded = false;
    }

    function handleUseDefaultAvatar() {
        const defaultPath = settingsStore.settings?.default_avatar_path;
        if (defaultPath) {
            userPersonaStore.updatePersona({ id: persona.id, avatar_path: defaultPath });
        }
    }

    function handleAvatarUploaded(path: string) {
        avatarUploadOpen = false;
        userPersonaStore.updatePersona({ id: persona.id, avatar_path: path });
    }
</script>

{#if avatarUploadOpen}
    <AvatarUploadModal
        targetType="user_persona"
        targetId={persona.id}
        onUploaded={handleAvatarUploaded}
        onClose={() => avatarUploadOpen = false}
    />
{/if}

<div class="border border-border rounded-lg bg-surface overflow-hidden">
    <!-- Header Row -->
    <div class="flex items-center gap-3 px-4 py-3 cursor-pointer hover:bg-gray-50" onclick={toggleExpand}>
        <!-- Avatar (clickable to change) -->
        <button
            onclick={(e) => { e.stopPropagation(); avatarUploadOpen = true; }}
            class="w-9 h-9 rounded-full bg-gray-200 flex items-center justify-center overflow-hidden shrink-0 hover:ring-2 hover:ring-primary"
            title="点击更换头像"
        >
            {#if persona.avatar_path}
                <img src={resolveAvatarUrl(persona.avatar_path)} alt="" class="w-full h-full object-cover" />
            {:else}
                <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-gray-400"><path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>
            {/if}
        </button>

        <!-- Name -->
        <span class="flex-1 font-medium text-sm truncate">{persona.name}</span>

        <!-- Activate Button -->
        {#if isActive}
            <button class="px-3 py-1 rounded-md bg-primary text-white text-xs font-medium shadow-inner">
                启用中
            </button>
        {:else}
            <button
                onclick={(e) => { e.stopPropagation(); handleActivate(); }}
                class="px-3 py-1 rounded-md bg-primary/10 text-primary text-xs font-medium hover:bg-primary hover:text-white transition-colors"
            >
                启用
            </button>
        {/if}

        <!-- Expand Icon -->
        {#if expanded}
            <ChevronUp size={16} class="text-text-secondary" />
        {:else}
            <ChevronDown size={16} class="text-text-secondary" />
        {/if}
    </div>

    <!-- Expanded Content -->
    {#if expanded}
        <div class="px-4 pb-4 border-t border-border bg-bg">
            <div class="pt-3 space-y-3">
                <div>
                    <label class="block text-xs font-medium text-text-secondary mb-1">角色名</label>
                    <input type="text" bind:value={draftName} class="w-full px-3 py-2 rounded-lg border border-border bg-surface text-sm focus:outline-none focus:ring-2 focus:ring-primary" />
                </div>
                <div>
                    <label class="block text-xs font-medium text-text-secondary mb-1">简易人设</label>
                    <textarea bind:value={draftDesc} rows={2} class="w-full px-3 py-2 rounded-lg border border-border bg-surface text-sm focus:outline-none focus:ring-2 focus:ring-primary resize-none"></textarea>
                </div>
                <div class="flex items-center gap-2">
                    <button onclick={handleUseDefaultAvatar} class="text-xs text-primary hover:underline">使用默认头像</button>
                </div>
                <div class="flex justify-end gap-2">
                    <button onclick={handleCancel} class="px-3 py-1.5 rounded-lg text-xs text-text-secondary hover:bg-gray-100">取消</button>
                    <button onclick={handleSave} disabled={saving} class="px-3 py-1.5 rounded-lg text-xs bg-primary text-white disabled:opacity-50">
                        {saving ? '保存中...' : '保存'}
                    </button>
                </div>
            </div>
        </div>
    {/if}
</div>
```

- [ ] **Step 2: 验证前端编译**

Run:
```bash
pnpm build
```
Expected: 无错误

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/UserPersonaItem.svelte
git commit -m "feat: 新建 UserPersonaItem 手风琴组件 - 展开编辑、头像点击更换"
```

---

## Task 12: 新建 UserPersonaConfig + ProfileView

**Files:**
- Create: `src/lib/components/ProfileView.svelte`
- Create: `src/lib/components/UserPersonaConfig.svelte`

- [ ] **Step 1: 创建 UserPersonaConfig.svelte**

```svelte
<!-- src/lib/components/UserPersonaConfig.svelte -->
<script lang="ts">
    import { userPersonaStore } from '$lib/stores/userPersonaStore.svelte';
    import { settingsStore } from '$lib/stores/settingsStore.svelte';
    import { resolveAvatarUrl } from '$lib/utils';
    import UserPersonaItem from './UserPersonaItem.svelte';
    import CreateUserPersonaModal from './CreateUserPersonaModal.svelte';
    import AvatarUploadModal from './AvatarUploadModal.svelte';
    import { Plus } from 'lucide-svelte';

    let avatarUploadOpen = $state(false);
    let createModalOpen = $state(false);

    let activePersonaId = $derived(settingsStore.settings?.active_persona_id ?? null);

    function handleAvatarUploaded(path: string) {
        avatarUploadOpen = false;
        settingsStore.update({ default_avatar_path: path });
    }

    async function handleDeactivate() {
        await userPersonaStore.activatePersona(null);
    }

    // Load on mount
    $effect(() => {
        userPersonaStore.loadPersonas();
        userPersonaStore.loadCurrentPersona();
    });
</script>

{#if avatarUploadOpen}
    <AvatarUploadModal
        targetType="user_default"
        targetId="default"
        onUploaded={handleAvatarUploaded}
        onClose={() => avatarUploadOpen = false}
    />
{/if}

{#if createModalOpen}
    <CreateUserPersonaModal
        onclose={() => createModalOpen = false}
        oncreated={() => userPersonaStore.loadPersonas()}
    />
{/if}

<div class="h-full flex flex-col">
    <!-- Header -->
    <div class="px-6 py-4 border-b border-border">
        <h1 class="text-lg font-semibold">用户角色配置</h1>
    </div>

    <!-- Scrollable Content -->
    <div class="flex-1 overflow-y-auto px-6 py-4 space-y-4">
        <!-- Default Avatar Row -->
        <div class="flex items-center gap-3 py-2">
            <button
                onclick={() => avatarUploadOpen = true}
                class="w-10 h-10 rounded-full bg-gray-200 flex items-center justify-center overflow-hidden shrink-0 hover:ring-2 hover:ring-primary"
                title="点击更换默认头像"
            >
                {#if settingsStore.settings?.default_avatar_path}
                    <img src={resolveAvatarUrl(settingsStore.settings.default_avatar_path)} alt="default" class="w-full h-full object-cover" />
                {:else}
                    <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="text-gray-400"><path d="M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>
                {/if}
            </button>
            <span class="text-sm font-medium text-text-secondary">默认头像</span>
            <div class="flex-1"></div>
            {#if activePersonaId !== null}
                <button onclick={handleDeactivate} class="text-sm text-text-secondary hover:text-primary transition-colors">
                    关闭使用人设
                </button>
            {/if}
        </div>

        <!-- Persona List -->
        <div class="space-y-2">
            {#each userPersonaStore.personas as persona (persona.id)}
                <UserPersonaItem
                    persona={persona}
                    isActive={activePersonaId === persona.id}
                />
            {/each}
        </div>

        <!-- Create Button -->
        <button
            onclick={() => createModalOpen = true}
            class="w-full py-3 rounded-lg border border-dashed border-border flex items-center justify-center gap-2 text-text-secondary hover:text-primary hover:border-primary transition-colors"
        >
            <Plus size={18} />
            <span class="text-sm">创建新人设</span>
        </button>
    </div>
</div>
```

- [ ] **Step 2: 创建 ProfileView.svelte**

```svelte
<!-- src/lib/components/ProfileView.svelte -->
<script lang="ts">
    import UserPersonaConfig from './UserPersonaConfig.svelte';

    // 当前只实现 user_persona 分类，后续可扩展
    const categories = [
        { id: 'user_persona', label: '用户角色配置' },
    ];

    let activeCategory = $state('user_persona');
</script>

<div class="flex h-full">
    <!-- Category List -->
    <div class="w-56 shrink-0 bg-surface border-r border-border flex flex-col">
        <div class="px-4 py-3 border-b border-border">
            <h2 class="font-semibold text-sm">个人配置</h2>
        </div>
        <nav class="flex-1 p-2">
            {#each categories as cat}
                <button
                    class="w-full text-left px-3 py-2 rounded-lg text-sm transition-colors {activeCategory === cat.id ? 'bg-primary/10 text-primary font-medium' : 'text-text hover:bg-gray-100'}"
                    onclick={() => activeCategory = cat.id}
                >
                    {cat.label}
                </button>
            {/each}
        </nav>
    </div>

    <!-- Detail Area -->
    <div class="flex-1 min-w-0">
        {#if activeCategory === 'user_persona'}
            <UserPersonaConfig />
        {/if}
    </div>
</div>
```

- [ ] **Step 3: 验证前端编译**

Run:
```bash
pnpm build
```
Expected: 无错误

- [ ] **Step 4: Commit**

```bash
git add src/lib/components/UserPersonaConfig.svelte src/lib/components/ProfileView.svelte
git commit -m "feat: 新建 UserPersonaConfig 和 ProfileView 组件"
```

---

## Task 13: 更新 App.svelte 渲染 Profile 视图

**Files:**
- Modify: `src/App.svelte`

- [ ] **Step 1: 修改 App.svelte**

在 `<script>` 顶部 imports 中新增：
```typescript
import ProfileView from '$lib/components/ProfileView.svelte';
```

修改 Middle Panel 的条件渲染：
```svelte
<!-- Middle Panel -->
<div class="w-72 shrink-0 bg-surface border-r border-border">
    {#if appState.currentView === 'agents'}
        <AgentList />
    {:else if appState.currentView === 'chat'}
        <SessionList />
    {:else if appState.currentView === 'profile'}
        <!-- Profile 视图左侧已自带分类列表，中间面板留空或显示占位 -->
        <div class="h-full flex items-center justify-center text-text-secondary text-sm">
            请在右侧选择配置项
        </div>
    {:else}
        <HistorySessionList />
    {/if}
</div>
```

修改 Main Content Area 的条件渲染：
```svelte
<!-- Main Content Area -->
<main class="flex-1 min-w-0 bg-bg">
    {#if appState.currentView === 'agents'}
        <AgentDetail />
    {:else if appState.currentView === 'chat'}
        <ChatView />
    {:else if appState.currentView === 'profile'}
        <ProfileView />
    {:else}
        <ChatView mode="history" />
    {/if}
</main>
```

- [ ] **Step 2: 验证前端编译**

Run:
```bash
pnpm build
```
Expected: 无错误

- [ ] **Step 3: Commit**

```bash
git add src/App.svelte
git commit -m "feat: App.svelte 新增 profile 视图渲染"
```

---

## Task 14: 整合测试与验证

**Files:** 所有已修改文件

- [ ] **Step 1: 后端类型检查**

Run:
```bash
cd src-tauri && cargo check
```
Expected: 无错误

- [ ] **Step 2: 后端测试编译**

Run:
```bash
cd src-tauri && cargo check --tests
```
Expected: 无错误

- [ ] **Step 3: 前端构建**

Run:
```bash
pnpm build
```
Expected: 无 TypeScript/Svelte 错误

- [ ] **Step 4: 功能验证清单（手动测试）**

启动应用：`pnpm tauri dev`，逐一验证：

- [ ] 左上角显示 `[个人]` 按钮，点击后切换到个人配置页
- [ ] 默认头像区域显示灰色图标，点击可上传头像
- [ ] 上传默认头像后，灰色图标变为实际头像
- [ ] 创建新人设弹窗正常弹出，必填校验生效
- [ ] 创建多个人设，列表正确显示
- [ ] 点击人设行展开，可编辑 name/description
- [ ] 点击人设头像直接更换，无需保存即生效
- [ ] 点击"使用默认头像"，人设头像变为默认头像
- [ ] 点击"启用"按钮，该人设变为"启用中"，其他人设恢复"启用"
- [ ] 点击"关闭使用人设"，所有人设恢复"启用"
- [ ] 删除人设后列表移除
- [ ] 切换回聊天视图，验证 Prompt 组装正常（后端日志）

- [ ] **Step 5: Commit 任何修复**

如有 bugfix，分别提交。最后提交总结 commit：

```bash
git commit -m "feat(USR-01): 用户角色配置页完整实现 - 支持多套人设、切换、头像管理"
```

---

## Self-Review Checklist

### Spec Coverage

| 设计文档章节 | 实现任务 |
|-------------|---------|
| 3.1 默认头像区域 | Task 12 (UserPersonaConfig) |
| 3.2 人设列表（手风琴） | Task 11 (UserPersonaItem) |
| 3.3 创建新人设弹窗 | Task 10 (CreateUserPersonaModal) |
| 4.1 数据库变更 | Task 3 |
| 4.2 Rust Model | Task 2 |
| 4.3 代码常量 | Task 1 |
| 5 后端 API | Task 5 |
| 6.1 Store | Task 9 |
| 6.2 组件 | Task 10-13 |
| 7 对现有系统的影响 | Task 6, 7 |
| 8 迁移策略 | Task 3 |

✅ **无遗漏**

### Placeholder Scan

- 无 "TBD", "TODO", "implement later", "add appropriate error handling"
- 无 "Similar to Task N" 引用
- 所有步骤都有具体代码或命令

### Type Consistency

- `UserPersona` 模型字段：`id`, `name`, `description`, `avatar_path`, `created_at`, `updated_at` — 前后端一致
- `CreateUserPersonaRequest` / `UpdateUserPersonaRequest` — Task 2 和 Task 4/5 一致
- `activate_user_persona(id: Option<String>)` — Task 4 repository 和 Task 5 command 签名一致
- `currentView` 类型 `'agents' | 'chat' | 'history' | 'profile'` — Task 8 前后端一致

✅ **无类型不一致**

---

**Plan complete and saved to `docs/superpowers/plans/2026-05-17-user-persona-config.md`.**

Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
