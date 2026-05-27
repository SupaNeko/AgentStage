# Session Page Title Summary 设计文档

**日期**: 2026-05-27  
**功能**: 重置会话时调用 LLM 总结聊天记录生成标题，存储到旧 chat_page 的 `name` 字段；支持前端手动修改标题；支持独立配置总结模型。

---

## 1. 背景与目标

当前 `reset_session` 会创建新的 `chat_page`（name = "续开"），旧 page 保留默认 name（"默认"/"续开"）。用户在历史模式下查看旧页面时，无法从 name 中识别内容主题。

**目标**:
- reset 时异步调用 LLM，为刚结束的旧 page 生成概括性标题
- 标题存到 `chat_pages.name`，历史页面列表直接展示
- Settings 中可配置"标题总结模型"，未配置则自动 fallback
- 前端支持手动修改任意历史 page 的标题
- 提示词独立，不和主系统提示词混在一起

---

## 2. 架构概述

```
┌──────────────┐     reset_session      ┌─────────────────────┐
│   Frontend   │ ─────────────────────→ │   Rust Backend      │
│              │                        │                     │
│ ChatView     │ ←──── page_id ─────────│ commands/session.rs │
│   (编辑UI)   │                        │                     │
│ SettingsPanel│ ←──── settings ────────│ commands/settings.rs│
│ (模型选择)   │                        │                     │
└──────────────┘                        └─────────────────────┘
                                                 │
                                                 │ spawn_generate_page_title
                                                 ↓
                                        ┌─────────────────────┐
                                        │ Scheduler           │
                                        │ run_generate_page_title
                                        │ (后台异步任务)       │
                                        └─────────────────────┘
                                                 │
                    ┌────────────────────────────┼────────────────────────────┐
                    ↓                            ↓                            ↓
            ┌──────────────┐            ┌──────────────┐            ┌──────────────┐
            │ messages     │            │ model_configs│            │ chat_pages   │
            │ (聊天记录)    │            │ (模型配置)    │            │ (更新 name)  │
            └──────────────┘            └──────────────┘            └──────────────┘
```

---

## 3. 数据层变更

### 3.1 Migration V20

```sql
-- V20: Session page title summary
ALTER TABLE app_settings ADD COLUMN summary_model_config_id TEXT;
```

- `chat_pages.name` 字段已存在，无需新增。
- `agents` 等表不受影响。

### 3.2 模型更新

