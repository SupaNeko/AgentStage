# 消息推送与前端渲染分离 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 移除后端 Stage 7 `snapshot_pages` 检查，由前端根据 `page_index` 和 `current_chat_page` 决定消息渲染与未读计数，实现"后端只管推送、前端决定显示"。

**Architecture:** 后端 `emit` 的 `Message` 包含完整 `page_index`；前端 `Session` 类型增加 `current_chat_page`；`App.svelte` 根据"消息是否在会话当前页"决定是否增加未读；`ChatView` 根据"消息是否在当前查看页"决定是否追加到消息列表。

**Tech Stack:** Rust + rusqlite + Tauri v2 + Svelte 5 + TypeScript

---

## 文件结构映射

| 文件 | 职责 |
|------|------|
| `src-tauri/src/models/session.rs` | `SessionResponse` 增加 `current_chat_page` |
| `src-tauri/src/db/session.rs` | `SELECT_COLUMNS` 和 `row_to_session_response` 增加 `current_chat_page` 映射 |
| `src-tauri/src/scheduler/mod.rs` | Stage 7 移除 `snapshot_pages` page_index 检查 |
| `src/lib/types.ts` | `Session` 增加 `current_chat_page`，`Message` 增加 `page_index` |
| `src/App.svelte` | `new_message` 监听器：未读计数 = "当前页有新消息" |
| `src/lib/components/ChatView.svelte` | `new_message` 监听器：只追加匹配当前查看页的消息 |
| `src/lib/components/ChatView.test.ts` | 更新 Mock 消息，增加 `page_index` |

---

### Task 1: 后端 — SessionResponse 增加 current_chat_page

**Files:**
- Modify: `src-tauri/src/models/session.rs`
- Modify: `src-tauri/src/db/session.rs`

- [ ] **Step 1: 修改 SessionResponse struct**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResponse {
    pub id: String,
    pub session_type: String,
    pub last_message_at: Option<i64>,
    pub last_message_preview: Option<String>,
    pub unread_count: i32,
    pub agent_id: Option<String>,
    pub agent_name: Option<String>,
    pub agent_avatar: Option<String>,
    pub group_name: Option<String>,
    pub group_avatar: Option<String>,
    pub mute_enabled: Option<bool>,
    pub current_chat_page: i32, // 新增
}
```

- [ ] **Step 2: 修改 SELECT_COLUMNS**

将第 5 行：
```rust
const SELECT_COLUMNS: &str = "s.id, s.session_type, s.last_message_at, s.last_message_preview, s.unread_count, ps.participant_2_id, a.name, a.avatar_path, gs.name, gs.avatar_path, gs.mute_enabled";
```

改为：
```rust
const SELECT_COLUMNS: &str = "s.id, s.session_type, s.last_message_at, s.last_message_preview, s.unread_count, ps.participant_2_id, a.name, a.avatar_path, gs.name, gs.avatar_path, gs.mute_enabled, COALESCE(ps.current_chat_page, gs.current_chat_page, 0)";
```

- [ ] **Step 3: 修改 row_to_session_response**

在第 20 行后增加：
```rust
    current_chat_page: row.get(11)?,
```

- [ ] **Step 4: 运行 `cargo check` 确认编译通过**

```bash
cd src-tauri && cargo check
```

Expected: `Finished dev profile` with zero errors.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/models/session.rs src-tauri/src/db/session.rs
git commit -m "feat: add current_chat_page to SessionResponse"
```

---

### Task 2: 后端 — scheduler Stage 7 移除 snapshot_pages 检查

**Files:**
- Modify: `src-tauri/src/scheduler/mod.rs`

- [ ] **Step 1: 替换 Stage 7 page_index 检查逻辑**

找到 Stage 7 的这段代码（约第 858-885 行）：

