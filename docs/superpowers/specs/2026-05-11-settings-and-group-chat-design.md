# 全局配置面板 + 群聊创建设计文档

*日期：2026-05-11*  
*版本：v1.0*  
*关联功能：SET-04（全局触发间隔配置）、SES-02（创建群聊会话）*

---

## 1. 设计目标

1. 在现有设置弹窗中实现**全局最小触发间隔**的配置，采用批量保存模式（保存按钮 + "已保存" Toast）。
2. 实现**群聊创建**完整前端流程（弹窗选择角色 → 创建 → 进入群聊）。
3. 在群聊会话窗口**右侧显示成员列表**（头像 + 名称），私聊不显示。

---

## 2. 整体架构

### 2.1 新增后端模块

| 文件 | 职责 |
|------|------|
| `src-tauri/src/commands/settings.rs` | `get_settings()` / `update_settings(req)` |
| `src-tauri/src/db/settings.rs` | 新增 `update_settings(conn, req)` |
| `src-tauri/src/commands/session.rs` | 新增 `create_group_session(req)` / `get_group_members(session_id)` |
| `src-tauri/src/db/session.rs` | 新增 `create_group_session()` / `get_group_members()` |
| `src-tauri/src/models/session.rs` | 新增 `CreateGroupSessionRequest` / `GroupMemberResponse` |

### 2.2 新增/修改前端模块

| 文件 | 职责 |
|------|------|
| `src/lib/stores/settingsStore.svelte.ts` | **新建**：应用级 settings store，启动时加载 |
| `src/lib/components/SettingsPanel.svelte` | **新建**：设置表单，替换 `App.svelte` 中的占位内容 |
| `src/lib/components/CreateGroupModal.svelte` | **新建**：群聊创建弹窗（群名 + 角色多选） |
| `src/lib/components/SessionList.svelte` | **修改**：Header 增加"新建群聊"按钮 |
| `src/lib/components/ChatView.svelte` | **修改**：增加右侧成员列表（仅群聊） |
| `src/App.svelte` | **修改**：集成 `SettingsPanel`，启动时 `loadSettings` |
| `src-tauri/src/lib.rs` | **修改**：注册 `get_settings`、`update_settings`、`create_group_session`、`get_group_members` |

### 2.3 数据流

```
[App 启动] → invoke('get_settings') → settingsStore.settings
[打开设置] → SettingsPanel 读取 settingsStore → 本地 draft 编辑
[点击保存] → invoke('update_settings', { global_min_trigger_interval }) 
          → 成功后更新 settingsStore → Toast "已保存"
[点击新建群聊] → CreateGroupModal → 选择角色 + 输入群名
[点击创建] → invoke('create_group_session', { name, agent_ids })
          → sessionStore.addSession → 自动进入群聊
[进入群聊] → ChatView 检测 session_type='group' 
          → invoke('get_group_members') → 右侧渲染成员列表
```

---

## 3. 全局配置面板详细设计

### 3.1 后端 Commands

**`src-tauri/src/commands/settings.rs`**（新建）：

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

### 3.2 后端 Repository

**`src-tauri/src/db/settings.rs`** — 新增 `update_settings`：

采用"读取当前值 → 用请求中的非空值覆盖 → 全字段 UPDATE"策略：

```rust
pub fn update_settings(conn: &Connection, req: &UpdateAppSettingsRequest) -> Result<()> {
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

### 3.3 前端 Store

**`src/lib/stores/settingsStore.svelte.ts`**（新建）：

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
            // 其余字段不传，后端用当前值保留
        };
        const updated = await invoke<AppSettings>('update_settings', { req });
        this.settings = updated;
        return updated;
    }
}

export const settingsStore = new SettingsStore();
```

### 3.4 前端 UI

**`src/lib/components/SettingsPanel.svelte`**（新建，替换 `App.svelte` 设置弹窗内容）：

- **单字段表单**：全局最小触发间隔
  - `<input type="number" min="0" />`，单位"秒"
  - 说明文字：`0 = 不限制，>0 = 等待 N 秒`
- **底部保存按钮**：点击后 `settingsStore.update(draft)`
- **保存成功**：`toastStore.show('已保存', 'success', 2000)`
- **保存失败**：`toastStore.show('保存失败：' + err, 'error')`

面板结构沿用现有居中模态弹窗（`max-w-lg`），替换原有占位内容。

---

## 4. 群聊创建详细设计

### 4.1 后端 DTO 扩展

**`src-tauri/src/models/session.rs`** 新增：

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

### 4.2 后端 Repository

