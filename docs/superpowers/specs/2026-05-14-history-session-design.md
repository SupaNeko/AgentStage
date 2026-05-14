# 历史会话功能设计文档

**日期**: 2026-05-14  
**方案**: 方案 A（Page 级隔离）  
**状态**: 待实现

---

## 1. 背景与目标

当前 `chat_pages` 表和 `page_index` 字段已存在于数据库中，`reset_session` 已能创建新的 chat page。但前端缺乏浏览和切换历史 page 的能力：

- `history` 视图仅为占位符（"历史会话功能即将推出..."）
- `ChatView` 始终只显示 `current_chat_page` 的消息
- 用户无法回顾或继续之前的对话轮次

**目标**：
1. `history` 视图按群聊/私聊分类展示所有会话
2. 每个会话在列表中只占用一行，点击进入详情页
3. 详情页标题栏正中提供下拉框，可翻阅该会话的每一次历史记录（chat page）
4. 以该次记录的最后更新时间作为下拉框选项
5. 可在旧 page 中继续发送消息，与 Chat 视图的最新 page 互不冲突

---

## 2. 数据模型与 API 设计

### 2.1 新增 Rust DTO

```rust
// src-tauri/src/models/chat_page.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatPage {
    pub id: String,
    pub session_id: String,
    pub page_index: i32,
    pub name: String,
    pub is_active: bool,
    pub message_count: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListChatPagesRequest {
    pub session_id: String,
}
```

### 2.2 修改现有 DTO

```rust
// src-tauri/src/models/message.rs
#[derive(Debug, Clone, Deserialize)]
pub struct GetSessionMessagesRequest {
    pub session_id: String,
    pub limit: i32,
    pub offset: i32,
    pub page_index: Option<i32>,  // None = 使用 current_chat_page
}

#[derive(Debug, Clone, Deserialize)]
pub struct SendMessageRequest {
    pub session_id: String,
    pub content: String,
    pub page_index: Option<i32>,  // None = 使用 current_chat_page
}
```

### 2.3 新增 Tauri 命令

| 命令 | 输入 | 输出 | 说明 |
|------|------|------|------|
| `list_chat_pages` | `ListChatPagesRequest` | `Vec<ChatPage>` | 返回某 session 的所有历史 page，按 `page_index` DESC。`updated_at` 和 `message_count` 通过 `messages` 表动态聚合计算。 |

### 2.4 修改 Tauri 命令

| 命令 | 修改内容 |
|------|----------|
| `get_session_messages` | 当 `req.page_index` 为 `Some(p)` 时，直接查询 page `p`；为 `None` 时保持现有行为（查 `current_chat_page`）。 |
| `send_user_message` | 当 `req.page_index` 为 `Some(p)` 时，调用 `insert_message` 时传入 `Some(p)`；为 `None` 时保持现有行为。 |

### 2.5 前端类型扩展

```typescript
// src/lib/types.ts
export interface ChatPage {
    id: string;
    session_id: string;
    page_index: number;
    name: string;
    is_active: boolean;
    message_count: number;
    created_at: number;
    updated_at: number;
}
```

### 2.6 关键决策：`updated_at` 动态聚合

现有 `chat_pages` 表的 `updated_at` 和 `message_count` 在创建后从未维护。为避免数据不一致，`list_chat_pages` 查询时从 `messages` 表动态聚合：

```sql
SELECT 
    cp.id, cp.session_id, cp.page_index, cp.name, cp.is_active, cp.created_at,
    COALESCE(msg_stats.msg_count, 0) as message_count,
    COALESCE(msg_stats.last_msg_at, cp.created_at) as updated_at
FROM chat_pages cp
LEFT JOIN (
    SELECT session_id, page_index, COUNT(*) as msg_count, MAX(created_at) as last_msg_at
    FROM messages
    WHERE is_deleted = 0
    GROUP BY session_id, page_index
) msg_stats ON cp.session_id = msg_stats.session_id AND cp.page_index = msg_stats.page_index
WHERE cp.session_id = ?1
ORDER BY cp.page_index DESC
```

---

## 3. 后端核心链路修改

### 3.1 PromptAssembler 修改

