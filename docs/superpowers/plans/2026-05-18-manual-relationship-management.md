# 手动好友关系管理 + 群聊关系 Bug 修复 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在角色关系设定中支持手动添加/删除好友关系，并修复群聊拉人时错误建立好友关系的 Bug。

**Architecture:** 最小改动方案。Backend 新增 `add_friendships` / `remove_friendship` Tauri Commands 和对应的 repository 函数；前端新增两个弹窗组件并集成到 `AgentRelationshipPanel`；Bug 修复仅需删除 `add_group_member` 中遍历插入 `friendships` 的循环。

**Tech Stack:** Rust (Tauri v2, rusqlite), Svelte 5, TailwindCSS v4

---

## 文件映射

| 文件 | 职责 |
|------|------|
| `src-tauri/src/db/session.rs` | Bug 修复：移除 `add_group_member` 中的 friendships 插入 |
| `src-tauri/src/db/agent_relationship.rs` | 新增 `add_friendship` / `remove_friendship` repository 函数 |
| `src-tauri/src/commands/agent_relationship.rs` | 新增 `add_friendships` / `remove_friendship` Tauri Commands |
| `src-tauri/src/lib.rs` | 注册新 Commands |
| `src/lib/components/AddRelationshipModal.svelte` | 多选卡片弹窗：选择目标角色并批量添加好友 |
| `src/lib/components/ConfirmDeleteRelationshipModal.svelte` | 删除确认弹窗：提示双向删除及降级逻辑 |
| `src/lib/components/AgentRelationshipPanel.svelte` | 集成"添加关系"按钮、删除图标、弹窗开关 |

---

### Task 1: Bug 修复 — 移除 `add_group_member` 中的 friendships 插入

**Files:**
- Modify: `src-tauri/src/db/session.rs:556-589`

**说明：** 当前 `add_group_member` 在插入 `group_members` 后，遍历群里其他 Agents 并向 `friendships` 表插入双向记录。群聊拉人不应自动建立好友关系，需删除这段逻辑。

- [ ] **Step 1: 删除 friendships 插入循环**

打开 `src-tauri/src/db/session.rs`，定位 `add_group_member` 函数。删除以下代码：

```rust
    let other_agents: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT participant_id FROM group_members
             WHERE session_id = ?1 AND participant_type = 'agent' AND participant_id != ?2"
        )?;
        let rows = stmt.query_map([session_id, agent_id], |row| row.get(0))?;
        rows.filter_map(|r| r.ok()).collect()
    };

    for other_id in other_agents {
        conn.execute(
            "INSERT OR IGNORE INTO friendships (id, agent_id_1, agent_id_2, participant_type_2, created_at, source_session_id)
             VALUES (?1, ?2, ?3, 'agent', ?4, ?5)",
            rusqlite::params![uuid::Uuid::new_v4().to_string(), agent_id, &other_id, now, session_id],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO friendships (id, agent_id_1, agent_id_2, participant_type_2, created_at, source_session_id)
             VALUES (?1, ?2, ?3, 'agent', ?4, ?5)",
            rusqlite::params![uuid::Uuid::new_v4().to_string(), &other_id, agent_id, now, session_id],
        )?;
    }
```

只保留 `group_members` 的插入和事务提交。

- [ ] **Step 2: 编译验证**

Run: `cd src-tauri && cargo check`
Expected: PASS（无编译错误）

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/db/session.rs
git commit -m "fix: remove incorrect friendships insertion in add_group_member"
```

---

### Task 2: Repository 层 — 新增 `add_friendship` / `remove_friendship`

**Files:**
- Modify: `src-tauri/src/db/agent_relationship.rs`

- [ ] **Step 1: 在文件末尾追加两个函数**

在 `src-tauri/src/db/agent_relationship.rs` 中，`delete_relationships_by_target` 函数之后追加：

```rust
pub fn add_friendship(conn: &Connection, agent_id_1: &str, agent_id_2: &str) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT OR IGNORE INTO friendships (id, agent_id_1, agent_id_2, participant_type_2, created_at, source_session_id)
         VALUES (?1, ?2, ?3, 'agent', ?4, NULL)",
        rusqlite::params![uuid::Uuid::new_v4().to_string(), agent_id_1, agent_id_2, now],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO friendships (id, agent_id_1, agent_id_2, participant_type_2, created_at, source_session_id)
         VALUES (?1, ?2, ?3, 'agent', ?4, NULL)",
        rusqlite::params![uuid::Uuid::new_v4().to_string(), agent_id_2, agent_id_1, now],
    )?;
    Ok(())
}

