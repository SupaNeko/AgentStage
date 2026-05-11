# 全局配置面板 + 群聊创建实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现全局配置面板（全局最小触发间隔）和群聊创建功能（含右侧成员列表）。

**Architecture:** 后端新增 Settings 和 Group Session 的 Tauri Commands + Repository 方法；前端新增 SettingsPanel、CreateGroupModal 组件，并修改 ChatView 增加群聊成员列表侧边栏。

**Tech Stack:** Tauri v2 (Rust + SQLite), Svelte 5 + TypeScript + TailwindCSS v4

---

## 文件结构

| 文件 | 职责 |
|------|------|
| `src-tauri/src/models/session.rs` | 新增 `CreateGroupSessionRequest`、`GroupMemberResponse` DTO |
| `src-tauri/src/db/settings.rs` | 新增 `update_settings()` repository 方法 |
| `src-tauri/src/db/session.rs` | 新增 `create_group_session()`、`get_group_members()` repository 方法 |
| `src-tauri/src/commands/settings.rs` | **新建**：`get_settings`、`update_settings` Tauri Commands |
| `src-tauri/src/commands/session.rs` | 新增 `create_group_session`、`get_group_members` Tauri Commands |
| `src-tauri/src/lib.rs` | 注册 4 个新 Commands |
| `src/lib/types.ts` | 新增 `GroupMember` 接口 |
| `src/lib/stores/settingsStore.svelte.ts` | **新建**：Settings 的加载与更新 store |
| `src/lib/components/SettingsPanel.svelte` | **新建**：设置表单（触发间隔 + 保存按钮） |
| `src/lib/components/CreateGroupModal.svelte` | **新建**：群聊创建弹窗（群名 + 角色多选） |
| `src/lib/components/SessionList.svelte` | 新增"新建群聊"按钮入口 |
| `src/lib/components/ChatView.svelte` | 新增右侧群聊成员列表（仅群聊显示） |
| `src/App.svelte` | 集成 `SettingsPanel`，启动时加载 settings |

---

## Task 1: 后端 DTO — 群聊请求/响应结构

**Files:**
- Modify: `src-tauri/src/models/session.rs`