当前 `PromptAssembler::assemble` 对所有 session 都使用 `current_chat_page`。需要改为：**对触发来源的 session 使用指定的 `page_index`，对其他 session 仍用 `current_chat_page`**。

**函数签名变更**：

```rust
pub fn assemble(
    conn: &Connection,
    agent_id: &str,
    trigger_session_id: Option<&str>,   // 新增
    trigger_page_index: Option<i32>,    // 新增
    _pending_messages: &[Message],
) -> Result<String, String>
```

**SQL 变更**（Layer 4 的 JOIN 子查询）：

```sql
JOIN (
    SELECT session_id, COALESCE(current_chat_page, 0) as page 
    FROM private_sessions WHERE agent_id = ?1
    UNION
    SELECT gs.session_id, COALESCE(gs.current_chat_page, 0) as page 
    FROM group_sessions gs
    JOIN group_members gm ON gs.session_id = gm.session_id
    WHERE gm.participant_id = ?1 AND gm.participant_type = 'agent'
) sp ON m.session_id = sp.session_id 
    AND m.page_index = CASE 
        WHEN ?2 IS NOT NULL AND m.session_id = ?2 THEN ?3 
        ELSE sp.page 
    END
```

`?1` = agent_id, `?2` = trigger_session_id, `?3` = trigger_page_index。

### 3.2 Scheduler 链路修改

`Message` 结构体已包含 `page_index: i32`。需要将该字段沿触发链路传递：

1. `on_new_message(msg: Message)` → `distribute_message(msg)`：直接使用 `msg.page_index`
2. `distribute_message` → `try_trigger_agent`：无需改动（try_trigger_agent 只传 agent_id）
3. `trigger_agent(agent_id)` → `trigger_agent_inner(agent_id, msg)`：`msg` 已包含 `page_index`
4. `trigger_agent_inner` Stage 3 调用 `PromptAssembler::assemble` 时传入 `msg.session_id` 和 `msg.page_index`
5. `trigger_agent_inner` Stage 7 的 `ToolExecutor` 已有 `session_pages` 绑定机制，`ToolExecutor.execute` 调用 `insert_message` 时已传入绑定好的 `page_index`（Agent 回复自然进入同一 page）

**修改点汇总**：

| 文件 | 函数 | 修改 |
|------|------|------|
| `src-tauri/src/llm/prompt.rs` | `PromptAssembler::assemble` | 增加 `trigger_session_id` 和 `trigger_page_index` 参数；SQL JOIN 条件使用 `CASE WHEN` |
| `src-tauri/src/scheduler/mod.rs` | `trigger_agent_inner` Stage 3 | 调用 `assemble` 时传入 `msg.session_id` 和 `msg.page_index` |
| `src-tauri/src/commands/message.rs` | `get_session_messages` | 支持 `req.page_index` |
| `src-tauri/src/commands/message.rs` | `send_user_message` | 支持 `req.page_index` |
| `src-tauri/src/commands/session.rs` | 新增 `list_chat_pages` | 新增命令 |

### 3.3 `get_session_messages` 修改逻辑

```rust
pub async fn get_session_messages(
    state: State<'_, DbState>,
    req: GetSessionMessagesRequest,
) -> Result<Vec<Message>, String> {
    let conn = get_db(&state).await?;
    
    let page_index = match req.page_index {
        Some(p) => p,
        None => {
            conn.query_row(
                "SELECT COALESCE(current_chat_page, 0) FROM private_sessions WHERE session_id = ?1
                 UNION ALL
                 SELECT COALESCE(current_chat_page, 0) FROM group_sessions WHERE session_id = ?1
                 LIMIT 1",
                [&req.session_id],
                |row| row.get(0),
            ).unwrap_or(0)
        }
    };
    
    let messages = message_repo::get_messages_by_session(&conn, &req.session_id, page_index, req.limit, req.offset)
        .map_err(|e| e.to_string())?;
    Ok(messages)
}
```

### 3.4 `send_user_message` 修改逻辑

