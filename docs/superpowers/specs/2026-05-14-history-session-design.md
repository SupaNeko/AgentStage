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
| History 模式与 Chat 模式共享调度链路导致竞态空白 | 见第 9 节：已决定彻底分离两套链路 |
| 已删除群聊消息泄漏到 Prompt | 见第 9.2 节：已在 SQL 中增加 `s.is_deleted = 0` 过滤 |

---

## 9. 需求补充与 Bug 修复（2026-05-15）

### 9.0 背景

当前 History 会话功能虽然已实现 page 级隔离，但在实际测试中发现以下问题：
1. **Bug 1**：在历史群聊中发送消息后，前端消息列表短暂空白，角色回复后才恢复。
2. **Bug 2**：已删除的群聊消息仍然出现在 Agent 的 Prompt 中。
3. **需求补充**：需要更清晰地分离当前会话和历史会话的调度逻辑；需要在前端增加常驻提示语；需要在后端日志中记录完整的 LLM prompt。

本节对原有设计进行**补充和修正**，替代原有设计中存在问题的部分。

---

### 9.1 历史会话与当前会话的调度逻辑彻底分离（替代原 §3.1 / §3.2 / §4.4）

#### 问题分析

当前设计让 History 模式复用了 `send_user_message` → `scheduler.on_new_message` → `distribute_message` → `try_trigger_agent` → LLM → `new_message` 事件广播 这一整套 Chat 模式的异步调度链路。`page_index` 仅作为可选参数在关键环节透传。

这导致 History 模式下：
- `send_user_message` 触发 Scheduler，Scheduler 可能触发多个 Agent。
- 前端 `handleSend` 中 `loadMessages` 的全量刷新与 `new_message` 事件的增量更新产生竞态。
- 当 `page_index` 参数在某一环节丢失或匹配失败时，前端消息列表被错误地重置或查询了错误的 page，表现为"短暂空白"。
- 修复难度大：需要让 Scheduler、ToolExecutor、`PromptAssembler` 的每一个环节都 100% page-aware，且前端事件过滤逻辑必须零失误。

#### 决策：两套完全独立的调度逻辑

| 维度 | Chat 模式（当前会话） | History 模式（历史会话） |
|------|----------------------|-------------------------|
| **后端命令** | `send_user_message` | **新增 `send_history_message`** |
| **调度器** | 使用 `Scheduler`（全局状态机、unread queue、自动触发链） | **不使用 Scheduler**，直接调用 LLM |
| **Prompt 组装** | `PromptAssembler`（注入所有 session 的上下文 + pending messages） | **简化版 Assembler**（仅注入当前 session + 当前 page 的消息） |
| **消息存储** | 写入 DB，`current_chat_page` 可能变化 | 写入 DB，**固定写入指定的 `page_index`** |
| **前端更新** | 监听 `new_message` 事件 | **直接等待命令返回值**，不监听事件 |
| **并发触发** | 支持多 Agent 并发触发、跨 session 触发 | **单 session 单轮对话**，不支持跨 session 触发 |
| **适用场景** | 日常聊天、群聊、Agent 间自然触发 | 回顾历史、在旧 page 中补充对话 |

#### 9.1.1 新增后端命令：`send_history_message`

```rust
// src-tauri/src/models/message.rs
#[derive(Debug, Clone, Deserialize)]
pub struct SendHistoryMessageRequest {
    pub session_id: String,
    pub content: String,
    pub page_index: i32,  // 必填，History 模式必须指定 page
}
```

```rust
// src-tauri/src/commands/message.rs
#[tauri::command]
pub async fn send_history_message(
    state: State<'_, DbState>,
    req: SendHistoryMessageRequest,
) -> Result<Vec<Message>, String> {
    let conn = get_db(&state).await?;
    
    // 1. 插入用户消息到指定 page
    let user_msg = message_repo::insert_message(
        &conn, &req.session_id, "user", "user", &req.content, "text", Some(req.page_index),
    ).map_err(|e| e.to_string())?;
    
    // 2. 更新会话最后消息预览
    let preview = crate::scheduler::truncate_preview(&req.content, 100);
    let _ = session_repo::update_session_last_message(&conn, &req.session_id, &preview);
    
    // 3. 查询该 session + 该 page 的所有历史消息作为上下文
    let history_msgs = message_repo::get_messages_by_session(&conn, &req.session_id, req.page_index, 1000, 0)
        .map_err(|e| e.to_string())?;
    
    // 4. 确定该 session 中需要回复的 Agent（私聊 = 对方 Agent；群聊 = 所有群成员 Agent）
    let target_agents = resolve_history_target_agents(&conn, &req.session_id)?;
    
    // 5. 为每个目标 Agent 组装简化 Prompt（仅包含当前 session + page 的消息）
    let mut replies: Vec<Message> = vec![user_msg];
    for agent_id in target_agents {
        let prompt = HistoryPromptAssembler::assemble(&conn, &agent_id, &req.session_id, req.page_index, &history_msgs)?;
        
        // 6. 调用 LLM
        let llm_response = call_llm(&agent_id, &prompt).await?;
        
        // 7. 插入 Agent 回复到同一 page
        let agent_msg = message_repo::insert_message(
            &conn, &req.session_id, "agent", &agent_id, &llm_response, "text", Some(req.page_index),
        ).map_err(|e| e.to_string())?;
        
        replies.push(agent_msg);
    }
    
    Ok(replies)
}
```

