# 会话管理、头像上传与人设自生成 UI 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现群聊解散/彻底删除/配置简化、Agent-Agent历史禁用输入、头像上传、人设自生成UI占位。

**Architecture:** 后端新增 `is_dissolved` 字段区分解散与删除；前端根据 mode 和 dissolved 状态控制UI显示与输入；头像通过 base64 上传保存到 `data/avatars/`。

**Tech Stack:** Tauri v2 + Rust + SQLite + Svelte 5 + TailwindCSS v4

---

## 文件变更清单

### 后端 (Rust)
- `src-tauri/src/db/schema.rs` — Migration V9: `group_sessions.is_dissolved`
- `src-tauri/src/db/migration.rs` — 注册 V9
- `src-tauri/src/db/session.rs` — `disband_group` 改逻辑；`list_sessions` / `get_session_by_id` 返回 `is_dissolved`
- `src-tauri/src/models/session.rs` — `SessionResponse` / `GroupSession` 加 `is_dissolved`
- `src-tauri/src/commands/session.rs` — `disband_group` 不变（调用 repo）
- `src-tauri/src/commands/message.rs` — `send_user_message` / `send_history_message` 检查 `is_dissolved`
- `src-tauri/src/commands/agent.rs` — 新增 `upload_avatar` 命令
- `src-tauri/src/db/agent.rs` — 新增 `update_avatar_path`
- `src-tauri/src/lib.rs` — 注册 `upload_avatar` 命令

### 前端 (Svelte/TypeScript)
- `src/lib/types.ts` — `Session` 加 `is_dissolved`
- `src/lib/stores/sessionStore.svelte.ts` — `loadSessions` 过滤 dissolved 群聊
- `src/lib/components/ChatView.svelte` — dissolved 禁用输入；history mode Agent-Agent 禁用输入
- `src/lib/components/SessionSettingsPanel.svelte` — 新增 `mode` prop；history 模式简化配置
- `src/lib/components/HistorySessionList.svelte` — 群聊右键菜单增加"彻底删除"
- `src/lib/components/AgentDetail.svelte` — 头像点击弹窗；人设自生成按钮
- `src/lib/components/CreateAgentModal.svelte` — 头像上传；人设自生成展开/折叠
- `src/lib/components/SettingsPanel.svelte` — 用户头像上传
- `src/lib/components/AvatarUploadModal.svelte` — 新建：头像查看+上传弹窗
- `src/lib/components/PersonaGenerateModal.svelte` — 新建：人设自生成占位弹窗
- `src/App.svelte` — 可能需要调整 settings panel 的调用

---

## Task 1: Migration V9 — 添加 `is_dissolved` 字段

**Files:**
- Modify: `src-tauri/src/db/schema.rs`
- Modify: `src-tauri/src/db/migration.rs`
- Test: `cargo check --tests`

- [ ] **Step 1: 添加 Migration V9 SQL**

在 `schema.rs` 末尾添加：

```rust
pub const MIGRATION_V9: &str = r#"
-- V9: 群聊解散支持
ALTER TABLE group_sessions ADD COLUMN is_dissolved INTEGER DEFAULT 0 CHECK(is_dissolved IN (0, 1));
"#;
```

- [ ] **Step 2: 注册 Migration V9**

在 `migration.rs` 的 `apply_migrations` 函数中添加 V9 的注册逻辑（参考 V8 的写法）。

- [ ] **Step 3: 编译验证**