```rust
pub async fn send_user_message(
    state: State<'_, DbState>,
    scheduler: State<'_, Scheduler>,
    req: SendMessageRequest,
) -> Result<Message, String> {
    let conn = get_db(&state).await?;
    
    let page_index = match req.page_index {
        Some(p) => p,
        None => {
            conn.query_row(
                "SELECT COALESCE(current_chat_page, 0) FROM private_sessions WHERE session_id = ?1
                 UNION ALL
                 SELECT COALESCE(current_chat_page, 0) FROM group_sessions WHERE session_id = ?1
                 LIMIT 1",
                [&req.session_id],
                |row| row.get(0),
            ).unwrap_or(0)
        }
    };
    
    let message = message_repo::insert_message(
        &conn, &req.session_id, "user", "user", &req.content, "text", Some(page_index),
    ).map_err(|e| e.to_string())?;
    
    // 更新会话最后消息预览
    let preview = crate::scheduler::truncate_preview(&req.content, 100);
    let _ = session_repo::update_session_last_message(&conn, &req.session_id, &preview);
    
    drop(conn);
    
    let _ = scheduler.on_new_message(&req.session_id, &message).await;
    Ok(message)
}
```

**注意**：`update_session_last_message` 更新的是 `sessions` 表的预览，不区分 page。这是合理的，因为 session 级别的预览应该反映整个会话的最新活动。

---

## 4. 前端组件与状态管理

### 4.1 `App.svelte` 路由调整

将 `history` 视图的占位符替换为实际组件：

```svelte
<!-- Middle Panel (w-72) -->
<div class="w-72 shrink-0 bg-surface border-r border-border">
    {#if appState.currentView === 'agents'}
        <AgentList />
    {:else if appState.currentView === 'chat'}
        <SessionList />
    {:else}
        <HistorySessionList />
    {/if}
</div>

<!-- Main Content Area -->
<main class="flex-1 min-w-0 bg-bg">
    {#if appState.currentView === 'agents'}
        <AgentDetail />
    {:else if appState.currentView === 'chat'}
        <ChatView mode="chat" />
    {:else}
        <ChatView mode="history" />
    {/if}
</main>
```

### 4.2 新建 `historyStore.svelte.ts`

```typescript
import { invoke } from '@tauri-apps/api/core';
import { logger } from '$lib/logger';
import type { Session, ChatPage } from '$lib/types';

export class HistoryStore {
    selectedSessionId = $state<string | null>(null);
    selectedPageIndex = $state<number | null>(null);
    chatPages = $state<ChatPage[]>([]);
    sessions = $state<Session[]>([]);
    loadingPages = $state(false);

    async loadSessions() {
        try {
            const all = await invoke<Session[]>('list_sessions');
            this.sessions = all;
            logger.debug('[DEBUG historyStore.loadSessions]', { count: all.length });
        } catch (err) {
            logger.error('Failed to load history sessions:', err);
        }
    }

    async loadChatPages(sessionId: string) {
        this.loadingPages = true;
        try {
            const pages = await invoke<ChatPage[]>('list_chat_pages', {
                req: { session_id: sessionId },
            });
            this.chatPages = pages;
            // 默认选中最新 page（page_index 最大）
            if (pages.length > 0) {
                this.selectedPageIndex = pages[0].page_index;
            } else {
                this.selectedPageIndex = 0;
            }
            logger.debug('[DEBUG historyStore.loadChatPages]', { sessionId, count: pages.length });
        } catch (err) {
            logger.error('Failed to load chat pages:', err);
            this.chatPages = [];
            this.selectedPageIndex = 0;
        } finally {
            this.loadingPages = false;
        }
    }

    selectSession(sessionId: string) {
        this.selectedSessionId = sessionId;
        this.loadChatPages(sessionId);
    }

    selectPage(pageIndex: number) {
        this.selectedPageIndex = pageIndex;
    }

    get groupedSessions() {
        const privateSessions = this.sessions.filter(s => s.session_type === 'private');
        const groupSessions = this.sessions.filter(s => s.session_type === 'group');
        return { private: privateSessions, group: groupSessions };
    }
}

export const historyStore = new HistoryStore();
```

### 4.3 新建 `HistorySessionList.svelte`

**布局**：
- 顶部 header："历史会话"
- 按群聊/私聊分组，每组可折叠
- 每个 session 一行：头像、名称、历史轮次数 badge（来自 `chatPages.length`，在选中后加载）
- 未选中 session 时，主区域显示占位提示

**组件结构**：