pub fn remove_friendship(conn: &Connection, agent_id_1: &str, agent_id_2: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM friendships WHERE agent_id_1 = ?1 AND agent_id_2 = ?2 AND participant_type_2 = 'agent'",
        (agent_id_1, agent_id_2),
    )?;
    conn.execute(
        "DELETE FROM friendships WHERE agent_id_1 = ?1 AND agent_id_2 = ?2 AND participant_type_2 = 'agent'",
        (agent_id_2, agent_id_1),
    )?;
    Ok(())
}
```

- [ ] **Step 2: 编译验证**

Run: `cd src-tauri && cargo check`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/db/agent_relationship.rs
git commit -m "feat(db): add add_friendship and remove_friendship repository functions"
```

---

### Task 3: Backend Commands — 新增 `add_friendships` / `remove_friendship`

**Files:**
- Modify: `src-tauri/src/commands/agent_relationship.rs`

- [ ] **Step 1: 在现有 Command 下方追加两个 Command**

打开 `src-tauri/src/commands/agent_relationship.rs`，在 `update_agent_relationship` 之后追加：

```rust
#[tauri::command]
pub async fn add_friendships(
    state: tauri::State<'_, crate::db::DbState>,
    observer_id: String,
    target_ids: Vec<String>,
) -> Result<(), String> {
    let mut conn = state.conn.lock().await;
    let conn = conn.as_mut().map_err(|e| e.to_string())?;
    for target_id in target_ids {
        crate::db::agent_relationship::add_friendship(conn, &observer_id, &target_id)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn remove_friendship(
    state: tauri::State<'_, crate::db::DbState>,
    observer_id: String,
    target_id: String,
) -> Result<(), String> {
    let mut conn = state.conn.lock().await;
    let conn = conn.as_mut().map_err(|e| e.to_string())?;
    crate::db::agent_relationship::remove_friendship(conn, &observer_id, &target_id)
        .map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 2: 编译验证**

Run: `cd src-tauri && cargo check`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/agent_relationship.rs
git commit -m "feat(commands): add add_friendships and remove_friendship Tauri commands"
```

---

### Task 4: 注册 Commands

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 导入新函数并在 handler 中注册**

打开 `src-tauri/src/lib.rs`：

1. 找到 `use commands::agent_relationship::{list_agent_relationships, update_agent_relationship};`，修改为：

```rust
use commands::agent_relationship::{list_agent_relationships, update_agent_relationship, add_friendships, remove_friendship};
```

2. 找到 `generate_handler!` 宏中的 `list_agent_relationships,` 和 `update_agent_relationship,`，在其后追加：

```rust
            add_friendships,
            remove_friendship,
```

- [ ] **Step 2: 编译验证**

Run: `cd src-tauri && cargo check`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: register add_friendships and remove_friendship commands"
```

---

### Task 5: 前端 — 新增 `AddRelationshipModal.svelte`

**Files:**
- Create: `src/lib/components/AddRelationshipModal.svelte`

- [ ] **Step 1: 创建弹窗组件文件**

创建 `src/lib/components/AddRelationshipModal.svelte`，写入完整代码：

```svelte
<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { agentStore } from '$lib/stores/agentStore.svelte';
    import { logger } from '$lib/logger';
    import { resolveAvatarUrl } from '$lib/utils';
    import { User, X, Plus } from 'lucide-svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';

    interface Props {
        open: boolean;
        observerId: string;
        existingFriendIds: string[];
        onClose: () => void;
        onAdded: () => void;
    }

    let { open, observerId, existingFriendIds, onClose, onAdded }: Props = $props();
    let selectedIds = $state<string[]>([]);
    let loading = $state(false);

    $effect(() => {
        if (open) {
            selectedIds = [];
            agentStore.loadAgents();
        }
    });

    const availableAgents = $derived(
        agentStore.agents.filter(a => a.id !== observerId && !existingFriendIds.includes(a.id) && !a.is_deleted)
    );

    function toggleAgent(id: string) {
        if (selectedIds.includes(id)) {
            selectedIds = selectedIds.filter(x => x !== id);
        } else {
            selectedIds = [...selectedIds, id];
        }
    }

    async function handleAdd() {
        if (selectedIds.length === 0) return;
        loading = true;
        try {
            await invoke('add_friendships', { observerId, targetIds: selectedIds });
            selectedIds = [];
            onAdded();
            onClose();
        } catch (err) {
            logger.error('Failed to add friendships:', err);
            toastStore.error('添加关系失败: ' + String(err));
        } finally {
            loading = false;
        }
    }