Run: `cd src-tauri && cargo check`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db/schema.rs src-tauri/src/db/migration.rs
git commit -m "feat: add migration V9 for group session dissolve"
```

---

## Task 2: 后端模型和查询更新

**Files:**
- Modify: `src-tauri/src/models/session.rs`
- Modify: `src-tauri/src/db/session.rs`
- Test: `cargo check --tests`

- [ ] **Step 1: 模型加字段**

`SessionResponse` 和 `GroupSession` 增加：
```rust
pub is_dissolved: bool,
```

- [ ] **Step 2: `disband_group` 改逻辑**

把 `db/session.rs` 的 `disband_group` 从：
```rust
UPDATE sessions SET is_deleted = 1, deleted_at = ?2 WHERE id = ?1 AND session_type = 'group'
```
改为：
```rust
UPDATE group_sessions SET is_dissolved = 1 WHERE session_id = ?1
```

- [ ] **Step 3: `list_sessions` 查询返回 `is_dissolved`**

在 `list_sessions` 的 SQL 中，JOIN `group_sessions` 时增加 `COALESCE(gs.is_dissolved, 0)`，并在构造 `SessionResponse` 时设置 `is_dissolved`。

- [ ] **Step 4: `get_session_by_id` 返回 `is_dissolved`**

同样的修改。

- [ ] **Step 5: 编译验证**

Run: `cd src-tauri && cargo check --tests`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/models/session.rs src-tauri/src/db/session.rs
git commit -m "feat: backend support for group session dissolve"
```

---

## Task 3: 后端消息发送检查

**Files:**
- Modify: `src-tauri/src/commands/message.rs`
- Test: `cargo check --tests`

- [ ] **Step 1: `send_user_message` 检查 dissolved**

在消息插入前，查询 session 的 `is_dissolved`，如果为 true 返回错误 `"该群聊已解散，无法发送消息"`。

- [ ] **Step 2: `send_history_message` 同样检查**

- [ ] **Step 3: 编译验证**

Run: `cd src-tauri && cargo check --tests`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/message.rs
git commit -m "feat: reject messages to dissolved group sessions"
```

---

## Task 4: 前端类型和 Store 更新

**Files:**
- Modify: `src/lib/types.ts`
- Modify: `src/lib/stores/sessionStore.svelte.ts`
- Test: Vitest + manual check

- [ ] **Step 1: `Session` 类型加 `is_dissolved`**

```typescript
export interface Session {
    // ... existing fields
    is_dissolved?: boolean;
}
```

- [ ] **Step 2: `sessionStore.loadSessions` 过滤 dissolved**

加载后过滤掉 `session_type === 'group' && s.is_dissolved` 的会话。

- [ ] **Step 3: 运行前端测试**

Run: `pnpm vitest run`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/lib/types.ts src/lib/stores/sessionStore.svelte.ts
git commit -m "feat: filter dissolved groups from current session list"
```

---

## Task 5: ChatView 禁用输入 + 历史模式 Agent-Agent 禁用

**Files:**
- Modify: `src/lib/components/ChatView.svelte`
- Test: Vitest + Playwright

- [ ] **Step 1: dissolved 禁用输入**

在输入区判断前增加：
```svelte
{#if selectedSession?.session_type === 'group' && selectedSession?.is_dissolved}
    <div class="shrink-0 border-t border-border p-4 bg-surface text-center text-sm text-text-secondary">
        该群聊已解散，无法发送消息
    </div>
{:else if isAgentAgentPrivate}
    <!-- 原有 Agent-Agent 禁用 -->
{:else}
    <!-- 正常输入 -->
{/if}
```

- [ ] **Step 2: 历史模式 Agent-Agent 禁用**

修改 `isAgentAgentPrivate` 的判断逻辑，或者增加条件：
```svelte
{#if (mode === 'chat' && isAgentAgentPrivate) || (mode === 'history' && isAgentAgentPrivate)}
```

更简洁的做法是把输入区渲染条件改为：
```svelte
{#if isAgentAgentPrivate || (selectedSession?.session_type === 'group' && selectedSession?.is_dissolved)}
```

- [ ] **Step 3: 运行测试**