```svelte
<script lang="ts">
    import { onMount } from 'svelte';
    import { historyStore } from '$lib/stores/historyStore.svelte';
    import { MessageSquare, ChevronDown, ChevronRight, Clock } from 'lucide-svelte';
    import { formatTime } from '$lib/utils';

    let expandedPrivate = $state(true);
    let expandedGroup = $state(true);

    onMount(() => {
        historyStore.loadSessions();
    });

    function handleSessionClick(sessionId: string) {
        historyStore.selectSession(sessionId);
    }
</script>

<div class="flex flex-col h-full w-full bg-surface border-r border-border">
    <header class="flex items-center justify-between p-4 border-b border-border">
        <h2 class="text-base font-semibold">历史会话</h2>
    </header>

    <div class="flex-1 overflow-y-auto">
        {#if historyStore.sessions.length === 0}
            <div class="flex flex-col items-center justify-center h-full text-text-secondary p-4">
                <MessageSquare size={40} class="mb-3 opacity-50" />
                <p class="text-sm">还没有会话</p>
            </div>
        {:else}
            <!-- 私聊分组 -->
            <div class="border-b border-border">
                <button onclick={() => expandedPrivate = !expandedPrivate} class="w-full flex items-center justify-between px-4 py-2.5 hover:bg-bg text-sm font-medium">
                    <span>私聊</span>
                    {#if expandedPrivate}<ChevronDown size={16} />{:else}<ChevronRight size={16} />{/if}
                </button>
                {#if expandedPrivate}
                    {#each historyStore.groupedSessions.private as session}
                        <button onclick={() => handleSessionClick(session.id)} class="w-full flex items-center gap-3 px-4 py-3 text-left hover:bg-bg {historyStore.selectedSessionId === session.id ? 'bg-primary/5 border-l-2 border-l-primary' : ''}">
                            <!-- 头像 -->
                            <div class="w-10 h-10 rounded-full bg-gray-300 flex items-center justify-center text-white shrink-0 overflow-hidden">
                                {#if session.agent_avatar}
                                    <img src={session.agent_avatar} alt={session.agent_name} class="w-full h-full object-cover" />
                                {:else}
                                    <MessageSquare size={20} />
                                {/if}
                            </div>
                            <div class="min-w-0 flex-1">
                                <div class="flex items-center justify-between">
                                    <h3 class="font-medium text-sm truncate">{session.agent_name || '未命名'}</h3>
                                </div>
                                <p class="text-xs text-text-secondary truncate">{session.last_message_preview || '暂无消息'}</p>
                            </div>
                        </button>
                    {/each}
                {/if}
            </div>

            <!-- 群聊分组 -->
            <div>
                <button onclick={() => expandedGroup = !expandedGroup} class="w-full flex items-center justify-between px-4 py-2.5 hover:bg-bg text-sm font-medium">
                    <span>群聊</span>
                    {#if expandedGroup}<ChevronDown size={16} />{:else}<ChevronRight size={16} />{/if}
                </button>
                {#if expandedGroup}
                    {#each historyStore.groupedSessions.group as session}
                        <button onclick={() => handleSessionClick(session.id)} class="w-full flex items-center gap-3 px-4 py-3 text-left hover:bg-bg {historyStore.selectedSessionId === session.id ? 'bg-primary/5 border-l-2 border-l-primary' : ''}">
                            <div class="w-10 h-10 rounded-full bg-gray-300 flex items-center justify-center text-white shrink-0 overflow-hidden">
                                {#if session.group_avatar}
                                    <img src={session.group_avatar} alt={session.group_name} class="w-full h-full object-cover" />
                                {:else}
                                    <MessageSquare size={20} />
                                {/if}
                            </div>
                            <div class="min-w-0 flex-1">
                                <div class="flex items-center justify-between">
                                    <h3 class="font-medium text-sm truncate">{session.group_name || '未命名群聊'}</h3>
                                </div>
                                <p class="text-xs text-text-secondary truncate">{session.last_message_preview || '暂无消息'}</p>
                            </div>
                        </button>
                    {/each}
                {/if}
            </div>
        {/if}
    </div>
</div>
```

### 4.4 增强 `ChatView.svelte`

为 `ChatView` 增加 `mode` prop，支持 `'chat' | 'history'` 两种模式。