**`src-tauri/src/db/session.rs`** 新增：

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

    let session_id = Uuid::new_v4().to_string();
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
) -> Result<Vec<GroupMemberResponse>> {
    let mut stmt = conn.prepare(
        "SELECT gm.participant_type, gm.participant_id,
                CASE WHEN gm.participant_type = 'user' THEN '用户' ELSE COALESCE(a.name, '未知角色') END as name, a.avatar_path
         FROM group_members gm
         LEFT JOIN agents a ON gm.participant_type = 'agent' AND gm.participant_id = a.id
         WHERE gm.session_id = ?1 AND gm.is_active = 1
         ORDER BY gm.participant_type DESC, name ASC"
    )?;
    let rows = stmt.query_map([session_id], |row| {
        Ok(GroupMemberResponse {
            participant_type: row.get(0)?,
            participant_id: row.get(1)?,
            name: row.get(2)?,
            avatar_path: row.get(3)?,
        })
    })?;
    rows.collect()
}
```

**设计说明**：
- 创建时验证至少 2 个角色
- 默认 `mute_enabled = 0`（不禁言）
- 用户自动作为 `participant_type='user'` 的成员加入
- `get_group_members` 中 `participant_type DESC` 保证用户排在最前面

### 4.3 后端 Commands

**`src-tauri/src/commands/session.rs`** 新增：

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

并在 `lib.rs` 的 `invoke_handler` 中注册这四个新命令。

### 4.4 前端群聊创建弹窗

**`src/lib/components/CreateGroupModal.svelte`**（新建）：

- 居中模态弹窗，`max-w-md`
- **Header**："新建群聊" + 关闭按钮
- **Body**：
  - 群名输入框（必填）
  - 角色选择区域：组件内调用 `invoke('list_agents')` 获取角色列表，每个角色有复选框 + 头像 + 名称
  - 说明：至少选择 2 个角色
- **Footer**："取消" + "创建"按钮
  - 创建按钮在 `agent_ids < 2` 或 `name` 为空时禁用
  - 创建中显示"创建中..."
- **创建成功**：`sessionStore.addSession(session)` → `sessionStore.selectSession(session.id)` → `appState.switchView('chat')` → Toast "群聊创建成功"

### 4.5 会话列表入口

**`src/lib/components/SessionList.svelte`** — 在 Header 右侧增加"新建群聊"按钮（`Plus` 图标），点击打开 `CreateGroupModal`。

### 4.6 群聊成员列表

**`src/lib/components/ChatView.svelte`** 布局修改：

- 外层从 `flex flex-col h-full` 改为 `flex h-full`
- 中间聊天区域包裹在 `flex-1 min-w-0 flex flex-col` 中
- **右侧条件渲染**：`{#if selectedSession?.session_type === 'group'}`
  - 固定宽度 `w-56`（224px）
  - Header："成员 (N)"
  - 列表：用户排在最前，每个成员显示头像 + 名称
  - 数据通过 `invoke('get_group_members', { sessionId })` 获取

---

## 5. 错误处理

| 场景 | 处理方式 |
|------|----------|
| 群聊角色数 < 2 | Rust 返回错误，前端 Toast "请选择至少 2 个角色" |
| 群聊名称为空 | 前端校验，创建按钮禁用 |
| 触发间隔输入负数 | 输入框 `min="0"` 拦截 |
| 查询已删除群聊成员 | 返回空数组（`is_active = 1` 过滤） |
| 创建群聊时角色不存在 | 成员列表显示"未知角色"（`COALESCE` 兜底） |
| 快速连续点击保存 | `saving = true` 时禁用按钮 |

---

## 6. 测试策略

### 6.1 Rust 单元测试

- `test_update_settings_preserve_untouched_fields`：更新触发间隔后，确认 theme/font_size 等未展示字段保持不变
- `test_create_group_session_min_2_agents`：传入 1 个角色时返回错误
- `test_create_group_session_and_get_members`：创建群聊后查询成员，验证用户 + 角色都在，且用户排在第一位

### 6.2 前端验证

- 设置面板：输入 0、1、30、9999 均可保存；空字符串被数字输入框阻止
- 群聊弹窗：未选角色/未输入名称时创建按钮禁用
- 成员列表：进入群聊后右侧正确渲染；进入私聊后右侧不显示
- 创建流程：创建成功后自动进入群聊，会话列表出现新群聊

---

## 7. 待后续迭代的功能（不在本次范围）

- 消息上限配置（SET-05）：后续放到角色配置页和群聊设置中
- Enter 发送切换：固定为 Enter 发送、Shift+Enter 换行
- 主题/字体/语言设置（SET-01）：待 UI 具备 dark 样式后再实现
- 群聊禁言开关 UI（SES-07）：本次只做数据层的 `mute_enabled = 0`，禁言 Toggle 在后续群聊管理功能中补充
- 会话置顶/归档/搜索（SES-04/06）：V1.1 排期

---

*文档版本：v1.0*  
*编写日期：2026-05-11*