```rust
            let snapshot_page = snapshot_pages.get(&msg.session_id).copied().unwrap_or(0);
            crate::logger::backend("DEBUG", &format!(
                "[DEBUG trigger_agent_inner] agent_id={}, session_id={}, msg_page={} vs snapshot_page={}, session_exists={}",
                agent_id, msg.session_id, msg.page_index, snapshot_page, session_exists
            ));
            if !session_exists {
                crate::logger::backend("DEBUG", &format!(
                    "[DEBUG trigger_agent_inner] agent_id={}, session_id={}, SKIP emit/distribute (session deleted)",
                    agent_id, msg.session_id
                ));
                continue;
            }
            if msg.page_index != snapshot_page {
                crate::logger::backend("DEBUG", &format!(
                    "[DEBUG trigger_agent_inner] agent_id={}, session_id={}, SKIP emit/distribute (page mismatch: msg.page={} vs snapshot_page={})",
                    agent_id, msg.session_id, msg.page_index, snapshot_page
                ));
                continue;
            }
```

替换为：

```rust
            crate::logger::backend("DEBUG", &format!(
                "[DEBUG trigger_agent_inner] agent_id={}, session_id={}, msg_page={}, session_exists={}",
                agent_id, msg.session_id, msg.page_index, session_exists
            ));
            if !session_exists {
                crate::logger::backend("DEBUG", &format!(
                    "[DEBUG trigger_agent_inner] agent_id={}, session_id={}, SKIP emit/distribute (session deleted)",
                    agent_id, msg.session_id
                ));
                continue;
            }
            // snapshot_pages 检查已移除：后端只负责推送消息，前端负责决定是否渲染
```

- [ ] **Step 2: 运行 `cargo check` 确认编译通过**

```bash
cd src-tauri && cargo check
```

Expected: `Finished dev profile` with zero errors.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/scheduler/mod.rs
git commit -m "refactor: remove snapshot_pages check from stage 7