**Props 定义**：

```typescript
interface Props {
    mode?: 'chat' | 'history';
}
let { mode = 'chat' }: Props = $props();
```

**模式差异处理**：

| 逻辑 | Chat 模式 (`mode='chat'`) | History 模式 (`mode='history'`) |
|------|---------------------------|--------------------------------|
| 加载消息 | `loadMessages(sessionId)`，不传 `page_index`（后端查 `current_chat_page`） | `loadMessages(sessionId, pageIndex)`，传入 `historyStore.selectedPageIndex` |
| 标题栏 | 无下拉框 | 有下拉框（显示 `historyStore.chatPages`） |
| 发送消息 | 不传 `page_index` | 传入 `historyStore.selectedPageIndex` |
| `new_message` 过滤 | 不过滤（所有消息都刷新列表，`loadMessages` 自动查 current page） | 只处理 `msg.page_index === historyStore.selectedPageIndex` 的消息 |
| session 来源 | `sessionStore.selectedSessionId` | `historyStore.selectedSessionId` |

**标题栏修改**：

```svelte
<header class="flex items-center justify-between px-6 py-4 border-b border-border bg-surface shrink-0 relative">
    {#if selectedSession}
        <div class="flex items-center gap-3">
            <div class="w-10 h-10 rounded-full bg-gray-300 flex items-center justify-center text-white shrink-0 overflow-hidden">
                {#if selectedSession.agent_avatar || selectedSession.group_avatar}
                    <img src={selectedSession.agent_avatar || selectedSession.group_avatar} alt="头像" class="w-full h-full object-cover" />
                {:else}
                    <MessageSquare size={20} />
                {/if}
            </div>
            <div>
                <h2 class="text-lg font-semibold">
                    {selectedSession.agent_name || selectedSession.group_name || '未命名会话'}
                </h2>
            </div>
        </div>

        <!-- Center: page selector (history mode only) -->
        {#if mode === 'history' && historyStore.chatPages.length > 0}
            <div class="absolute left-1/2 -translate-x-1/2">
                <select
                    value={historyStore.selectedPageIndex ?? 0}
                    onchange={(e) => {
                        const idx = Number((e.target as HTMLSelectElement).value);
                        historyStore.selectPage(idx);
                        if (historyStore.selectedSessionId) {
                            messageStore.loadMessages(historyStore.selectedSessionId, idx);
                        }
                    }}
                    class="px-3 py-1.5 bg-bg border border-border rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-primary/20 min-w-[180px]"
                >
                    {#each historyStore.chatPages as page (page.page_index)}
                        <option value={page.page_index}>
                            {page.name} #{page.page_index + 1} — {formatTime(page.updated_at)}
                        </option>
                    {/each}
                </select>
            </div>
        {/if}

        <button onclick={() => settingsOpen = !settingsOpen} class="p-2 hover:bg-bg rounded-lg text-text-secondary transition-colors" title="会话配置">
            <Settings size={20} />
        </button>
    {:else}
        <h2 class="text-lg font-semibold text-text-secondary">
            {mode === 'history' ? '选择一个会话查看历史' : '选择一个会话开始聊天'}
        </h2>
    {/if}
</header>
```

**消息加载修改**：

```typescript
$effect(() => {
    const id = mode === 'chat' ? sessionStore.selectedSessionId : historyStore.selectedSessionId;
    const pageIdx = mode === 'history' ? historyStore.selectedPageIndex : null;
    
    if (id) {
        if (pageIdx != null) {
            messageStore.loadMessages(id, pageIdx);
        } else {
            messageStore.loadMessages(id);
        }
        // ... 其余不变
    }
});
```

**发送消息修改**：

```typescript
async function handleSend() {
    const content = inputText.trim();
    const sessionId = mode === 'chat' ? sessionStore.selectedSessionId : historyStore.selectedSessionId;
    const pageIdx = mode === 'history' ? historyStore.selectedPageIndex : null;
    
    if (!content || !sessionId) return;
    
    // ... optimistic update ...
    
    const payload: Record<string, unknown> = { session_id: sessionId, content };
    if (pageIdx != null) {
        payload.page_index = pageIdx;
    }
    
    await invoke('send_user_message', { req: payload });
    
    // 刷新消息
    if (pageIdx != null) {
        await messageStore.loadMessages(sessionId, pageIdx);
    } else {
        await messageStore.loadMessages(sessionId);
    }
}
```