**关键区别**：
- **不调用 `scheduler.on_new_message`**：没有 unread queue、没有自动触发链、没有 `is_triggering` 状态机。
- **不 emit `new_message` 事件**：命令直接返回 `Vec<Message>`（包含用户消息和所有 Agent 回复），前端通过 `await` 获取完整结果。
- **Prompt 仅包含当前 session + page**：见 9.1.2。

#### 9.1.2 新增：`HistoryPromptAssembler`

这是一个**简化版**的 Prompt 组装器，专门用于 History 模式。

```rust
// src-tauri/src/llm/history_prompt.rs
pub struct HistoryPromptAssembler;

impl HistoryPromptAssembler {
    pub fn assemble(
        conn: &Connection,
        agent_id: &str,
        session_id: &str,
        page_index: i32,
        history_messages: &[Message],
    ) -> Result<String, String> {
        // Layer 1: Agent 自我设定
        let layer1 = get_agent_system_prompt(conn, agent_id)?;
        
        // Layer 2: 当前 session + page 的消息历史（即 history_messages 参数）
        // 不需要再查 DB，直接传入
        let layer2 = format_messages_as_context(history_messages, agent_id);
        
        // Layer 3: 本次对话的引导语
        let layer3 = "请基于以上对话上下文继续回复。注意：你正在回顾或补充一段历史对话。";
        
        let full_prompt = format!("{}\n\n{}\n\n{}", layer1, layer2, layer3);
        
        // 新增需求：记录完整 prompt 到日志
        log::info!(
            "[HistoryPromptAssembler] Full prompt for agent {} (session={}, page={}):\n---PROMPT START---\n{}\n---PROMPT END---",
            agent_id, session_id, page_index, full_prompt
        );
        
        Ok(full_prompt)
    }
}
```

**与 `PromptAssembler` 的核心差异**：

| 特性 | `PromptAssembler` (Chat 模式) | `HistoryPromptAssembler` (History 模式) |
|------|------------------------------|----------------------------------------|
| 查询范围 | 所有关联 session 的 `current_chat_page` + pending messages | **仅当前 session + 指定 page** |
| Layer 2 (其他 session 消息) | 包含 | **不包含** |
| Layer 4 (pending messages) | 包含 | **不包含**（History 模式没有 pending queue） |
| SQL 复杂度 | 多表 JOIN + CASE WHEN | 无复杂 JOIN，直接传入已查询的消息列表 |
| 调用时机 | Scheduler 触发 | `send_history_message` 直接调用 |
| 日志 | 分段记录 | **记录完整 prompt**（见 9.3） |

#### 9.1.3 前端 History 模式改造

**`ChatView.svelte`（History 模式）发送逻辑**：

```typescript
async function handleSend() {
    const content = inputText.trim();
    const sessionId = historyStore.selectedSessionId;
    const pageIdx = historyStore.selectedPageIndex;
    
    if (!content || !sessionId || pageIdx == null) return;
    
    // 乐观更新：立即显示用户消息
    const tempId = `temp-${Date.now()}`;
    const optimisticMsg: Message = {
        id: tempId,
        session_id: sessionId,
        sender_id: 'user',
        sender_type: 'user',
        sender_name: '用户',
        content,
        msg_type: 'text',
        created_at: Date.now(),
        page_index: pageIdx,
    };
    messageStore.addMessage(optimisticMsg);
    inputText = '';
    
    try {
        // 调用新的 History 专用命令
        const replies = await invoke<Message[]>('send_history_message', {
            req: { session_id: sessionId, content, page_index: pageIdx },
        });
        
        // 移除乐观更新的临时消息
        messageStore.messages = messageStore.messages.filter(m => m.id !== tempId);
        
        // 将后端返回的完整消息列表（用户消息 + Agent 回复）设置为当前列表
        messageStore.messages = replies;
        
    } catch (err) {
        logger.error('Failed to send history message:', err);
        // 保留乐观更新的消息，显示错误提示
    }
}
```

**`ChatView.svelte`（History 模式）事件监听**：

History 模式**不再监听 `new_message` 事件**。所有消息更新都通过 `send_history_message` 的返回值同步获取。