- [ ] **Step 1: 在 `models/session.rs` 末尾追加两个 DTO**

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct CreateGroupSessionRequest {
    pub name: String,
    pub agent_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroupMemberResponse {
    pub participant_type: String,
    pub participant_id: String,
    pub name: String,
    pub avatar_path: Option<String>,
}
```

- [ ] **Step 2: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: PASS（无新增错误，原有错误保留）

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/models/session.rs
git commit -m "feat(models): add CreateGroupSessionRequest and GroupMemberResponse DTOs"
```

---

## Task 2: 后端 Repository — 更新设置

**Files:**
- Modify: `src-tauri/src/db/settings.rs`

- [ ] **Step 1: 在 `db/settings.rs` 中 `get_or_create_settings` 之后追加 `update_settings`**

```rust
pub fn update_settings(conn: &Connection, req: &crate::models::settings::UpdateAppSettingsRequest) -> Result<()> {
    let current = get_or_create_settings(conn)?;
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE app_settings SET 
            global_min_trigger_interval = ?1, private_message_limit_default = ?2,
            group_message_limit_default = ?3, private_limit_enabled_default = ?4,
            group_limit_enabled_default = ?5, theme = ?6, font_size = ?7,
            language = ?8, enter_to_send = ?9, launch_on_startup = ?10,
            minimize_to_tray = ?11, updated_at = ?12 WHERE id = 1",
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
            now,
        ],
    )?;
    Ok(())
}
```

- [ ] **Step 2: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/db/settings.rs
git commit -m "feat(db): add update_settings repository method"
```

---

## Task 3: 后端 Commands — Settings

**Files:**
- Create: `src-tauri/src/commands/settings.rs`
- Modify: `src-tauri/src/commands/mod.rs`

- [ ] **Step 1: 创建 `commands/settings.rs`**

```rust
use tauri::State;
use crate::db::connection::{get_db, DbState};
use crate::db::settings as settings_repo;
use crate::models::settings::{SettingsResponse, UpdateAppSettingsRequest};

#[tauri::command]
pub async fn get_settings(state: State<'_, DbState>) -> Result<SettingsResponse, String> {
    let conn = get_db(&state).await?;
    let settings = settings_repo::get_or_create_settings(&conn)
        .map_err(|e| e.to_string())?;
    Ok(settings.into())
}

#[tauri::command]
pub async fn update_settings(
    state: State<'_, DbState>,
    req: UpdateAppSettingsRequest,
) -> Result<SettingsResponse, String> {
    let conn = get_db(&state).await?;
    settings_repo::update_settings(&conn, &req)
        .map_err(|e| e.to_string())?;
    let settings = settings_repo::get_or_create_settings(&conn)
        .map_err(|e| e.to_string())?;
    Ok(settings.into())
}
```

- [ ] **Step 2: 在 `commands/mod.rs` 中新增 `pub mod settings;`**

```rust
pub mod agent;
pub mod log;
pub mod message;
pub mod session;
pub mod settings;
```

- [ ] **Step 3: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/settings.rs src-tauri/src/commands/mod.rs
git commit -m "feat(commands): add get_settings and update_settings"
```

---

## Task 4: 后端 Repository — 群聊创建与成员查询

**Files:**
- Modify: `src-tauri/src/db/session.rs`

- [ ] **Step 1: 在 `db/session.rs` 末尾追加两个方法**

```rust
pub fn create_group_session(
    conn: &Connection,
    name: &str,
    agent_ids: &[String],
) -> Result<SessionResponse> {
    if agent_ids.len() < 2 {
        return Err(rusqlite::Error::InvalidParameterName(
            "群聊至少需要选择 2 个角色".into()
        ));
    }

    let session_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    let tx = conn.unchecked_transaction()?;

    conn.execute(
        "INSERT INTO sessions (id, session_type, created_at, updated_at) VALUES (?1, 'group', ?2, ?3)",
        (&session_id, now, now),
    )?;

    conn.execute(
        "INSERT INTO group_sessions (session_id, name, mute_enabled, created_at) VALUES (?1, ?2, 0, ?3)",
        (&session_id, name, now),
    )?;

    conn.execute(
        "INSERT INTO group_members (session_id, participant_type, participant_id, joined_at) VALUES (?1, 'user', 'user', ?2)",
        (&session_id, now),
    )?;

    for agent_id in agent_ids {
        conn.execute(
            "INSERT INTO group_members (session_id, participant_type, participant_id, joined_at) VALUES (?1, 'agent', ?2, ?3)",
            (&session_id, agent_id, now),
        )?;
    }

    tx.commit()?;
    get_session_by_id(conn, &session_id)?
        .ok_or(rusqlite::Error::QueryReturnedNoRows)
}

pub fn get_group_members(
    conn: &Connection,
    session_id: &str,
) -> Result<Vec<crate::models::session::GroupMemberResponse>> {
    let mut stmt = conn.prepare(
        "SELECT gm.participant_type, gm.participant_id,
                CASE WHEN gm.participant_type = 'user' THEN '用户' ELSE COALESCE(a.name, '未知角色') END as name,
                a.avatar_path
         FROM group_members gm
         LEFT JOIN agents a ON gm.participant_type = 'agent' AND gm.participant_id = a.id
         WHERE gm.session_id = ?1 AND gm.is_active = 1
         ORDER BY gm.participant_type DESC, name ASC"
    )?;
    let rows = stmt.query_map([session_id], |row| {
        Ok(crate::models::session::GroupMemberResponse {
            participant_type: row.get(0)?,
            participant_id: row.get(1)?,
            name: row.get(2)?,
            avatar_path: row.get(3)?,
        })
    })?;
    rows.collect()
}
```

- [ ] **Step 2: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/db/session.rs
git commit -m "feat(db): add create_group_session and get_group_members"
```

---

## Task 5: 后端 Commands — 群聊

**Files:**
- Modify: `src-tauri/src/commands/session.rs`

- [ ] **Step 1: 在 `commands/session.rs` 中追加两个 Command**

在文件顶部追加 import：

```rust
use crate::models::session::{CreateGroupSessionRequest, GroupMemberResponse};
```

在文件末尾追加：

```rust
#[tauri::command]
pub async fn create_group_session(
    state: State<'_, DbState>,
    req: CreateGroupSessionRequest,
) -> Result<SessionResponse, String> {
    crate::logger::backend("DEBUG", &format!("[DEBUG create_group_session] name={}, agents={:?}", req.name, req.agent_ids));
    let conn = get_db(&state).await?;
    let session = session_repo::create_group_session(&conn, &req.name, &req.agent_ids)
        .map_err(|e| e.to_string())?;
    Ok(session)
}

#[tauri::command]
pub async fn get_group_members(
    state: State<'_, DbState>,
    session_id: String,
) -> Result<Vec<GroupMemberResponse>, String> {
    let conn = get_db(&state).await?;
    let members = session_repo::get_group_members(&conn, &session_id)
        .map_err(|e| e.to_string())?;
    Ok(members)
}
```

- [ ] **Step 2: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/session.rs
git commit -m "feat(commands): add create_group_session and get_group_members"
```

---

## Task 6: 注册新 Commands

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 在 `lib.rs` 顶部追加 imports**

```rust
use commands::settings::{get_settings, update_settings};
```

并将 `use commands::session::...` 一行扩展为：

```rust
use commands::session::{create_group_session, create_private_session, delete_session, get_group_members, get_session, list_sessions};
```

- [ ] **Step 2: 在 `invoke_handler` 中注册 4 个新命令**

```rust
.invoke_handler(tauri::generate_handler![
    create_agent,
    get_agent,
    list_agents,
    update_agent,
    delete_agent,
    create_private_session,
    create_group_session,
    list_sessions,
    get_session,
    delete_session,
    send_user_message,
    get_session_messages,
    get_settings,
    update_settings,
    get_group_members,
    log_frontend,
])
```

- [ ] **Step 3: 编译检查**

Run: `cd src-tauri && cargo check`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(lib): register get_settings, update_settings, create_group_session, get_group_members"
```

---

## Task 7: 前端类型定义

**Files:**
- Modify: `src/lib/types.ts`

- [ ] **Step 1: 读取 `src/lib/types.ts` 确认现有内容，然后在末尾追加 `GroupMember` 接口**

```typescript
export interface GroupMember {
    participant_type: 'user' | 'agent';
    participant_id: string;
    name: string;
    avatar_path: string | null;
}
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/types.ts
git commit -m "feat(types): add GroupMember interface"
```

---

## Task 8: 前端 Settings Store

**Files:**
- Create: `src/lib/stores/settingsStore.svelte.ts`

- [ ] **Step 1: 创建 store 文件**

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
git commit -m "feat(store): add settingsStore for global config"
```

---

## Task 9: 前端 SettingsPanel 组件

**Files:**
- Create: `src/lib/components/SettingsPanel.svelte`

- [ ] **Step 1: 创建组件**

```svelte
<script lang="ts">
    import { settingsStore } from '$lib/stores/settingsStore.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { X } from 'lucide-svelte';

    let draft = $state({ global_min_trigger_interval: 30 });
    let saving = $state(false);

    $effect(() => {
        if (settingsStore.settings) {
            draft = {
                global_min_trigger_interval: settingsStore.settings.global_min_trigger_interval,
            };
        }
    });

    async function handleSave() {
        saving = true;
        try {
            await settingsStore.update({
                global_min_trigger_interval: draft.global_min_trigger_interval,
            });
            toastStore.show('已保存', 'success', 2000);
        } catch (err) {
            toastStore.show(`保存失败：${err}`, 'error');
        } finally {
            saving = false;
        }
    }

    let { onclose }: { onclose: () => void } = $props();
</script>

<div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onclick={(e) => { if (e.target === e.currentTarget) onclose(); }}>
    <div class="bg-surface rounded-xl shadow-xl w-full max-w-lg max-h-[80vh] overflow-y-auto">
        <div class="flex items-center justify-between p-4 border-b border-border">
            <h3 class="text-lg font-semibold">设置</h3>
            <button onclick={onclose} class="p-1 hover:bg-gray-100 rounded">
                <X size={20} />
            </button>
        </div>
        <div class="p-6 space-y-6">
            <div>
                <label class="block text-sm font-medium mb-1">全局最小触发间隔（秒）</label>
                <input
                    type="number"
                    min="0"
                    bind:value={draft.global_min_trigger_interval}
                    class="w-full px-3 py-2 bg-bg border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20"
                />
                <p class="text-xs text-text-secondary mt-1">0 = 不限制，>0 = 角色收到消息后至少等待 N 秒才会被触发</p>
            </div>
        </div>
        <div class="p-4 border-t border-border flex justify-end">
            <button
                onclick={handleSave}
                disabled={saving}
                class="px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-dark transition-colors disabled:opacity-50"
            >
                {saving ? '保存中...' : '保存'}
            </button>
        </div>
    </div>
</div>
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/components/SettingsPanel.svelte
git commit -m "feat(ui): add SettingsPanel component for global trigger interval"
```

---

## Task 10: App.svelte 集成设置面板

**Files:**
- Modify: `src/App.svelte`

- [ ] **Step 1: 在 `App.svelte` 中导入并替换原有设置弹窗**

在 `<script>` 顶部追加：

```typescript
import SettingsPanel from '$lib/components/SettingsPanel.svelte';
import { settingsStore } from '$lib/stores/settingsStore.svelte';
```

在 `onMount` 中追加：

```typescript
settingsStore.load();
```

将原有设置弹窗代码（约第 122-137 行）替换为：

```svelte
{#if appState.settingsOpen}
    <SettingsPanel onclose={() => appState.closeSettings()} />
{/if}
```

- [ ] **Step 2: 前端编译检查**

Run: `npx svelte-check --tsconfig ./tsconfig.json`
Expected: PASS（0 errors）

- [ ] **Step 3: Commit**

```bash
git add src/App.svelte
git commit -m "feat(app): integrate SettingsPanel and load settings on mount"
```

---

## Task 11: CreateGroupModal 组件

**Files:**
- Create: `src/lib/components/CreateGroupModal.svelte`

- [ ] **Step 1: 创建组件**

```svelte
<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { onMount } from 'svelte';
    import { sessionStore } from '$lib/stores/sessionStore.svelte';
    import { appState } from '$lib/stores/appState.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import type { Agent, Session } from '$lib/types';
    import { X, Users, User, Bot } from 'lucide-svelte';

    let groupName = $state('');
    let selectedAgentIds = $state<Set<string>>(new Set());
    let agents = $state<Agent[]>([]);
    let loadingAgents = $state(true);
    let creating = $state(false);

    async function loadAgents() {
        loadingAgents = true;
        try {
            agents = await invoke<Agent[]>('list_agents');
        } catch (err) {
            toastStore.show('加载角色列表失败', 'error');
        } finally {
            loadingAgents = false;
        }
    }

    onMount(() => { loadAgents(); });

    function toggleAgent(agentId: string) {
        const next = new Set(selectedAgentIds);
        if (next.has(agentId)) next.delete(agentId);
        else next.add(agentId);
        selectedAgentIds = next;
    }

    async function handleCreate() {
        const name = groupName.trim();
        if (!name) { toastStore.show('请输入群聊名称', 'error'); return; }
        if (selectedAgentIds.size < 2) { toastStore.show('请选择至少 2 个角色', 'error'); return; }
        creating = true;
        try {
            const session = await invoke<Session>('create_group_session', {
                req: { name, agent_ids: Array.from(selectedAgentIds) },
            });
            sessionStore.addSession(session);
            sessionStore.selectSession(session.id);
            appState.switchView('chat');
            toastStore.show('群聊创建成功', 'success', 2000);
            onclose?.();
        } catch (err) {
            toastStore.show(`创建失败：${err}`, 'error');
        } finally {
            creating = false;
        }
    }

    let { onclose }: { onclose?: () => void } = $props();
</script>

<div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
    <div class="bg-surface rounded-xl shadow-xl w-full max-w-md max-h-[80vh] flex flex-col">
        <div class="flex items-center justify-between p-4 border-b border-border shrink-0">
            <h3 class="text-lg font-semibold flex items-center gap-2">
                <Users size={20} /> 新建群聊
            </h3>
            <button onclick={onclose} class="p-1 hover:bg-gray-100 rounded"><X size={20} /></button>
        </div>
        <div class="p-4 space-y-4 overflow-y-auto flex-1">
            <div>
                <label class="block text-sm font-medium mb-1">群聊名称</label>
                <input bind:value={groupName} placeholder="输入群聊名称..."
                    class="w-full px-3 py-2 bg-bg border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20" />
            </div>
            <div>
                <label class="block text-sm font-medium mb-2">
                    选择角色 <span class="text-text-secondary font-normal">(至少 2 个)</span>
                </label>
                {#if loadingAgents}
                    <p class="text-sm text-text-secondary">加载中...</p>
                {:else}
                    <div class="space-y-1">
                        {#each agents as agent}
                            <label class="flex items-center gap-3 p-2 rounded-lg hover:bg-bg cursor-pointer">
                                <input type="checkbox" checked={selectedAgentIds.has(agent.id)}
                                    onchange={() => toggleAgent(agent.id)} />
                                <div class="w-8 h-8 rounded-full bg-primary/10 flex items-center justify-center text-primary shrink-0 overflow-hidden">
                                    {#if agent.avatar_path}
                                        <img src={agent.avatar_path} alt={agent.name} class="w-full h-full object-cover" />
                                    {:else}
                                        <Bot size={16} />
                                    {/if}
                                </div>
                                <span class="text-sm">{agent.name}</span>
                            </label>
                        {/each}
                    </div>
                {/if}
            </div>
        </div>
        <div class="p-4 border-t border-border flex justify-end gap-2 shrink-0">
            <button onclick={onclose} class="px-4 py-2 text-sm rounded-lg hover:bg-bg border border-border">取消</button>
            <button onclick={handleCreate}
                disabled={creating || selectedAgentIds.size < 2 || !groupName.trim()}
                class="px-4 py-2 bg-primary text-white text-sm rounded-lg hover:bg-primary-dark transition-colors disabled:opacity-50">
                {creating ? '创建中...' : '创建'}
            </button>
        </div>
    </div>
</div>
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/components/CreateGroupModal.svelte
git commit -m "feat(ui): add CreateGroupModal for group chat creation"
```

---

## Task 12: SessionList 新增群聊入口

**Files:**
- Modify: `src/lib/components/SessionList.svelte`

- [ ] **Step 1: 导入 CreateGroupModal 和 Plus 图标，在 Header 增加按钮**

在 `<script>` 中追加：

```typescript
import CreateGroupModal from './CreateGroupModal.svelte';
import { Plus } from 'lucide-svelte';
let showCreateGroup = $state(false);
```

修改 Header 区域：

```svelte
<header class="flex items-center justify-between p-4 border-b border-border">
    <h2 class="text-base font-semibold">会话列表</h2>
    <button onclick={() => showCreateGroup = true}
        class="p-1.5 hover:bg-bg rounded-lg text-text-secondary hover:text-text transition-colors" title="新建群聊">
        <Plus size={18} />
    </button>
</header>
```

在文件最末尾（`</div>` 之后）追加：

```svelte
{#if showCreateGroup}
    <CreateGroupModal onclose={() => showCreateGroup = false} />
{/if}
```

- [ ] **Step 2: 前端编译检查**

Run: `npx svelte-check --tsconfig ./tsconfig.json`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/SessionList.svelte
git commit -m "feat(ui): add create group chat button to SessionList"
```

---

## Task 13: ChatView 群聊成员列表

**Files:**
- Modify: `src/lib/components/ChatView.svelte`

- [ ] **Step 1: 导入新依赖并增加成员状态**

在 `<script>` 顶部追加：

```typescript
import { User } from 'lucide-svelte';
import type { GroupMember } from '$lib/types';
```

在 script 中增加状态：

```typescript
let members = $state<GroupMember[]>([]);
let loadingMembers = $state(false);
```

在 `$effect` 中（sessionId 变化时）追加群聊成员加载：

```typescript
$effect(() => {
    const id = sessionStore.selectedSessionId;
    if (id) {
        messageStore.loadMessages(id);
        const session = sessionStore.sessions.find(s => s.id === id);
        if (session?.session_type === 'group') {
            loadingMembers = true;
            invoke<GroupMember[]>('get_group_members', { sessionId: id })
                .then((data) => { members = data; })
                .catch((err) => logger.error('Failed to load group members:', err))
                .finally(() => { loadingMembers = false; });
        } else {
            members = [];
        }
    } else {
        messageStore.setSessionId(null);
        members = [];
    }
});
```

- [ ] **Step 2: 修改外层布局，增加右侧成员列表**

将最外层 `<div class="flex flex-col h-full bg-bg">` 改为：

```svelte
<div class="flex h-full bg-bg">
    <div class="flex flex-col flex-1 min-w-0">
```

并在中间聊天区域结束（原 `</div>`）后追加右侧边栏，再闭合外层：

```svelte
    </div>
    {#if selectedSession?.session_type === 'group'}
        <aside class="w-56 border-l border-border bg-surface flex flex-col shrink-0">
            <div class="p-3 border-b border-border">
                <h3 class="text-sm font-medium">成员 ({members.length})</h3>
            </div>
            <div class="flex-1 overflow-y-auto p-2 space-y-1">
                {#if loadingMembers}
                    <p class="text-xs text-text-secondary p-2">加载中...</p>
                {:else}
                    {#each members as member}
                        <div class="flex items-center gap-2 p-2 rounded-lg hover:bg-bg">
                            <div class="w-8 h-8 rounded-full bg-primary/10 flex items-center justify-center text-primary shrink-0 overflow-hidden">
                                {#if member.avatar_path}
                                    <img src={member.avatar_path} alt={member.name} class="w-full h-full object-cover" />
                                {:else}
                                    <User size={16} />
                                {/if}
                            </div>
                            <span class="text-sm truncate">{member.name}</span>
                        </div>
                    {/each}
                {/if}
            </div>
        </aside>
    {/if}
</div>
```

- [ ] **Step 3: 前端编译检查**

Run: `npx svelte-check --tsconfig ./tsconfig.json`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/lib/components/ChatView.svelte
git commit -m "feat(ui): add group member list sidebar in ChatView"
```

---

## Task 14: Rust 单元测试

**Files:**
- Modify: `src-tauri/src/db/settings.rs`
- Modify: `src-tauri/src/db/session.rs`

- [ ] **Step 1: 在 `db/settings.rs` 文件末尾追加测试模块**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::settings::UpdateAppSettingsRequest;

    fn init_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V1).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V2).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V3).unwrap();
        conn
    }

    #[test]
    fn test_update_settings_preserve_untouched_fields() {
        let conn = init_test_db();
        let before = get_or_create_settings(&conn).unwrap();
        assert_eq!(before.theme, "system");
        assert_eq!(before.font_size, "medium");

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
        };
        update_settings(&conn, &req).unwrap();

        let after = get_or_create_settings(&conn).unwrap();
        assert_eq!(after.global_min_trigger_interval, 60);
        assert_eq!(after.theme, "system");
        assert_eq!(after.font_size, "medium");
    }
}
```

- [ ] **Step 2: 在 `db/session.rs` 文件末尾追加测试模块**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn init_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V1).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V2).unwrap();
        conn.execute_batch(crate::db::schema::MIGRATION_V3).unwrap();
        conn
    }

    #[test]
    fn test_create_group_session_min_2_agents() {
        let conn = init_test_db();
        let result = create_group_session(&conn, "Test Group", &["agent1".into()]);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_group_session_and_get_members() {
        let conn = init_test_db();
        // 需要先插入两个测试角色
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Agent One", 0i64),
        ).unwrap();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent2", "Agent Two", 0i64),
        ).unwrap();

        let session = create_group_session(&conn, "Test Group", &["agent1".into(), "agent2".into()]).unwrap();
        assert_eq!(session.session_type, "group");
        assert_eq!(session.group_name, Some("Test Group".into()));

        let members = get_group_members(&conn, &session.id).unwrap();
        assert_eq!(members.len(), 3); // user + 2 agents
        assert_eq!(members[0].participant_type, "user");
        assert_eq!(members[0].name, "用户");
        assert_eq!(members[1].name, "Agent One");
        assert_eq!(members[2].name, "Agent Two");
    }
}
```