**`new_message` 事件过滤**（`onMount` 中的 listen）：

```typescript
listen('new_message', (event) => {
    const msg = event.payload as { session_id: string; page_index?: number; content?: string; id?: string };
    
    if (mode === 'chat') {
        // Chat 模式：只要 session 匹配就刷新（loadMessages 自动查 current_chat_page）
        if (msg.session_id === sessionStore.selectedSessionId) {
            messageStore.loadMessages(msg.session_id);
        }
        // 更新 session 列表预览
        sessionStore.sessions = sessionStore.sessions.map(s => ...);
    } else {
        // History 模式：必须 session 和 page_index 都匹配
        if (msg.session_id === historyStore.selectedSessionId && msg.page_index === historyStore.selectedPageIndex) {
            const exists = messageStore.messages.some(m => m.id === msg.id);
            if (!exists) {
                messageStore.addMessage(msg as unknown as Message);
            }
        }
    }
}).then(fn => unlistenFns.push(fn));
```

### 4.5 增强 `messageStore.svelte.ts`

增加带 `page_index` 的消息加载：

```typescript
export class MessageStore {
    messages = $state<Message[]>([]);
    currentSessionId = $state<string | null>(null);
    currentPageIndex = $state<number | null>(null);

    async loadMessages(sessionId: string, pageIndex?: number) {
        logger.debug('[DEBUG messageStore.loadMessages]', { sessionId, pageIndex });
        try {
            const req: Record<string, unknown> = { session_id: sessionId, limit: 50, offset: 0 };
            if (pageIndex !== undefined) {
                req.page_index = pageIndex;
            }
            const result = await invoke<Message[]>('get_session_messages', { req });
            this.messages = result.reverse();
            this.currentSessionId = sessionId;
            this.currentPageIndex = pageIndex ?? null;
        } catch (err) {
            this.messages = [];
            this.currentSessionId = sessionId;
            this.currentPageIndex = pageIndex ?? null;
        }
    }
    
    // ... addMessage, setSessionId 不变
}
```

---

## 5. 交互流程与边界情况

### 5.1 完整用户流程

```
用户点击 "历史会话" (LeftNav)
    ↓
中间面板加载 HistorySessionList（按私聊/群聊分组）
    ↓
用户点击某个 session
    ↓
historyStore.selectSession(id) → 调用 list_chat_pages
    ↓
下拉框默认选中最新 page，主区域 ChatView 加载该 page 消息
    ↓
用户点击下拉框，选择旧 page
    ↓
messageStore.loadMessages(id, oldPageIndex) → 加载旧 page 消息
    ↓
用户在旧 page 中输入并发送
    ↓
send_user_message 传入 page_index → 消息写入旧 page
    ↓
scheduler.on_new_message 使用该 page 触发 agent
    ↓
agent 回复写入同一 page
    ↓
new_message 事件广播 → History 视图匹配 page_index 后显示
    ↓
Chat 视图也收到事件 → loadMessages 查 current_chat_page → 不包含旧 page 消息 → 无变化
```

### 5.2 边界情况

| 场景 | 处理 |
|------|------|
| Session 只有 1 个 page | 下拉框仍然显示，但只有 1 个选项（`默认 #1`） |
| 下拉框选择时消息正在加载 | `select` 元素不禁用（切换应即时响应），消息区域显示 loading skeleton 或 spinner |
| 旧 page 中发送消息时 agent_message_count 已达上限 | 显示相同的黄色 message limit warning bar，点击"重置限制"后解除冻结 |
| `new_message` 事件不含 `page_index` | 后端 `emit` 必须包含 `page_index`。若缺失，History 模式不过滤（兜底行为） |
| History 视图中 session 被解散 | `HistorySessionList` 调用 `list_sessions`（已过滤 `is_deleted=0`），解散后自动从列表消失 |
| Chat 视图和 History 视图同时打开同一 session 的不同 page | Chat 视图始终显示 current_chat_page 的消息；History 视图显示选中的旧 page。互不干扰。 |
| 用户在 History 视图中发送消息后，切回 Chat 视图 | Chat 视图显示的是 current_chat_page 的消息，旧 page 的消息不会出现在这里 |

