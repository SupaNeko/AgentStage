# 手动好友关系管理 + 群聊关系 Bug 修复

## 1. 背景与目标

当前 AgentStage 中，角色之间的关系通过以下三层数据表达：

1. **`friendships` 表**：硬性"好友"关系（agent-agent 或 agent-user）。
2. **`group_members` 表**：群成员关系（"群友"）。
3. **`agent_relationships` 表**：主观关系描述文本（每个 observer 对 target 的主观看法，如"他是我的好朋友"）。

**问题：** 当多个角色被创建但未被拉入同一个群聊时，它们之间互不可见。用户希望**手动**将任意两个角色添加为好友，使它们在 Prompt 中互相感知。

**Bug：** 将原本没关系的角色拉入已有群聊时，`add_group_member` 错误地向 `friendships` 表插入了双向记录，导致该角色被其他群成员显示为"好友"而非"群友"。

## 2. Bug 分析

**根因位置：** `src-tauri/src/db/session.rs:574-585`

```rust
for other_id in other_agents {
    conn.execute(
        "INSERT OR IGNORE INTO friendships ...",
        rusqlite::params![..., agent_id, &other_id, ..., session_id],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO friendships ...",
        rusqlite::params![..., &other_id, agent_id, ..., session_id],
    )?;
}
```

**问题：** `add_group_member` 在插入 `group_members` 后，遍历群里的其他 Agents 并向 `friendships` 表插入双向记录。这使得 `list_relationships_by_observer` 的"好友"UNION（第 92-106 行）命中这些记录，将本应显示为"群友"的关系错误标记为"好友"。

**修复：** 移除 `add_group_member` 中遍历插入 `friendships` 的逻辑。群聊拉人只应建立群成员关系，不应自动建立好友关系。

## 3. 方案概述

采用**最小改动方案**（方案 A）：

- 修复 bug：从 `add_group_member` 中移除错误的 `friendships` 插入。
- 新增 Backend API：`add_friendships`（批量添加双向好友）和 `remove_friendship`（双向删除好友）。
- 新增前端弹窗：`AddRelationshipModal`（多选卡片弹窗）和 `ConfirmDeleteRelationshipModal`（删除确认）。
- 主观关系描述（`agent_relationships`）在添加好友时保持为空，删除好友时保留不删除。

## 4. Backend 变更

### 4.1 Bug 修复

**文件：** `src-tauri/src/db/session.rs`

在 `add_group_member` 中删除第 565-585 行（遍历 `other_agents` 插入 `friendships` 的循环）。只保留 `group_members` 的插入。

### 4.2 新增 Repository 函数

**文件：** `src-tauri/src/db/agent_relationship.rs`

新增 `add_friendship`：
```rust
pub fn add_friendship(conn: &Connection, agent_id_1: &str, agent_id_2: &str) -> Result<()> {
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT OR IGNORE INTO friendships (id, agent_id_1, agent_id_2, participant_type_2, created_at, source_session_id) VALUES (?1, ?2, ?3, 'agent', ?4, NULL)",
        rusqlite::params![uuid::Uuid::new_v4().to_string(), agent_id_1, agent_id_2, now],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO friendships (id, agent_id_1, agent_id_2, participant_type_2, created_at, source_session_id) VALUES (?1, ?2, ?3, 'agent', ?4, NULL)",
        rusqlite::params![uuid::Uuid::new_v4().to_string(), agent_id_2, agent_id_1, now],
    )?;
    Ok(())
}
```

新增 `remove_friendship`：
```rust
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

### 4.3 新增 Tauri Commands

**文件：** `src-tauri/src/commands/agent_relationship.rs`

新增 `add_friendships`：
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
```