Backend now emits all messages regardless of page_index.
Frontend is responsible for filtering based on current viewing page."
```

---

### Task 3: 前端 — 类型定义增加 page_index 和 current_chat_page

**Files:**
- Modify: `src/lib/types.ts`

- [ ] **Step 1: Session 类型增加 current_chat_page**

```typescript
export interface Session {
    id: string;
    session_type: string;
    last_message_at: number | null;
    last_message_preview: string | null;
    unread_count: number;
    agent_id?: string;
    agent_name?: string;
    agent_avatar?: string;
    group_name?: string;
    group_avatar?: string;
    mute_enabled?: boolean;
    current_chat_page?: number; // 新增
}
```

- [ ] **Step 2: Message 类型增加 page_index**

```typescript
export interface Message {
    id: string;
    session_id: string;
    sender_type: string;
    sender_id: string;
    sender_name?: string;
    sender_avatar?: string | null;
    content: string;
    created_at: number;
    message_type: string;
    page_index?: number; // 新增
}
```

- [ ] **Step 3: Commit**

```bash
git add src/lib/types.ts
git commit -m "feat: add page_index to Message and current_chat_page to Session types"
```

---

### Task 4: 前端 — App.svelte 重写 new_message 未读逻辑

**Files:**
- Modify: `src/App.svelte`

- [ ] **Step 1: 修改 new_message 事件处理器**

将现有的 `listen('new_message', ...)` 块替换为：

```typescript
listen('new_message', (event) => {
    const msg = event.payload as { 
        session_id: string; 
        content?: string; 
        created_at?: number; 
        id?: string;
        page_index?: number;
    };
    
    sessionStore.sessions = sessionStore.sessions.map((s) => {
        if (s.id !== msg.session_id) return s;
        
        // 未读语义：当前页面有新消息
        const isCurrentPage = msg.page_index !== undefined 
                              && msg.page_index === s.current_chat_page;
        
        // 当前会话且当前页面 = 用户正在看，不增加未读
        const isCurrentlyViewing = msg.session_id === sessionStore.selectedSessionId
                                    && appState.currentView === 'chat'
                                    && isCurrentPage;
        
        return {
            ...s,
            unread_count: (isCurrentPage && !isCurrentlyViewing) 
                ? s.unread_count + 1 
                : s.unread_count,
            last_message_preview: msg.content || s.last_message_preview,
            last_message_at: msg.created_at || Date.now(),
        };
    });
}).then((fn) => unlistenFns.push(fn));
```

- [ ] **Step 2: Commit**

```bash
git add src/App.svelte
git commit -m "feat: unread count = current page only, backend emits all messages"
```

---

### Task 5: 前端 — ChatView.svelte 增加 page_index 过滤

**Files:**
- Modify: `src/lib/components/ChatView.svelte`

- [ ] **Step 1: 修改 ChatView 的 new_message 监听器**

找到 `ChatView.svelte` 中 `listen('new_message', ...)` 的代码块（约在 `onMount` 中），将其替换为：

```typescript
listen('new_message', (event) => {
    const payload = event.payload as Message;
    if (payload.session_id !== sessionId) return;
    
    // 根据当前查看的页面决定是否追加消息
    const currentPage = mode === 'chat'
        ? (sessionStore.sessions.find(s => s.id === sessionId)?.current_chat_page ?? 0)
        : (historyStore.selectedPageIndex ?? 0);
    
    if (payload.page_index !== undefined && payload.page_index === currentPage) {
        messageStore.addMessage(payload);
        // 原有的滚动逻辑保持不变
        if (shouldAutoScroll) {
            requestAnimationFrame(() => scrollToBottom());
        }
    }
    // page_index 不匹配的消息：不追加到当前视图
    //（App.svelte 已负责更新会话列表预览和未读）
});
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/components/ChatView.svelte
git commit -m "feat: ChatView only appends messages matching current viewing page"
```

---

### Task 6: 前端测试更新

**Files:**
- Modify: `src/lib/components/ChatView.test.ts`

- [ ] **Step 1: Mock 消息增加 page_index**

搜索 `ChatView.test.ts` 中所有创建 Mock `Message` 对象的地方，补充 `page_index: 0`（或对应测试需要的值）。

例如，将：
```typescript
const msg = { id: 'm1', session_id: 's1', sender_type: 'agent', sender_id: 'a1', content: 'Hello', created_at: Date.now(), message_type: 'text' };
```

改为：
```typescript
const msg = { id: 'm1', session_id: 's1', sender_type: 'agent', sender_id: 'a1', content: 'Hello', created_at: Date.now(), message_type: 'text', page_index: 0 };
```

- [ ] **Step 2: 运行前端测试**

```bash
pnpm test  # 或 npx vitest run
```

Expected: 所有测试通过。

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/ChatView.test.ts
git commit -m "test: add page_index to mock messages"
```

---

### Task 7: 最终验证

- [ ] **Step 1: 后端编译**

```bash
cd src-tauri && cargo check
```

Expected: `Finished dev profile` with zero errors.

- [ ] **Step 2: 后端测试编译**

```bash
cd src-tauri && cargo check --tests
```

Expected: `Finished dev profile` with zero errors.

- [ ] **Step 3: 提交最终变更**

```bash
git log --oneline -5
```

---

## 自审清单

- [x] **Spec coverage**: 
  - 后端移除 `snapshot_pages` → Task 2
  - `SessionResponse` 增加 `current_chat_page` → Task 1
  - `Message` / `Session` 类型增加字段 → Task 3
  - `App.svelte` 未读逻辑重写 → Task 4
  - `ChatView` page_index 过滤 → Task 5
  - 测试更新 → Task 6

- [x] **Placeholder scan**: 无 TBD/TODO，所有步骤包含具体代码

- [x] **Type一致性**: 
  - `current_chat_page` 在 Rust `SessionResponse`、前端 `Session`、SQL SELECT 中名称一致
  - `page_index` 在 Rust `Message`、前端 `Message`、emit payload 中名称一致