**`src-tauri/src/models/settings.rs`**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {
    // ... existing fields ...
    pub summary_model_config_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsResponse {
    // ... existing fields ...
    pub summary_model_config_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateAppSettingsRequest {
    // ... existing fields ...
    pub summary_model_config_id: Option<String>,
}
```

**`src-tauri/src/models/chat_page.rs`** — 新增请求 DTO：

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateChatPageNameRequest {
    pub session_id: String,
    pub page_index: i32,
    pub name: String,
}
```

**`src/lib/types.ts`** — 前端类型：

```typescript
export interface AppSettings {
    // ... existing fields ...
    summary_model_config_id: string | null;
}

export interface UpdateChatPageNameRequest {
    session_id: string;
    page_index: number;
    name: string;
}
```

---

## 4. 后端设计

### 4.1 独立提示词模板

新增到 `src-tauri/src/llm/prompt_templates.rs`：

```rust
pub const PAGE_TITLE_SUMMARY_PROMPT: &str = r#"请根据以下聊天记录，生成一个简短的中文标题（10-20字），概括本次对话的核心主题。

聊天记录：
{session_messages}

要求：
- 只输出标题文本，不要加引号、不要解释、不要输出任何其他内容
- 标题应简洁、准确，便于后续识别
- 如果聊天内容很简短或没有实质内容，输出"闲聊"即可
"#;
```

### 4.2 后台任务：`run_generate_page_title`

位于 `src-tauri/src/scheduler/mod.rs`：

```rust
pub fn spawn_generate_page_title(&self, session_id: String, page_index: i32) {
    let scheduler = self.clone();
    tokio::spawn(async move {
        if let Err(e) = scheduler.run_generate_page_title(&session_id, page_index).await {
            crate::logger::error(&format!("[PageTitle] failed for session={} page={}: {}", session_id, page_index, e));
        }
    });
}

async fn run_generate_page_title(&self, session_id: &str, page_index: i32) -> Result<(), String> {
    // 1. 获取聊天记录（最近 50 条，按时间排序）
    // 2. 若消息为空或只有系统消息 → 直接返回 Ok（保留默认 name）
    // 3. 拼接文本: [时间] 发送者: 内容
    // 4. 获取 summary model config
    // 5. 若找不到可用模型 → 返回 Ok（保留默认 name）
    // 6. 构建 system prompt（用 PAGE_TITLE_SUMMARY_PROMPT）
    // 7. 调用 LLM (provider.chat，无 tools)
    // 8. 处理响应：trim、去除 <think> 标签内容、截断到 30 字
    // 9. 若处理后为空 → 返回 Ok（保留默认 name）
    // 10. UPDATE chat_pages SET name = ? WHERE session_id = ? AND page_index = ?
    // 11. 返回 Ok
}
```

### 4.3 模型配置解析 helper

新增到 `src-tauri/src/db/model_config.rs`：

```rust
/// 解析标题总结要使用的模型配置。
/// 优先使用 settings 中配置的 summary_model_config_id；
/// 若未配置或找不到，则 fallback 到 model_configs 表中第一个有 api_key_encrypted 的记录。
pub fn resolve_summary_model_config(conn: &Connection, settings: &AppSettings) -> Result<Option<ModelConfig>, String> {
    // 1. 若 summary_model_config_id 有值，先尝试查找
    // 2. 若找不到或无 api_key，则遍历 model_configs 表找第一个 api_key_encrypted IS NOT NULL 的
    // 3. 返回 Option<ModelConfig>
}
```

### 4.4 Tauri 命令

**`src-tauri/src/commands/chat_page.rs`**（新建文件）：

```rust
use tauri::State;
use crate::db::connection::{get_db, DbState};
use crate::models::chat_page::UpdateChatPageNameRequest;

#[tauri::command]
pub async fn update_chat_page_name(
    state: State<'_, DbState>,
    req: UpdateChatPageNameRequest,
) -> Result<(), String> {
    let conn = get_db(&state).await?;
    crate::db::chat_page::update_name(&conn, &req.session_id, req.page_index, &req.name)
        .map_err(|e| e.to_string())
}
```

**`src-tauri/src/db/chat_page.rs`** — 新增：

```rust
pub fn update_name(conn: &Connection, session_id: &str, page_index: i32, name: &str) -> Result<()> {
    conn.execute(
        "UPDATE chat_pages SET name = ?1 WHERE session_id = ?2 AND page_index = ?3",
        rusqlite::params![name, session_id, page_index],
    )?;
    Ok(())
}
```

### 4.5 reset_session 触发点

`src-tauri/src/commands/session.rs` 中 `reset_session` 命令：

```rust
#[tauri::command]
pub async fn reset_session(...) -> Result<String, String> {
    // ... 现有逻辑 ...
    if new_page_index > 0 {
        let old_page_index = new_page_index - 1;
        scheduler.spawn_session_summary(req.session_id.clone(), old_page_index);
        // 新增：
        scheduler.spawn_generate_page_title(req.session_id.clone(), old_page_index);
    }
    Ok(page_id)
}
```

---

## 5. 前端设计

### 5.1 SettingsPanel — 标题总结模型选择

在 `SettingsPanel.svelte` 的「模型」Tab（`activeTab === 'models'`）中，在 `ModelConfigPanel` 下方新增区域：

```svelte
<div class="mt-6 pt-6 border-t border-border">
    <h4 class="font-medium mb-2">标题总结模型</h4>
    <select
        value={draft.summary_model_config_id ?? ''}
        onchange={(e) => draft.summary_model_config_id = e.currentTarget.value || null}
        class="w-full px-3 py-2 bg-bg border border-border rounded-lg"
    >
        <option value="">自动选择（第一个可用模型）</option>
        {#each modelConfigStore.configs as cfg}
            <option value={cfg.id}>{cfg.name} ({cfg.model_name})</option>
        {/each}
    </select>
    <p class="text-xs text-text-secondary mt-1">
        重置会话时，用于总结聊天记录生成历史页面标题。不选则自动使用第一个配置了 API Key 的模型。
    </p>
</div>
```

保存时 `summary_model_config_id` 随 `update_settings` 一并提交。

### 5.2 ChatView — 历史页面标题编辑

在 `ChatView.svelte` 的历史页面选择器区域（`mode === 'history'`），把每个 page 项改造为可编辑：

```svelte
{#each historyStore.chatPages as page (page.page_index)}
    <div class="flex items-center gap-1 px-2 py-1 rounded hover:bg-bg">
        {#if editingPageIndex === page.page_index}
            <input
                bind:value={editingName}
                onkeydown={handleEditKey}
                onblur={saveEdit}
                class="text-sm px-1 py-0.5 bg-bg border border-primary rounded w-40"
                autofocus
            />
        {:else}
            <button
                class="text-sm truncate flex-1 text-left {historyStore.selectedPageIndex === page.page_index ? 'font-medium text-primary' : ''}"
                onclick={() => historyStore.selectPage(page.page_index)}
            >
                {page.name} #{page.page_index + 1}
            </button>
            <button
                onclick={() => startEdit(page)}
                class="p-1 text-text-secondary hover:text-text opacity-0 group-hover:opacity-100 transition-opacity"
            >
                <Pencil size={12} />
            </button>
        {/if}
    </div>
{/each}
```

交互行为：
- 点击铅笔图标 → 进入编辑模式
- Enter → 保存
- Esc → 取消
- 失去焦点 → 保存
- 保存时 trim，若为空字符串则设为 `"未命名对话"`
- 调用 `update_chat_page_name` 后更新本地 `historyStore.chatPages`

### 5.3 settingsStore 更新

`src/lib/stores/settingsStore.svelte.ts` 中 `load()` 和 `update()` 需处理新增字段：

```typescript
export interface AppSettings {
    // ...
    summary_model_config_id: string | null;
}
```

---

## 6. 数据流

```
[用户点击重置]
    │
    ▼
[Frontend] sessionStore.resetSession(sessionId)
    │
    ▼
[IPC] invoke('reset_session', { session_id })
    │
    ▼
[Rust] reset_session command
    ├─ session_repo::reset_session() → 创建新 page，返回 new_page_index
    ├─ scheduler.cancel_session()
    └─ if new_page_index > 0:
         ├─ spawn_session_summary(session_id, old_page_index)      [现有]
         └─ spawn_generate_page_title(session_id, old_page_index)  [新增]
    │
    ▼
[返回] page_id → Frontend 清空消息列表、重新加载 sessions
    │
    ▼
[后台任务] run_generate_page_title (异步，不影响前端响应)
    ├─ 查询 messages (session_id + page_index)
    ├─ 拼接 session_messages_text
    ├─ resolve_summary_model_config()
    ├─ 构建 prompt → 调用 LLM
    ├─ 清洗响应（trim、去 think、截断）
    └─ UPDATE chat_pages SET name = title
    │
    ▼
[Frontend 下次查看历史] list_chat_pages 返回已更新的 name
```

---

## 7. 错误处理

| 场景 | 行为 |
|------|------|
| 未配置 summary model，且 model_configs 为空 | 静默跳过，旧 page name 不变 |
| 未配置 summary model，fallback 遍历无可用 | 静默跳过，旧 page name 不变 |
| LLM 调用超时/网络错误/4xx/5xx | 记录 error log，旧 page name 不变 |
| LLM 返回空内容/纯空白 | 保留默认 name，不做更新 |
| 聊天记录为空或只有系统消息 | 保留默认 name，不做更新 |
| 响应包含 `<think>...</think>` | 去除 think 标签及内容，只取外层文本 |
| 前端编辑标题，API 调用失败 | Toast 提示错误，恢复原标题 |
| 前端输入为空或纯空白 | 保存时 trim，为空则设为 `"未命名对话"` |

---

## 8. 测试策略

### 8.1 后端测试

1. **Migration V20**: 验证 `summary_model_config_id` 字段存在，旧数据不受影响
2. **resolve_summary_model_config**: 
   - 配置了有效 model_config_id → 返回对应配置
   - 配置了无效 id → fallback 到第一个有 api_key 的
   - 无任何 model_configs → 返回 None
3. **run_generate_page_title**:
   - 空消息 → 不更新 name
   - 正常消息 + mock LLM → 正确更新 name
   - LLM 返回含 think 标签 → 正确清洗
4. **update_chat_page_name command**: 验证 UPDATE 生效

### 8.2 前端测试

1. **SettingsPanel**: summary model 下拉框正确加载 model configs，保存后回显正确
2. **ChatView 历史模式**: 
   - 点击铅笔进入编辑
   - Enter 保存、Esc 取消、失焦保存
   - 空输入 fallback 到 "未命名对话"

### 8.3 E2E 测试

- reset session 后，后台任务生成标题，历史页面列表刷新后显示新标题

---

## 9. 文件清单

### 新建
- `src-tauri/src/commands/chat_page.rs`

### 修改
- `src-tauri/src/db/schema.rs` — 新增 MIGRATION_V20
- `src-tauri/src/db/migration.rs` — 注册 V20
- `src-tauri/src/db/model_config.rs` — 新增 `resolve_summary_model_config`
- `src-tauri/src/db/chat_page.rs` — 新增 `update_name`
- `src-tauri/src/db/settings.rs` — 读写 `summary_model_config_id`
- `src-tauri/src/models/settings.rs` — 新增字段
- `src-tauri/src/models/chat_page.rs` — 新增 `UpdateChatPageNameRequest`
- `src-tauri/src/llm/prompt_templates.rs` — 新增 `PAGE_TITLE_SUMMARY_PROMPT`
- `src-tauri/src/scheduler/mod.rs` — 新增 `spawn_generate_page_title` / `run_generate_page_title`
- `src-tauri/src/commands/session.rs` — reset_session 追加调用
- `src-tauri/src/commands/settings.rs` — 命令注册无需改（参数透传）
- `src-tauri/src/lib.rs` — 注册 `update_chat_page_name` 命令
- `src/lib/types.ts` — 新增字段
- `src/lib/stores/settingsStore.svelte.ts` — 处理新增字段
- `src/lib/components/SettingsPanel.svelte` — 新增模型选择 UI
- `src/lib/components/ChatView.svelte` — 历史 page 标题编辑 UI

---

## 10. 注意事项

- **Tauri v2 参数命名**: Frontend `invoke` 中 `update_settings` 的 `summaryModelConfigId` 会被自动转为 `summary_model_config_id`（顶层参数）。但 `UpdateChatPageNameRequest` 是嵌套在 `req` 中的，所以 `req` 内的字段必须用 snake_case：`session_id`、`page_index`、`name`。
- **后台任务无阻塞**: `spawn_generate_page_title` 是纯后台任务，reset_session 命令在 spawn 后立即返回，前端不会等待标题生成。
- **模型配置 api_key 解密**: `resolve_summary_model_config` 返回的 `ModelConfig` 需要包含解密后的 `api_key`，复用现有的 `get_by_id` 逻辑即可（它已处理解密）。