---

## 6. 测试策略

### 6.1 后端单元测试（Rust）

| 测试 | 内容 |
|------|------|
| `test_list_chat_pages_returns_correct_order` | 创建 session → reset 两次 → `list_chat_pages` 返回 3 条，按 `page_index` DESC |
| `test_list_chat_pages_aggregates_message_stats` | 在 page 0 插入 2 条消息，page 1 插入 3 条 → 返回的 `message_count` 和 `updated_at` 正确 |
| `test_get_session_messages_with_page_index` | 传入 `page_index=1`，只返回 page 1 的消息 |
| `test_send_user_message_with_page_index` | 传入 `page_index=0`，消息写入 page 0，`current_chat_page` 不变 |
| `test_prompt_assemble_uses_trigger_page_for_trigger_session` | Agent 参与 session A (page 0) 和 session B (page 1)。在 session A page 0 触发时，PromptAssembler 对 session A 使用 page 0，对 session B 使用 current_chat_page (page 1) |
| `test_prompt_assemble_defaults_to_current_chat_page_when_no_trigger` | 不传 trigger_session 时，所有 session 都用 current_chat_page（向后兼容） |

### 6.2 前端单元测试（Vitest）

| 测试 | 内容 |
|------|------|
| `HistorySessionList renders private and group sections` | 传入私聊和群聊数据，渲染两组折叠面板 |
| `HistorySessionList expands and collapses` | 点击折叠按钮，对应组展开/收起 |
| `ChatView history mode shows page selector` | `mode='history'` 且 `chatPages` 非空时，标题栏显示 `select` 元素 |
| `ChatView history mode hides page selector when empty` | `chatPages` 为空时，不显示下拉框 |
| `messageStore.loadMessages passes page_index` | 调用 `loadMessages(id, 2)` 时，`invoke` 参数包含 `page_index: 2` |

### 6.3 E2E 测试（Playwright）

| 测试 | 内容 |
|------|------|
| `history view shows grouped sessions` | 点击 History 图标，中间面板显示"私聊"和"群聊"分组 |
| `history detail loads chat pages dropdown` | 点击 session，主区域标题栏出现下拉框，选项按时间倒序 |
| `switching page updates message list` | 选择下拉框中的旧 page，消息列表内容变化 |
| `sending message in old page stays in old page` | 在旧 page 发送消息，消息出现在列表中；切换回最新 page，该消息不出现 |

---

## 7. 实现顺序建议

1. **后端**：新增 `list_chat_pages` 命令 + `ChatPage` DTO
2. **后端**：修改 `GetSessionMessagesRequest` / `get_session_messages` 支持 `page_index`
3. **后端**：修改 `SendMessageRequest` / `send_user_message` 支持 `page_index`
4. **后端**：修改 `PromptAssembler::assemble` 支持 trigger page
5. **后端**：修改 `scheduler/mod.rs` 传递 `page_index` 到 `assemble`
6. **前端**：新建 `historyStore.svelte.ts`
7. **前端**：新建 `HistorySessionList.svelte`
8. **前端**：增强 `ChatView.svelte` 支持 `mode='history'`
9. **前端**：增强 `messageStore.svelte.ts` 支持 `page_index`
10. **前端**：修改 `App.svelte` 路由
11. **测试**：后端单元测试 + 前端 Vitest + Playwright E2E

---

## 8. 风险与回退

| 风险 | 缓解措施 |
|------|----------|
| `PromptAssembler` SQL 修改引入性能问题 | `CASE WHEN` 只在 JOIN 条件中使用，对已有索引影响小。如性能下降，可将 `trigger_session_id` 的 page 预查出并 UNION 到子查询中 |
| `new_message` 事件缺少 `page_index` 导致 History 模式漏消息 | 后端 emit 时务必包含 `page_index`；前端做兜底：若缺失，不过滤直接显示 |
| 旧 page 继续对话导致 agent context 混乱 | 这是设计意图。`PromptAssembler` 对该 session 使用该 page 的上下文，对其他 session 用 current page，行为一致且可控 |