```typescript
onMount(() => {
    // Chat 模式才需要监听 new_message
    if (mode === 'chat') {
        listen('new_message', ...).then(fn => unlistenFns.push(fn));
        listen('agent_typing', ...).then(fn => unlistenFns.push(fn));
        listen('agent_completed', ...).then(fn => unlistenFns.push(fn));
        listen('agent_error', ...).then(fn => unlistenFns.push(fn));
    }
    // History 模式：无需监听任何事件
});
```

**常驻提示语（前端 UI）**：

在 History 模式的 `ChatView` 标题栏下方、消息列表上方，增加一个常驻的提示横幅：

```svelte
{#if mode === 'history'}
    <div class="px-4 py-2 bg-amber-50 border-b border-amber-200 text-amber-800 text-sm flex items-center gap-2">
        <Clock size={16} />
        <span>当前处于<strong>历史会话</strong>模式。此处的对话仅基于当前会话的历史记录，不会影响其他会话，也不会触发跨会话的 Agent 互动。</span>
    </div>
{/if}
```

CSS 建议（Tailwind v4）：
```css
/* 使用 Tailwind v4 @theme token 或 inline */
bg-amber-50, border-amber-200, text-amber-800
```

#### 9.1.4 数据流对比图

**Chat 模式（不变）**：
```
User → send_user_message → insert_message → scheduler.on_new_message
                                                    ↓
                                          distribute_message → unread queues
                                                    ↓
                                          try_trigger_agent → LLM
                                                    ↓
                                          ToolExecutor → insert_message
                                                    ↓
                                          emit('new_message') → 所有监听者
                                                    ↓
                                          Frontend (ChatView) 刷新列表
```

**History 模式（新）**：
```
User → send_history_message → insert_message (指定 page)
                                  ↓
                          查询该 page 历史消息
                                  ↓
                          HistoryPromptAssembler (仅当前 session)
                                  ↓
                          LLM 调用（同步等待）
                                  ↓
                          insert_message (Agent 回复，同一 page)
                                  ↓
                          返回 Vec<Message> 给前端
                                  ↓
                          Frontend (ChatView) 直接渲染返回结果
```

---

### 9.2 Bug 修复：已删除群聊消息泄漏到 Prompt（替代原 §3.1 SQL）

#### 问题

`PromptAssembler` 的 Layer 2（其他 session 的最近 10 条消息）和 Layer 3/Layer 4 的 SQL 查询中，JOIN `sessions` 表时未过滤 `is_deleted = 0`。导致已解散（soft-deleted）的群聊消息仍然被注入到 Agent 的 Prompt 中。

#### 修复

在所有涉及 `sessions` 表 JOIN 的 `PromptAssembler` SQL 中，增加 `s.is_deleted = 0` 条件。

**Layer 2 修复后**：
```sql
SELECT m.*, s.session_type
FROM messages m
JOIN sessions s ON m.session_id = s.session_id
WHERE m.session_id != ?1 
  AND m.sender_id != ?2
  AND m.is_deleted = 0
  AND s.is_deleted = 0   -- <-- 新增
ORDER BY m.created_at DESC
LIMIT 10
```

**Layer 3 修复后**（Agent 自己的 session 消息）：
```sql
SELECT m.*, s.session_type
FROM messages m
JOIN sessions s ON m.session_id = s.session_id
WHERE m.session_id = ?1
  AND m.sender_id != ?2
  AND m.is_deleted = 0
  AND s.is_deleted = 0   -- <-- 新增
ORDER BY m.created_at DESC
LIMIT 50
```

**Layer 4 修复后**（所有关联 session 消息）：
```sql
-- 子查询 sp 已经通过 private_sessions / group_sessions 关联，
-- 但最外层 JOIN sessions 时同样需要过滤
SELECT m.*, s.session_type
FROM messages m
JOIN sessions s ON m.session_id = s.session_id
JOIN (...) sp ON m.session_id = sp.session_id AND ...
WHERE m.is_deleted = 0
  AND s.is_deleted = 0   -- <-- 新增
ORDER BY m.created_at ASC
```

> **注意**：`HistoryPromptAssembler` 由于直接传入 `history_messages` 参数，不涉及跨 session 查询，因此**不受此 bug 影响**。这也是分离两套逻辑的优势之一。

---

### 9.3 新增需求：后端日志记录完整 LLM Prompt

#### 需求描述

在 `PromptAssembler::assemble` 和 `HistoryPromptAssembler::assemble` 中，将最终发送给 LLM 的完整 prompt 字符串记录到后端日志，以便调试和分析。

#### 实现规范