新增 `remove_friendship`：
```rust
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

### 4.4 Command 注册

**文件：** `src-tauri/src/lib.rs`

在 `tauri::generate_handler!` 中追加 `add_friendships` 和 `remove_friendship`。

## 5. 前端变更

### 5.1 `AgentRelationshipPanel.svelte`

**位置：** `src/lib/components/AgentRelationshipPanel.svelte`

改动点：
1. 导入新增的弹窗组件和 `Plus`、`X` 图标。
2. 在关系列表下方（空列表时也在对应位置）添加"添加关系"按钮。
3. 为每条 `target_label === '好友'` 的关系卡片右上角添加红色 `X` 删除图标。
4. 管理两个弹窗的开关状态：`showAddModal`、`showDeleteModal`、`deleteTarget`。
5. 删除成功后调用 `loadRelationships()` 刷新列表。

### 5.2 新增 `AddRelationshipModal.svelte`

**位置：** `src/lib/components/AddRelationshipModal.svelte`

Props：
```typescript
interface Props {
    open: boolean;
    observerId: string;
    existingFriendIds: string[]; // 已是好友的 Agent IDs（用于过滤）
    onClose: () => void;
    onAdded: () => void;
}
```

行为：
- 弹窗打开时调用 `list_agents` 获取全部 Agents。
- 过滤掉：当前 `observerId` 自身、已是好友的 Agents（`existingFriendIds`）、用户人设（`user_persona` 类型不在 `list_agents` 返回中，自然排除）。
- 群友和未关联的 Agents 可以选（支持将群友升级为好友）。
- 卡片网格布局，点击切换选中状态（背景变深 + 边框高亮）。
- 底部"取消" + "添加"按钮。未选中时"添加"置灰。
- 调用 `add_friendships(observerId, selectedIds)`，成功后 `onAdded()` + `onClose()`。

### 5.3 新增 `ConfirmDeleteRelationshipModal.svelte`

**位置：** `src/lib/components/ConfirmDeleteRelationshipModal.svelte`

Props：
```typescript
interface Props {
    open: boolean;
    targetName: string;
    onClose: () => void;
    onConfirm: () => void;
}
```

行为：
- 标题"删除关系"。
- 内容："删除关系是双向的，双方的关系列表中都会移除对方。如果两个角色仍在同一个群中，关系将降级为群友。"
- 底部"取消" + "确认删除"（红色按钮）。
- 调用 `onConfirm()` 后关闭。

## 6. 数据流

```
用户打开"关系设定"
  → AgentRelationshipPanel 调用 list_agent_relationships
    → 显示用户 + 好友 + 群友列表

用户点击"添加关系"
  → 打开 AddRelationshipModal
    → 调用 list_agents
      → 前端过滤（排除自身 + 已有好友）
        → 展示可选 Agent 卡片网格
          → 用户多选 → 点击"添加"
            → 调用 add_friendships(observerId, selectedIds)
              → 成功后 onAdded() → 刷新列表

用户点击好友卡片上的删除图标
  → 打开 ConfirmDeleteRelationshipModal
    → 用户点击"确认删除"
      → 调用 remove_friendship(observerId, targetId)
        → 成功后刷新列表
          → 若双方仍在同群，该角色自动以"群友"身份重新出现
```

## 7. 错误处理

- 所有后端错误（`add_friendships`、`remove_friendship`、`list_agents` 加载失败）统一通过前端已有的 Toast/Notification 卡片机制在页面上方弹出提示。
- 加载 Agents 列表失败时弹出卡片提示，不提供重试按钮。
- 添加/删除失败时不关闭弹窗，用户可再次尝试或取消。

## 8. 边界情况

| 场景 | 预期行为 |
|------|----------|
| 添加已是好友的 Agent | `INSERT OR IGNORE` 跳过，无错误 |
| 删除后双方仍在同群 | `friendships` 记录删除，但 `group_members` 仍在，列表中自动降级为"群友" |
| 删除后双方不在同群且无其他关联 | 该 Agent 从列表中完全消失（直到进入同群或手动添加好友） |
| 删除好友时保留主观描述 | `agent_relationships` 记录不删除，若未来重新添加好友，描述文本仍在 |
| 群聊中两个 Agent 原本没有关系 | 显示为"群友"（bug 修复后） |
| 用户将群友手动添加为好友 | 显示为"好友"，好友关系优先级高于群友 |
| 用户删除 Agent | 数据库级联删除 friendships 和 agent_relationships（现有行为，不受影响） |

## 9. 不做的范围

- 不在 `friendships` 表中增加 `source` 字段区分来源。
- 不改写 LLM Tool `start_private_chat` 中的 friendships 插入逻辑（私聊建立好友关系的行为保留）。
- 不修改 `agent_relationships` 表的 schema。
- 不提供关系描述的批量导入/导出。