Run: `pnpm vitest run`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/lib/components/ChatView.svelte
git commit -m "feat: disable input for dissolved groups and agent-agent history"
```

---

## Task 6: SessionSettingsPanel 配置简化 + 历史彻底删除

**Files:**
- Modify: `src/lib/components/SessionSettingsPanel.svelte`
- Modify: `src/lib/components/HistorySessionList.svelte`
- Modify: `src/lib/components/ChatView.svelte` (props 传递)
- Test: Vitest

- [ ] **Step 1: SessionSettingsPanel 新增 `mode` prop**

```typescript
interface Props {
    open: boolean;
    sessionId: string;
    sessionType: string;
    members: GroupMember[];
    mode?: 'chat' | 'history';  // 新增
    onClose: () => void;
    onMembersChange: () => void;
}
```

- [ ] **Step 2: 条件渲染**

- 禁言开关：`{#if mode !== 'history'}`
- 成员管理的移除按钮：`{#if mode !== 'history' && member.participant_type === 'agent'}`
- 添加成员按钮：`{#if mode !== 'history'}`
- 重置按钮：`{#if mode !== 'history'}`
- 解散按钮：`{#if mode !== 'history'}`

- [ ] **Step 3: ChatView 传递 `mode`**

`<SessionSettingsPanel ... mode={mode} />`

- [ ] **Step 4: HistorySessionList 右键菜单**

给群聊标签添加 `@contextmenu` 事件：
```svelte
<div oncontextmenu={(e) => { e.preventDefault(); showContextMenu = true; menuX = e.clientX; menuY = e.clientY; }}>
```

右键菜单内容：
- 彻底删除（调用 `invoke('delete_session', { id: session.id })`）

- [ ] **Step 5: 运行测试**

Run: `pnpm vitest run`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/lib/components/SessionSettingsPanel.svelte src/lib/components/HistorySessionList.svelte src/lib/components/ChatView.svelte
git commit -m "feat: simplify history session settings and add right-click delete"
```

---

## Task 7: Agent-Agent 历史会话禁用输入（补充确认）

已在 Task 5 中完成，无需额外步骤。

---

## Task 8: 后端头像上传命令

**Files:**
- Modify: `src-tauri/src/commands/agent.rs` (或新建 `src-tauri/src/commands/upload.rs`)
- Modify: `src-tauri/src/db/agent.rs`
- Modify: `src-tauri/src/db/session.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `cargo check --tests`

- [ ] **Step 1: 新建上传命令模块**

新建 `src-tauri/src/commands/upload.rs`：

```rust
use tauri::State;
use crate::db::connection::{get_db, DbState};
use base64::{Engine as _, engine::general_purpose};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, serde::Deserialize)]
pub struct UploadAvatarRequest {
    pub target_type: String, // "user", "agent", "group"
    pub target_id: String,
    pub image_data_base64: String,
}

#[tauri::command]
pub async fn upload_avatar(
    state: State<'_, DbState>,
    req: UploadAvatarRequest,
) -> Result<String, String> {
    let conn = get_db(&state).await?;
    
    // 1. 确定保存目录
    let app_dir = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("No exe dir")?
        .join("data")
        .join("avatars")
        .join(&req.target_type);
    
    fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;
    
    // 2. 解析 base64（去除 data:image/xxx;base64, 前缀）
    let base64_data = if let Some(idx) = req.image_data_base64.find(',') {
        &req.image_data_base64[idx + 1..]
    } else {
        &req.image_data_base64
    };
    
    let image_bytes = general_purpose::STANDARD.decode(base64_data).map_err(|e| e.to_string())?;
    
    // 3. 判断格式（简单检查 magic bytes）
    let ext = if image_bytes.starts_with(b"\x89PNG") {
        "png"
    } else if image_bytes.starts_with(b"\xff\xd8") {
        "jpg"
    } else {
        "png"
    };
    
    let filename = format!("{}.{}" , req.target_id, ext);
    let filepath = app_dir.join(&filename);
    fs::write(&filepath, image_bytes).map_err(|e| e.to_string())?;
    
    // 4. 更新数据库
    let relative_path = format!("data/avatars/{}/{}" , req.target_type, filename);
    match req.target_type.as_str() {
        "agent" => {
            conn.execute("UPDATE agents SET avatar_path = ?1 WHERE id = ?2", (&relative_path, &req.target_id)).map_err(|e| e.to_string())?;
        }
        "group" => {
            conn.execute("UPDATE group_sessions SET avatar_path = ?1 WHERE session_id = ?2", (&relative_path, &req.target_id)).map_err(|e| e.to_string())?;
        }
        "user" => {
            conn.execute("UPDATE user_personas SET avatar_path = ?1 WHERE is_default = 1", [&relative_path]).map_err(|e| e.to_string())?;
        }
        _ => return Err("Invalid target_type".to_string()),
    }
    
    Ok(relative_path)
}
```

- [ ] **Step 2: 在 `lib.rs` 中注册命令**

```rust
mod commands {
    pub mod upload;
    // ...
}

// generate_handler! 中加入 upload_avatar
```

- [ ] **Step 3: 添加 base64 依赖**

检查 `Cargo.toml` 是否已有 `base64` crate，没有则添加。

- [ ] **Step 4: 编译验证**

Run: `cd src-tauri && cargo check --tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/upload.rs src-tauri/src/lib.rs src-tauri/Cargo.toml
git commit -m "feat: backend avatar upload command"
```

---

## Task 9: 头像上传弹窗组件

**Files:**
- Create: `src/lib/components/AvatarUploadModal.svelte`
- Test: Vitest

- [ ] **Step 1: 创建组件**

```svelte
<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { X, Upload } from 'lucide-svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';

    interface Props {
        open: boolean;
        targetType: 'user' | 'agent' | 'group';
        targetId: string;
        currentAvatar: string | null;
        onClose: () => void;
        onUploaded: (path: string) => void;
    }

    let { open, targetType, targetId, currentAvatar, onClose, onUploaded }: Props = $props();
    let uploading = $state(false);
    let fileInput: HTMLInputElement;

    function handleFileSelect(e: Event) {
        const file = (e.target as HTMLInputElement).files?.[0];
        if (!file) return;
        const reader = new FileReader();
        reader.onload = async (ev) => {
            const base64 = ev.target?.result as string;
            if (!base64) return;
            uploading = true;
            try {
                const path = await invoke<string>('upload_avatar', {
                    req: { target_type: targetType, target_id: targetId, image_data_base64: base64 }
                });
                toastStore.show('头像上传成功', 'success', 2000);
                onUploaded(path);
            } catch (err) {
                toastStore.show('上传失败: ' + String(err), 'error', 5000);
            } finally {
                uploading = false;
            }
        };
        reader.readAsDataURL(file);
    }
</script>

{#if open}
    <div class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center" onclick={onClose}>
        <div class="bg-surface rounded-xl p-6 w-80 shadow-xl" onclick={(e) => e.stopPropagation()}>
            <div class="flex items-center justify-between mb-4">
                <h3 class="font-semibold">头像管理</h3>
                <button onclick={onClose} class="p-1 hover:bg-bg rounded"><X size={18} /></button>
            </div>
            <div class="flex flex-col items-center gap-4">
                {#if currentAvatar}
                    <img src={currentAvatar} alt="当前头像" class="w-24 h-24 rounded-full object-cover" />
                {:else}
                    <div class="w-24 h-24 rounded-full bg-primary/10 flex items-center justify-center text-primary">
                        <span class="text-2xl">?</span>
                    </div>
                {/if}
                <input type="file" accept="image/*" bind:this={fileInput} onchange={handleFileSelect} class="hidden" />
                <button
                    onclick={() => fileInput?.click()}
                    disabled={uploading}
                    class="flex items-center gap-2 px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-dark transition-colors disabled:opacity-50"
                >
                    <Upload size={16} />
                    {uploading ? '上传中...' : '上传新头像'}
                </button>
            </div>
        </div>
    </div>
{/if}
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/components/AvatarUploadModal.svelte
git commit -m "feat: avatar upload modal component"
```

---

## Task 10: Agent 头像和用户头像集成

**Files:**
- Modify: `src/lib/components/AgentDetail.svelte`
- Modify: `src/lib/components/CreateAgentModal.svelte`
- Modify: `src/lib/components/SettingsPanel.svelte`
- Test: Vitest

- [ ] **Step 1: AgentDetail 头像点击弹窗**

头像区域改为可点击，点击后打开 `AvatarUploadModal`，上传成功后更新 `agent.avatar_path`。

- [ ] **Step 2: CreateAgentModal 同样集成**

- [ ] **Step 3: SettingsPanel 用户头像**

在用户设置区域增加头像显示和上传按钮。

- [ ] **Step 4: Commit**

```bash
git add src/lib/components/AgentDetail.svelte src/lib/components/CreateAgentModal.svelte src/lib/components/SettingsPanel.svelte
git commit -m "feat: integrate avatar upload for agents and user"
```

---

## Task 11: 群聊头像集成

**Files:**
- Modify: `src/lib/components/SessionSettingsPanel.svelte`
- Test: Vitest

- [ ] **Step 1: 群聊设置中增加头像上传**

在群聊配置的成员管理上方增加群聊头像区域和"更换群聊头像"按钮。

- [ ] **Step 2: Commit**

```bash
git add src/lib/components/SessionSettingsPanel.svelte
git commit -m "feat: group chat avatar upload"
```

---

## Task 12: 人设自生成 UI — AgentDetail

**Files:**
- Create: `src/lib/components/PersonaGenerateModal.svelte`
- Modify: `src/lib/components/AgentDetail.svelte`
- Test: Vitest

- [ ] **Step 1: 创建占位弹窗**

```svelte
<script lang="ts">
    import { X } from 'lucide-svelte';
    interface Props { open: boolean; onClose: () => void; }
    let { open, onClose }: Props = $props();
</script>

{#if open}
    <div class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center" onclick={onClose}>
        <div class="bg-surface rounded-xl p-6 w-96 shadow-xl" onclick={(e) => e.stopPropagation()}>
            <div class="flex items-center justify-between mb-4">
                <h3 class="font-semibold">人设自生成</h3>
                <button onclick={onClose} class="p-1 hover:bg-bg rounded"><X size={18} /></button>
            </div>
            <p class="text-sm text-text-secondary">功能开发中...</p>
        </div>
    </div>
{/if}
```

- [ ] **Step 2: AgentDetail 增加按钮**

在保存按钮旁边增加"人设自生成"按钮，点击打开弹窗。

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/PersonaGenerateModal.svelte src/lib/components/AgentDetail.svelte
git commit -m "feat: persona auto-generate UI placeholder for agent detail"
```

---

## Task 13: 人设自生成 UI — CreateAgentModal

**Files:**
- Modify: `src/lib/components/CreateAgentModal.svelte`
- Test: Vitest

- [ ] **Step 1: 新增展开/折叠逻辑**

```typescript
let showGenerateFields = $state(false);
let referenceCharacter = $state('');
let additionalInfo = $state('');
```

- [ ] **Step 2: UI 布局**

在表单底部增加：
- "人设自生成"按钮，点击切换 `showGenerateFields`
- 条件渲染：
  - "参考角色"输入框
  - "补充信息"输入框
  - "生成"按钮（disabled）

- [ ] **Step 3: 运行测试**

Run: `pnpm vitest run`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/lib/components/CreateAgentModal.svelte
git commit -m "feat: persona auto-generate UI placeholder for create agent"
```

---

## Spec 覆盖检查

| 需求 | 对应 Task |
|------|----------|
| 群聊解散 (is_dissolved) | Task 1, 2, 3, 4, 5 |
| 解散后当前页归历史 | Task 4 (前端过滤) |
| 历史解散群聊不支持对话 | Task 3 (后端拒绝), Task 5 (前端禁用) |
| 二次确认 | 已有 ConfirmDialog |
| 历史彻底删除 | Task 6 |
| 历史配置简化 | Task 6 |
| Agent-Agent 历史禁用输入 | Task 5 |
| 头像上传 (用户/Agent/群聊) | Task 8, 9, 10, 11 |
| 人设自生成 UI | Task 12, 13 |

无遗漏。