```rust
// 在 PromptAssembler::assemble 末尾（拼接完 final_prompt 后）
log::info!(
    "[PromptAssembler] Full prompt for agent {} | trigger_session={:?} | trigger_page={:?} | prompt_length={}\n---PROMPT START---\n{}\n---PROMPT END---",
    agent_id,
    trigger_session_id,
    trigger_page_index,
    final_prompt.len(),
    final_prompt
);
```

```rust
// 在 HistoryPromptAssembler::assemble 末尾
log::info!(
    "[HistoryPromptAssembler] Full prompt for agent {} | session={} | page={} | prompt_length={}\n---PROMPT START---\n{}\n---PROMPT END---",
    agent_id,
    session_id,
    page_index,
    full_prompt.len(),
    full_prompt
);
```

**规范**：
- 日志级别：`INFO`（确保默认可见，无需开启 DEBUG/TRACE）。
- 包含元数据：`agent_id`、`session_id`、`page_index`、prompt 字符长度。
- 使用 `---PROMPT START---` / `---PROMPT END---` 包裹，便于脚本提取。
- **安全**：prompt 内容只包含公开的消息文本和系统设定，**不包含 API Key**（API Key 在 LLM client 层使用，不会出现在 prompt 中）。

---

### 9.4 更新后的实现顺序

基于本节补充，实现顺序调整为：

#### Phase 1: 后端修复与新增
1. **修复** `PromptAssembler` 所有 SQL：增加 `s.is_deleted = 0` 过滤（Bug 2）。
2. **新增** `HistoryPromptAssembler`（`src-tauri/src/llm/history_prompt.rs`）。
3. **新增** `send_history_message` Tauri 命令（`src-tauri/src/commands/message.rs`）。
4. **新增** `SendHistoryMessageRequest` DTO（`src-tauri/src/models/message.rs`）。
5. **新增** `log::info!` 完整 prompt 日志（`PromptAssembler` + `HistoryPromptAssembler`）。
6. **注册** 新命令到 `src-tauri/src/lib.rs`。
7. **测试**：后端 Rust 单元测试（`send_history_message`、`HistoryPromptAssembler`、`is_deleted` 过滤）。

#### Phase 2: 前端改造
8. **修改** `ChatView.svelte`：
   - History 模式下 `handleSend` 改用 `send_history_message`。
   - History 模式下移除 `new_message` / `agent_typing` / `agent_completed` 事件监听。
   - 新增常驻提示语横幅（历史会话模式说明）。
9. **修改** `messageStore.svelte.ts`：History 模式下不再依赖事件驱动更新。
10. **样式**：确保提示语横幅使用 Tailwind v4 的 amber 色板。

#### Phase 3: 测试
11. **Vitest**：History 模式 `handleSend` 调用正确命令、不监听事件。
12. **Playwright E2E**：
    - 历史群聊发送消息后列表不空白。
    - 常驻提示语可见。
    - 已删除群聊消息不出现在新会话的 Agent Prompt 中（需要通过 mock 验证 prompt 内容）。

---

### 9.5 状态更新

| 项目 | 原状态 | 新状态 |
|------|--------|--------|
| 方案 A（Page 级隔离） | 已实现 | **部分重构**（History 链路独立） |
| `send_user_message` + `page_index` | 方案设计 | **History 模式弃用**，仅 Chat 模式保留 |
| `PromptAssembler` CASE WHEN | 已实现 | **Chat 模式保留**，History 模式使用新 Assembler |
| `new_message` 事件（History） | 方案设计 | **History 模式弃用** |
| `sessions.is_deleted` 过滤 | 未涉及 | **新增需求** |
| 完整 Prompt 日志 | 未涉及 | **新增需求** |
| 前端常驻提示语 | 未涉及 | **新增需求** |

---

## 10. 风险与回退（更新）

| 风险 | 缓解措施 |
|------|----------|
| `HistoryPromptAssembler` 过于简化，导致 Agent 回复质量下降 | 明确告知用户"历史模式"仅用于回顾/补充，不保证与当前会话完全一致的角色互动体验。如需要完整上下文，引导用户回到 Chat 模式 |
| `send_history_message` 同步调用 LLM，群聊中多 Agent 时延迟较高 | 可考虑并行调用多个 Agent 的 LLM（`join_all`）。如延迟不可接受，再评估是否恢复 Scheduler 方案 |
| 删除 `new_message` 监听后，History 模式无法实时感知其他用户/Agent 在当前 page 的操作 | 这是设计意图。History 模式是"单用户回顾/补充"场景，不支持多用户并发编辑同一历史 page。如需要，未来可添加轮询机制 |
| `s.is_deleted = 0` 过滤影响 PromptAssembler 性能 | `sessions.session_id` 是主键，`is_deleted` 过滤不会阻止索引使用。如大规模数据下性能下降，可在 `sessions(is_deleted)` 上加索引 |

---

*文档版本：v2（2026-05-15）*
*原始版本：v1（2026-05-14）*