- [ ] **Step 3: 运行测试**

Run: `cd src-tauri && cargo test`
Expected: 3 tests PASS

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db/settings.rs src-tauri/src/db/session.rs
git commit -m "test: add unit tests for update_settings and group session creation"
```

---

## Task 15: 端到端验证

**Files:**
- (无文件修改，纯验证)

- [ ] **Step 1: Rust 编译通过**

Run: `cd src-tauri && cargo check`
Expected: PASS

- [ ] **Step 2: 前端类型检查通过**

Run: `npx svelte-check --tsconfig ./tsconfig.json`
Expected: PASS（0 errors）

- [ ] **Step 3: 启动应用验证**

Run: `pnpm tauri dev`
Expected: 应用正常启动

- [ ] **Step 4: 验证全局配置**

1. 点击左下角"设置"按钮
2. 修改"全局最小触发间隔"为 60
3. 点击"保存"
4. 预期：弹出"已保存"Toast，关闭后重新打开设置，值保持 60

- [ ] **Step 5: 验证群聊创建**

1. 在会话列表点击右上角 "+" 按钮
2. 输入群名"测试群聊"
3. 选择至少 2 个角色
4. 点击"创建"
5. 预期：自动进入群聊，会话列表出现新群聊，右侧显示成员列表（用户 + 选中的角色）

- [ ] **Step 6: 验证私聊无成员列表**

1. 点击一个私聊会话
2. 预期：右侧不显示成员列表，聊天区域占满宽度

- [ ] **Step 7: Commit 验证结果**

```bash
git add -A
git commit -m "feat: complete global settings panel and group chat creation"
```

---

## Self-Review

### Spec Coverage Check

| Spec 要求 | 实现 Task |
|-----------|-----------|
| 全局最小触发间隔配置（批量保存） | Task 2, 3, 8, 9, 10 |
| 保存后 Toast "已保存" | Task 9 |
| 群聊创建弹窗（群名 + 角色多选） | Task 11 |
| 创建后直接进入群聊 | Task 11 |
| 群聊右侧成员列表 | Task 13 |
| 私聊无成员列表 | Task 13（条件渲染） |
| 会话列表新增群聊入口 | Task 12 |
| Rust 单元测试 | Task 14 |

**无遗漏。**

### Placeholder Scan

- 无 "TBD"、"TODO"、"implement later"
- 所有代码步骤包含完整代码
- 无 "Similar to Task N" 引用

### Type Consistency

- `CreateGroupSessionRequest` / `GroupMemberResponse` 在 Task 1、4、5 中名称一致
- `SettingsResponse` / `UpdateAppSettingsRequest` 在 Task 2、3 中名称一致
- `AppSettings` interface 在 Task 8 中字段与后端 `SettingsResponse` 对应

---

*Plan version: v1.0*  
*Date: 2026-05-11*