</script>

{#if open}
    <div class="fixed inset-0 z-[100] flex items-center justify-center bg-black/50" onclick={onClose} role="dialog" aria-modal="true">
        <div class="bg-surface rounded-xl p-6 w-[28rem] max-w-full shadow-lg border border-border" onclick={(e) => e.stopPropagation()}>
            <div class="flex items-center justify-between mb-2">
                <h3 class="text-lg font-semibold">添加关系</h3>
                <button onclick={onClose} class="p-1 hover:bg-bg rounded-lg"><X size={18} /></button>
            </div>
            <p class="text-xs text-text-secondary mb-4">添加关系是双向的，被添加的角色关系列表中也会增加该角色。</p>
            <div class="max-h-64 overflow-y-auto grid grid-cols-2 gap-2 mb-4">
                {#each availableAgents as agent}
                    <button
                        onclick={() => toggleAgent(agent.id)}
                        class="flex items-center gap-2 p-2 rounded-lg border border-border text-left transition-colors {selectedIds.includes(agent.id) ? 'bg-primary/10 border-primary' : 'hover:bg-bg'}"
                    >
                        <div class="w-8 h-8 rounded-full bg-primary/10 flex items-center justify-center text-primary shrink-0 overflow-hidden">
                            {#if agent.avatar_path}
                                <img src={resolveAvatarUrl(agent.avatar_path)} alt={agent.name} class="w-full h-full object-cover" />
                            {:else}
                                <User size={16} />
                            {/if}
                        </div>
                        <span class="text-sm truncate">{agent.name}</span>
                    </button>
                {:else}
                    <p class="text-sm text-text-secondary col-span-2 py-4 text-center">没有可添加的角色</p>
                {/each}
            </div>
            <div class="flex gap-2">
                <button onclick={onClose} class="flex-1 py-2 bg-bg text-text-primary rounded-lg hover:bg-surface border border-border">
                    取消
                </button>
                <button
                    onclick={handleAdd}
                    disabled={selectedIds.length === 0 || loading}
                    class="flex-1 py-2 bg-primary text-white rounded-lg hover:bg-primary-dark disabled:opacity-50"
                >
                    {loading ? '添加中...' : `添加 (${selectedIds.length})`}
                </button>
            </div>
        </div>
    </div>
{/if}
```

**注意：** 如果 `toastStore` 不存在（项目中可能使用其他 toast 机制），请替换为项目中实际使用的 toast 调用方式。根据 `CHAT-20`，项目已有 `Toast` 全局通知，通过 `toastStore` 调用。若 `toastStore` 不存在，则在 `src/lib/stores/toastStore.svelte.ts` 中确认或改用 `logger` + 弹窗内错误文本。

- [ ] **Step 2: Commit**

```bash
git add src/lib/components/AddRelationshipModal.svelte
git commit -m "feat(ui): add AddRelationshipModal component for multi-select friend addition"
```

---

### Task 6: 前端 — 新增 `ConfirmDeleteRelationshipModal.svelte`

**Files:**
- Create: `src/lib/components/ConfirmDeleteRelationshipModal.svelte`

- [ ] **Step 1: 创建弹窗组件文件**

创建 `src/lib/components/ConfirmDeleteRelationshipModal.svelte`：

```svelte
<script lang="ts">
    import { X, AlertTriangle } from 'lucide-svelte';

    interface Props {
        open: boolean;
        targetName: string;
        onClose: () => void;
        onConfirm: () => void;
    }

    let { open, targetName, onClose, onConfirm }: Props = $props();
    let loading = $state(false);

    async function handleConfirm() {
        loading = true;
        try {
            await onConfirm();
        } finally {
            loading = false;
        }
    }
</script>

{#if open}
    <div class="fixed inset-0 z-[100] flex items-center justify-center bg-black/50" onclick={onClose} role="dialog" aria-modal="true">
        <div class="bg-surface rounded-xl p-6 w-96 max-w-full shadow-lg border border-border" onclick={(e) => e.stopPropagation()}>
            <div class="flex items-center gap-2 mb-3">
                <AlertTriangle size={20} class="text-red-500" />
                <h3 class="text-lg font-semibold">删除关系</h3>
            </div>
            <p class="text-sm text-text-primary mb-1">确定要删除与 <strong>{targetName}</strong> 的好友关系吗？</p>
            <p class="text-xs text-text-secondary mb-5">
                删除关系是双向的，双方的关系列表中都会移除对方。如果两个角色仍在同一个群中，关系将降级为群友。
            </p>
            <div class="flex gap-2">
                <button onclick={onClose} class="flex-1 py-2 bg-bg text-text-primary rounded-lg hover:bg-surface border border-border">
                    取消
                </button>
                <button
                    onclick={handleConfirm}
                    disabled={loading}
                    class="flex-1 py-2 bg-red-500 text-white rounded-lg hover:bg-red-600 disabled:opacity-50"
                >
                    {loading ? '删除中...' : '确认删除'}
                </button>
            </div>
        </div>
    </div>
{/if}
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/components/ConfirmDeleteRelationshipModal.svelte
git commit -m "feat(ui): add ConfirmDeleteRelationshipModal component"
```

---

### Task 7: 前端 — 修改 `AgentRelationshipPanel.svelte`

**Files:**
- Modify: `src/lib/components/AgentRelationshipPanel.svelte`

- [ ] **Step 1: 导入新增组件和图标**

在 `<script>` 顶部追加导入：

```typescript
import AddRelationshipModal from './AddRelationshipModal.svelte';
import ConfirmDeleteRelationshipModal from './ConfirmDeleteRelationshipModal.svelte';
import { Plus, X } from 'lucide-svelte';
import { toastStore } from '$lib/stores/toastStore.svelte';
```

（若 `toastStore` 路径或名称不同，请按项目实际调整。）

- [ ] **Step 2: 新增状态变量**

在 `let saveTimeouts = ...` 之后追加：

```typescript
let showAddModal = $state(false);
let showDeleteModal = $state(false);
let deleteTarget = $state<RelationshipItem | null>(null);
```

- [ ] **Step 3: 新增删除处理函数**

在 `handleBlur` 之后追加：

```typescript
async function handleRemove(item: RelationshipItem) {
    try {
        await invoke('remove_friendship', {
            observerId: agentId,
            targetId: item.target_id,
        });
        logger.debug('[DEBUG AgentRelationshipPanel] removed', { agentId, targetId: item.target_id });
        loadRelationships();
    } catch (err) {
        logger.error('Failed to remove friendship:', err);
        toastStore.error('删除关系失败: ' + String(err));
    }
}

function openDeleteModal(item: RelationshipItem) {
    deleteTarget = item;
    showDeleteModal = true;
}
```

- [ ] **Step 4: 修改关系卡片模板 — 添加删除图标**

找到关系卡片的 `<!-- Info -->` div（约第 100 行），在 `<div class="flex items-center gap-2 mb-1.5">` 内部，`<span class="text-[10px] ...">{item.target_label}</span>` 之后追加：

```svelte
                        {#if item.target_label === '好友'}
                            <button
                                onclick={() => openDeleteModal(item)}
                                class="ml-auto p-1 text-red-400 hover:text-red-600 hover:bg-red-50 rounded-md transition-colors"
                                title="删除关系"
                            >
                                <X size={14} />
                            </button>
                        {/if}
```

- [ ] **Step 5: 在列表底部添加"添加关系"按钮**

在 `</div>`（`space-y-3` 的闭合标签）和 `{:else}` 的空列表提示之间，以及列表内容之后，找到 `{#each items as item ...}` 的闭合 `{/each}`。在 `{/each}` 之后、`</div>`（`space-y-3` 闭合）之前插入：

```svelte
                <button
                    onclick={() => showAddModal = true}
                    class="w-full flex items-center justify-center gap-2 p-3 border border-dashed border-border rounded-lg text-text-secondary hover:text-primary hover:border-primary hover:bg-primary/5 transition-colors"
                >
                    <Plus size={16} />
                    <span class="text-sm">添加关系</span>
                </button>
```

若列表为空（`{:else if items.length === 0}`），在其提示语的 `<p class="mt-1">` 下方也添加此按钮：

```svelte
            <button
                onclick={() => showAddModal = true}
                class="mt-4 inline-flex items-center gap-2 px-4 py-2 bg-primary text-white text-sm rounded-lg hover:bg-primary-dark transition-colors"
            >
                <Plus size={16} />
                添加关系
            </button>
```

- [ ] **Step 6: 在组件最底部添加两个弹窗实例**

在 `</div>`（组件最外层闭合）之前追加：

```svelte
<AddRelationshipModal
    open={showAddModal}
    observerId={agentId}
    existingFriendIds={items.filter(i => i.target_label === '好友' && i.target_type === 'agent').map(i => i.target_id)}
    onClose={() => showAddModal = false}
    onAdded={() => { showAddModal = false; loadRelationships(); }}
/>

{#if deleteTarget}
    <ConfirmDeleteRelationshipModal
        open={showDeleteModal}
        targetName={deleteTarget.target_name}
        onClose={() => { showDeleteModal = false; deleteTarget = null; }}
        onConfirm={async () => {
            await handleRemove(deleteTarget!);
            showDeleteModal = false;
            deleteTarget = null;
        }}
    />
{/if}
```

- [ ] **Step 7: 编译验证**

Run: `npx svelte-check --tsconfig ./tsconfig.json`
Expected: PASS（无类型错误）

- [ ] **Step 8: Commit**

```bash
git add src/lib/components/AgentRelationshipPanel.svelte
git commit -m "feat(ui): integrate add/remove friendship into AgentRelationshipPanel"
```

---

### Task 8: 端到端验证

- [ ] **Step 1: Rust 编译验证**

Run: `cd src-tauri && cargo check`
Expected: PASS

- [ ] **Step 2: 前端类型检查**

Run: `npx svelte-check --tsconfig ./tsconfig.json`
Expected: PASS

- [ ] **Step 3: 功能测试清单（手动验证）**

启动应用 `pnpm tauri dev`，按以下顺序验证：

1. **Bug 修复验证：**
   - 创建两个角色 A 和 B，确保它们之间没有私聊（即不是好友）。
   - 创建一个群聊，先加入 A，再加入 B。
   - 打开 A 的关系设定，确认 B 显示为"群友"而非"好友"。

2. **手动添加好友：**
   - 打开角色 A 的关系设定，点击"添加关系"。
   - 弹窗中应展示所有非好友角色（含群友和未关联角色）。
   - 多选 B 和另一个角色 C，点击"添加"。
   - 确认弹窗关闭，列表刷新，B 和 C 显示为"好友"，且关系描述为空文本框。
   - 打开 B 的关系设定，确认 A 也出现在 B 的好友列表中（双向生效）。

3. **手动删除好友：**
   - 在 A 的关系设定中，点击 B 卡片右上角的红色 X。
   - 确认弹窗显示正确的提示文字。
   - 点击"确认删除"。
   - 确认 B 从 A 的好友列表中消失。若 A 和 B 仍在同群，B 应以"群友"身份重新出现在列表中。
   - 打开 B 的关系设定，确认 A 也已从 B 的好友列表中移除。

4. **删除后主观描述保留：**
   - 在删除前给 B 写一段主观描述并保存。
   - 删除好友关系后，如果再将 B 添加回好友，确认之前写的主观描述仍然保留。

- [ ] **Step 4: 最终 Commit**

全部验证通过后：

```bash
git commit --allow-empty -m "feat: manual friendship management + group relation bug fix (AGT-16)"
```

---

## Self-Review Checklist

**1. Spec coverage:**
- [x] Bug 修复（`add_group_member` 移除 friendships 插入）→ Task 1
- [x] 手动添加好友（多选弹窗、双向生效）→ Task 2, 3, 5, 7
- [x] 手动删除好友（确认弹窗、双向生效、仅好友可删）→ Task 2, 3, 6, 7
- [x] 删除后若同群则降级为群友 → 由现有 SQL 查询天然实现（Task 1 修复后）
- [x] 主观描述保持为空/保留不删除 → 代码中未操作 `agent_relationships` 表
- [x] 错误提示通过 Toast → Task 5, 7

**2. Placeholder scan:**
- [x] 无 TBD / TODO / "implement later"
- [x] 所有步骤包含实际代码
- [x] 无 "Similar to Task N" 引用

**3. Type consistency：**
- [x] `add_friendships` 参数名 `observerId` / `targetIds` 与前端调用一致
- [x] `remove_friendship` 参数名 `observerId` / `targetId` 与前端调用一致
- [x] `RelationshipItem` 字段名未变更（`target_id`, `target_type`, `target_label`, `target_name`）
- [x] `Agent` 类型字段与 `agentStore.agents` 一致
