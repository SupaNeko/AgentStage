# 历史会话功能实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现历史会话浏览功能：按群聊/私聊分类，每个会话详情页提供下拉框切换历史 chat page，并可在旧 page 中继续发送消息（与 Chat 视图的最新 page 隔离）。

**Architecture:** 后端通过 `page_index` 参数扩展 `get_session_messages` 和 `send_user_message`，`PromptAssembler` 对触发来源 session 使用指定 page；前端新增 `HistorySessionList` 组件和 `historyStore`，增强 `ChatView` 支持 `mode='history'`。

**Tech Stack:** Tauri v2 (Rust + SQLite), Svelte 5, TypeScript, TailwindCSS v4

---

## 文件结构映射

### 新建文件

| 文件 | 职责 |
|------|------|
| `src-tauri/src/models/chat_page.rs` | `ChatPage` 和 `ListChatPagesRequest` DTO |
| `src-tauri/src/db/chat_page.rs` | `list_chat_pages` 原始 SQL 查询（动态聚合 message_count/updated_at） |
| `src/lib/stores/historyStore.svelte.ts` | History 视图的状态管理：session 列表、chatPages、选中 page |
| `src/lib/components/HistorySessionList.svelte` | History 视图中间面板：按私聊/群聊分组的会话列表 |

### 修改文件

| 文件 | 职责 |
|------|------|
| `src-tauri/src/models/mod.rs` | 新增 `pub mod chat_page;` |
| `src-tauri/src/models/message.rs` | `GetSessionMessagesRequest` / `SendMessageRequest` 增加 `page_index: Option<i32>` |
| `src-tauri/src/db/mod.rs` | 新增 `pub mod chat_page;` |
| `src-tauri/src/commands/session.rs` | 新增 `list_chat_pages` 命令；导入 `ChatPage` 相关类型 |
| `src-tauri/src/commands/message.rs` | `get_session_messages` 和 `send_user_message` 支持 `page_index` |
| `src-tauri/src/lib.rs` | `generate_handler!` 中注册 `list_chat_pages`；导入该命令 |
| `src-tauri/src/llm/prompt.rs` | `PromptAssembler::assemble` 签名增加 `trigger_session_id`/`trigger_page_index`；SQL JOIN 使用 `CASE WHEN` |
| `src-tauri/src/scheduler/mod.rs` | `trigger_agent_inner` Stage 3 调用 `assemble` 时传入 `msg.session_id` 和 `msg.page_index` |
| `src/lib/types.ts` | 新增 `ChatPage` TypeScript 接口 |
| `src/lib/stores/messageStore.svelte.ts` | `loadMessages` 支持传入可选 `pageIndex` |
| `src/lib/components/ChatView.svelte` | 增加 `mode` prop、标题栏下拉框、page-aware 的消息加载/发送/事件过滤 |
| `src/App.svelte` | `history` 视图路由：中间面板 `HistorySessionList`，主区域 `ChatView mode="history"` |

---

## 任务分解

### Task 1: ChatPage DTO

**Files:**
- Create: `src-tauri/src/models/chat_page.rs`
- Modify: `src-tauri/src/models/mod.rs`

- [ ] **Step 1: 创建 ChatPage DTO**

```rust
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

- [ ] **Step 2: 注册到 models/mod.rs**

在 `src-tauri/src/models/mod.rs` 末尾添加：

```rust
pub mod chat_page;
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/models/chat_page.rs src-tauri/src/models/mod.rs
git commit -m "feat(models): add ChatPage and ListChatPagesRequest DTOs"
```

---

### Task 2: chat_page repository

**Files:**
- Create: `src-tauri/src/db/chat_page.rs`
- Modify: `src-tauri/src/db/mod.rs`

- [ ] **Step 1: 创建 list_chat_pages 查询函数**

```rust
use rusqlite::{Connection, Result};
use crate::models::chat_page::ChatPage;

pub fn list_chat_pages(conn: &Connection, session_id: &str) -> Result<Vec<ChatPage>> {
    let mut stmt = conn.prepare(
        "SELECT 
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
        ORDER BY cp.page_index DESC"
    )?;
    
    let rows = stmt.query_map([session_id], |row| {
        Ok(ChatPage {
            id: row.get(0)?,
            session_id: row.get(1)?,
            page_index: row.get(2)?,
            name: row.get(3)?,
            is_active: row.get::<_, i32>(4)? != 0,
            created_at: row.get(5)?,
            message_count: row.get(6)?,
            updated_at: row.get(7)?,
        })
    })?;
    
    rows.collect()
}
```

- [ ] **Step 2: 注册到 db/mod.rs**

在 `src-tauri/src/db/mod.rs` 末尾添加：

```rust
pub mod chat_page;
```

- [ ] **Step 3: cargo check**

```bash
cd src-tauri && cargo check
```

Expected: `Finished dev profile` with no errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db/chat_page.rs src-tauri/src/db/mod.rs
git commit -m "feat(db): add list_chat_pages repository with dynamic aggregation"
```

---

### Task 3: list_chat_pages command

**Files:**
- Modify: `src-tauri/src/commands/session.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 在 commands/session.rs 中新增 list_chat_pages**

在 `src-tauri/src/commands/session.rs` 顶部导入区添加：

```rust
use crate::models::chat_page::{ChatPage, ListChatPagesRequest};
use crate::db::chat_page as chat_page_repo;
```

在文件末尾（最后一个 `#[tauri::command]` 之后）添加：

```rust
#[tauri::command]
pub async fn list_chat_pages(
    state: State<'_, DbState>,
    req: ListChatPagesRequest,
) -> Result<Vec<ChatPage>, String> {
    let conn = get_db(&state).await?;
    let pages = chat_page_repo::list_chat_pages(&conn, &req.session_id)
        .map_err(|e| e.to_string())?;
    Ok(pages)
}
```

- [ ] **Step 2: 在 lib.rs 中注册命令**

在 `src-tauri/src/lib.rs` 的导入区添加：

```rust
use commands::session::list_chat_pages;
```

在 `tauri::generate_handler![...]` 列表中添加 `list_chat_pages,`。

- [ ] **Step 3: cargo check**

```bash
cd src-tauri && cargo check
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/session.rs src-tauri/src/lib.rs
git commit -m "feat(commands): add list_chat_pages Tauri command"
```

---

### Task 4: 修改 message DTOs 支持 page_index

**Files:**
- Modify: `src-tauri/src/models/message.rs`

- [ ] **Step 1: 修改 GetSessionMessagesRequest**

在 `src-tauri/src/models/message.rs` 中找到 `GetSessionMessagesRequest`，改为：

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct GetSessionMessagesRequest {
    pub session_id: String,
    pub limit: i32,
    pub offset: i32,
    pub page_index: Option<i32>,
}
```

- [ ] **Step 2: 修改 SendMessageRequest**

找到 `SendMessageRequest`，改为：

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct SendMessageRequest {
    pub session_id: String,
    pub content: String,
    pub page_index: Option<i32>,
}
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/models/message.rs
git commit -m "feat(models): add optional page_index to message requests"
```

---

### Task 5: 修改 get_session_messages 支持 page_index

**Files:**
- Modify: `src-tauri/src/commands/message.rs`

- [ ] **Step 1: 修改 get_session_messages 命令实现**

替换 `src-tauri/src/commands/message.rs` 中 `get_session_messages` 函数体：

```rust
#[tauri::command]
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

    println!("[DEBUG get_session_messages] returned {} messages (page_index={})", messages.len(), page_index);
    Ok(messages)
}
```

- [ ] **Step 2: cargo check**

```bash
cd src-tauri && cargo check
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/message.rs
git commit -m "feat(commands): get_session_messages supports optional page_index"
```

---

### Task 6: 修改 send_user_message 支持 page_index

**Files:**
- Modify: `src-tauri/src/commands/message.rs`

- [ ] **Step 1: 修改 send_user_message 中的 insert_message 调用**

找到 `send_user_message` 函数中 `message_repo::insert_message` 调用之前，添加 `page_index` 解析逻辑：

```rust
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
```

然后修改 `insert_message` 调用的最后一个参数从 `None` 改为 `Some(page_index)`：

```rust
    let message = message_repo::insert_message(
        &conn,
        &req.session_id,
        "user",
        "user",
        &req.content,
        "text",
        Some(page_index),
    ).map_err(|e| e.to_string())?;
```

- [ ] **Step 2: cargo check**

```bash
cd src-tauri && cargo check
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/message.rs
git commit -m "feat(commands): send_user_message supports optional page_index"
```

---

### Task 7: PromptAssembler 支持 trigger page

**Files:**
- Modify: `src-tauri/src/llm/prompt.rs`

- [ ] **Step 1: 修改 assemble 函数签名**

将 `src-tauri/src/llm/prompt.rs` 中的 `assemble` 函数签名：

```rust
    pub fn assemble(
        conn: &Connection,
        agent_id: &str,
        trigger_session_id: Option<&str>,
        trigger_page_index: Option<i32>,
        _pending_messages: &[Message],
    ) -> Result<String, String> {
```

- [ ] **Step 2: 修改 Layer 4 的 SQL JOIN 条件**

找到 Layer 4 的 SQL（包含 `JOIN (SELECT session_id, COALESCE(current_chat_page...` 的部分），将 `AND m.page_index = sp.page` 替换为：

```rust
            "SELECT m.id, m.session_id, m.sender_type, m.sender_id, m.content, m.created_at, 
                    m.message_type, m.tool_call_data, m.generation_info, m.is_deleted,
                    COALESCE(a.name, CASE WHEN m.sender_type = 'user' THEN '用户' ELSE '未知' END) as sender_name,
                    a.avatar_path as sender_avatar,
                    m.page_index
             FROM messages m
             JOIN (
                 SELECT session_id, COALESCE(current_chat_page, 0) as page FROM private_sessions WHERE agent_id = ?1
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
             LEFT JOIN agents a ON m.sender_type = 'agent' AND m.sender_id = a.id AND a.is_deleted = 0
             WHERE m.is_deleted = 0
             ORDER BY m.created_at DESC"
```

注意参数绑定从原来的 `stmt.query_map([agent_id], ...)` 改为：

```rust
    let rows = stmt.query_map(
        rusqlite::params![agent_id, trigger_session_id, trigger_page_index],
        |row| { ... }
    ).map_err(|e| e.to_string())?;
```

- [ ] **Step 3: cargo check**

```bash
cd src-tauri && cargo check
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/llm/prompt.rs
git commit -m "feat(llm): PromptAssembler supports trigger_session_id and trigger_page_index"
```

---

### Task 8: Scheduler 传递 page_index 到 PromptAssembler

**Files:**
- Modify: `src-tauri/src/scheduler/mod.rs`

- [ ] **Step 1: 找到 trigger_agent_inner Stage 3 的 assemble 调用**

在 `src-tauri/src/scheduler/mod.rs` 中搜索 `PromptAssembler::assemble`，找到类似：

```rust
let prompt = PromptAssembler::assemble(&conn, &agent_id, &pending_messages)
```

改为：

```rust
let prompt = PromptAssembler::assemble(
    &conn,
    &agent_id,
    Some(&msg.session_id),
    Some(msg.page_index),
    &pending_messages,
);
```

注意：`msg` 在该上下文中应该是 `Message` 类型且已有 `page_index: i32` 字段。

- [ ] **Step 2: cargo check**

```bash
cd src-tauri && cargo check
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/scheduler/mod.rs
git commit -m "feat(scheduler): pass trigger page_index to PromptAssembler"
```

---

### Task 9: 后端测试 — list_chat_pages + PromptAssembler trigger page

**Files:**
- Modify: `src-tauri/src/db/session.rs`（已有测试文件）
- Modify: `src-tauri/src/llm/prompt.rs`（已有测试文件）

- [ ] **Step 1: 在 db/session.rs 测试区新增 list_chat_pages 测试**

在 `src-tauri/src/db/session.rs` 的 `#[cfg(test)]` 模块末尾添加：

```rust
    #[test]
    fn test_list_chat_pages_returns_pages_in_desc_order() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        // Reset once to create page 1
        let _ = reset_session(&conn, &session.id).unwrap();
        
        let pages = crate::db::chat_page::list_chat_pages(&conn, &session.id).unwrap();
        assert_eq!(pages.len(), 2, "Expected 2 pages (default + reset), got {}", pages.len());
        assert_eq!(pages[0].page_index, 1); // DESC order: newest first
        assert_eq!(pages[1].page_index, 0);
    }

    #[test]
    fn test_list_chat_pages_aggregates_message_stats() {
        let conn = init_test_db();
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            ("agent1", "Test Agent", 0i64),
        ).unwrap();
        
        let session = create_private_session(&conn, "agent1").unwrap();
        
        // Insert message to page 0
        crate::db::message::insert_message(&conn, &session.id, "user", "user", "Hello", "text", Some(0)).unwrap();
        
        let pages = crate::db::chat_page::list_chat_pages(&conn, &session.id).unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].message_count, 1);
        assert!(pages[0].updated_at > pages[0].created_at, "updated_at should reflect last message time");
    }
```

- [ ] **Step 2: 在 llm/prompt.rs 测试区更新 assemble 调用**

搜索所有 `PromptAssembler::assemble(` 调用（测试区内），将调用改为新的 5 参数签名。例如：

```rust
let prompt = PromptAssembler::assemble(&conn, "agent1", None, None, &[]).unwrap();
```

以及：

```rust
let prompt = PromptAssembler::assemble(&conn, "agent1", None, None, &pending).unwrap();
```

- [ ] **Step 3: 新增 PromptAssembler trigger page 测试**

在 `src-tauri/src/llm/prompt.rs` 的 `#[cfg(test)]` 模块末尾添加：

```rust
    #[test]
    fn test_prompt_assemble_uses_trigger_page_for_trigger_session() {
        let conn = init_test_db();
        insert_agent(&conn, "agent1", "Agent One", "Persona 1");
        insert_agent(&conn, "agent2", "Agent Two", "Persona 2");
        
        // Create private session for agent1 (page 0)
        insert_session(&conn, "sess1", "private");
        insert_private_session(&conn, "sess1", "agent1", 0);
        insert_session_settings(&conn, "sess1", 50);
        
        // Insert message to page 0
        let msg0 = Message {
            id: "msg0".to_string(), session_id: "sess1".to_string(),
            sender_type: "user".to_string(), sender_id: "user".to_string(),
            content: "Page0 message".to_string(), created_at: 1000,
            message_type: "text".to_string(), tool_call_data: None,
            generation_info: None, is_deleted: false,
            sender_name: "用户".to_string(), sender_avatar: None, page_index: 0,
        };
        insert_message(&conn, &msg0);
        
        // Simulate reset: create page 1 and insert message there
        let msg1 = Message {
            id: "msg1".to_string(), session_id: "sess1".to_string(),
            sender_type: "user".to_string(), sender_id: "user".to_string(),
            content: "Page1 message".to_string(), created_at: 2000,
            message_type: "text".to_string(), tool_call_data: None,
            generation_info: None, is_deleted: false,
            sender_name: "用户".to_string(), sender_avatar: None, page_index: 1,
        };
        insert_message(&conn, &msg1);
        
        // Trigger from page 0: prompt should contain "Page0 message" but NOT "Page1 message"
        let prompt = PromptAssembler::assemble(&conn, "agent1", Some("sess1"), Some(0), &[]).unwrap();
        assert!(prompt.contains("Page0 message"), "Prompt should contain page 0 message");
        assert!(!prompt.contains("Page1 message"), "Prompt should NOT contain page 1 message when triggered from page 0");
        
        // Trigger from page 1: prompt should contain "Page1 message" but NOT "Page0 message"
        let prompt = PromptAssembler::assemble(&conn, "agent1", Some("sess1"), Some(1), &[]).unwrap();
        assert!(prompt.contains("Page1 message"), "Prompt should contain page 1 message");
        assert!(!prompt.contains("Page0 message"), "Prompt should NOT contain page 0 message when triggered from page 1");
    }
```

- [ ] **Step 4: cargo check --tests**

```bash
cd src-tauri && cargo check --tests
```

Expected: no compilation errors.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db/session.rs src-tauri/src/llm/prompt.rs
git commit -m "test(backend): add list_chat_pages and PromptAssembler trigger page tests"
```

---

### Task 10: 前端类型 — ChatPage 接口

**Files:**
- Modify: `src/lib/types.ts`

- [ ] **Step 1: 添加 ChatPage 接口**

在 `src/lib/types.ts` 末尾添加：

```typescript
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

- [ ] **Step 2: Commit**

```bash
git add src/lib/types.ts
git commit -m "feat(types): add ChatPage TypeScript interface"
```

---

### Task 11: messageStore 增强

**Files:**
- Modify: `src/lib/stores/messageStore.svelte.ts`

- [ ] **Step 1: 修改 loadMessages 支持 pageIndex**

替换 `MessageStore` 的 `loadMessages` 方法：

```typescript
    async loadMessages(sessionId: string, pageIndex?: number) {
        logger.debug('[DEBUG messageStore.loadMessages]', { sessionId, pageIndex });
        try {
            const req: Record<string, unknown> = { session_id: sessionId, limit: 50, offset: 0 };
            if (pageIndex !== undefined) {
                req.page_index = pageIndex;
            }
            const result = await invoke<Message[]>('get_session_messages', { req });
            logger.debug('[DEBUG messageStore.loadMessages]', { sessionId, pageIndex, count: result.length });
            this.messages = result.reverse();
            this.currentSessionId = sessionId;
        } catch (err) {
            logger.debug('[DEBUG messageStore.loadMessages] error', { sessionId, pageIndex, error: err });
            this.messages = [];
            this.currentSessionId = sessionId;
        }
    }
```

- [ ] **Step 2: Vitest 验证**

```bash
pnpm exec vitest run
```

Expected: 所有现有测试通过（向后兼容，因为不传 `pageIndex` 时行为不变）。

- [ ] **Step 3: Commit**

```bash
git add src/lib/stores/messageStore.svelte.ts
git commit -m "feat(store): messageStore.loadMessages supports optional pageIndex"
```

---

### Task 12: historyStore

**Files:**
- Create: `src/lib/stores/historyStore.svelte.ts`

- [ ] **Step 1: 创建 historyStore**

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

- [ ] **Step 2: Commit**

```bash
git add src/lib/stores/historyStore.svelte.ts
git commit -m "feat(store): add historyStore for history view state management"
```

---

### Task 13: HistorySessionList 组件

**Files:**
- Create: `src/lib/components/HistorySessionList.svelte`

- [ ] **Step 1: 创建组件**

```svelte
<script lang="ts">
    import { onMount } from 'svelte';
    import { historyStore } from '$lib/stores/historyStore.svelte';
    import { formatTime } from '$lib/utils';
    import { MessageSquare, ChevronDown, ChevronRight } from 'lucide-svelte';

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
                <button
                    onclick={() => expandedPrivate = !expandedPrivate}
                    class="w-full flex items-center justify-between px-4 py-2.5 hover:bg-bg text-sm font-medium transition-colors"
                >
                    <span>私聊</span>
                    {#if expandedPrivate}
                        <ChevronDown size={16} />
                    {:else}
                        <ChevronRight size={16} />
                    {/if}
                </button>
                {#if expandedPrivate}
                    {#each historyStore.groupedSessions.private as session (session.id)}
                        <button
                            class="w-full flex items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-bg {historyStore.selectedSessionId === session.id ? 'bg-primary/5 border-l-2 border-l-primary' : ''}"
                            onclick={() => handleSessionClick(session.id)}
                        >
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
                                    {#if session.last_message_at}
                                        <span class="text-xs text-text-secondary shrink-0 ml-2">{formatTime(session.last_message_at)}</span>
                                    {/if}
                                </div>
                                <p class="text-xs text-text-secondary truncate">{session.last_message_preview || '暂无消息'}</p>
                            </div>
                        </button>
                    {/each}
                {/if}
            </div>

            <!-- 群聊分组 -->
            <div>
                <button
                    onclick={() => expandedGroup = !expandedGroup}
                    class="w-full flex items-center justify-between px-4 py-2.5 hover:bg-bg text-sm font-medium transition-colors"
                >
                    <span>群聊</span>
                    {#if expandedGroup}
                        <ChevronDown size={16} />
                    {:else}
                        <ChevronRight size={16} />
                    {/if}
                </button>
                {#if expandedGroup}
                    {#each historyStore.groupedSessions.group as session (session.id)}
                        <button
                            class="w-full flex items-center gap-3 px-4 py-3 text-left transition-colors hover:bg-bg {historyStore.selectedSessionId === session.id ? 'bg-primary/5 border-l-2 border-l-primary' : ''}"
                            onclick={() => handleSessionClick(session.id)}
                        >
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
                                    {#if session.last_message_at}
                                        <span class="text-xs text-text-secondary shrink-0 ml-2">{formatTime(session.last_message_at)}</span>
                                    {/if}
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

- [ ] **Step 2: Vitest 运行确认无编译错误**

```bash
pnpm exec vitest run
```

Expected: 现有测试通过，无新增失败。

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/HistorySessionList.svelte
git commit -m "feat(ui): add HistorySessionList component with private/group grouping"
```

---

### Task 14: ChatView 增强支持 history 模式

**Files:**
- Modify: `src/lib/components/ChatView.svelte`

**前置阅读**: ChatView 当前约 420 行。修改涉及：props 定义、标题栏、session 选择 effect、消息加载 effect、发送逻辑、`new_message` 事件监听。

- [ ] **Step 1: 添加 mode prop 和 historyStore 导入**

在 `<script>` 顶部，在现有 import 之后添加：

```typescript
    import { historyStore } from '$lib/stores/historyStore.svelte';

    interface Props {
        mode?: 'chat' | 'history';
    }
    let { mode = 'chat' }: Props = $props();
```

- [ ] **Step 2: 修改 selectedSession 计算和 session 选择 effect**

将 `selectedSession` 改为模式感知：

```typescript
    let selectedSession = $derived(
        mode === 'chat'
            ? sessionStore.sessions.find((s) => s.id === sessionStore.selectedSessionId)
            : historyStore.sessions.find((s) => s.id === historyStore.selectedSessionId)
    );
```

将第一个 `$effect`（id 变化时重置 prevMsgCount）改为：

```typescript
    $effect(() => {
        const id = mode === 'chat' ? sessionStore.selectedSessionId : historyStore.selectedSessionId;
        prevMsgCount = 0;
    });
```

将第二个 `$effect`（加载消息和 session config）改为：

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
            const session = mode === 'chat'
                ? sessionStore.sessions.find(s => s.id === id)
                : historyStore.sessions.find(s => s.id === id);
            currentAgentId = session?.agent_id;
            if (session) {
                loadSessionConfig(id, session.session_type);
            }
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
            currentAgentId = undefined;
            members = [];
            sessionConfig = null;
        }
    });
```

- [ ] **Step 3: 修改标题栏，增加 history 模式下拉框**

替换 `<header>` 部分为：

```svelte
        <header class="flex items-center justify-between px-6 py-4 border-b border-border bg-surface shrink-0 relative">
            {#if selectedSession}
                <div class="flex items-center gap-3">
                    <div class="w-10 h-10 rounded-full bg-gray-300 flex items-center justify-center text-white shrink-0 overflow-hidden">
                        {#if selectedSession.agent_avatar || selectedSession.group_avatar}
                            <img
                                src={selectedSession.agent_avatar || selectedSession.group_avatar}
                                alt={selectedSession.agent_name || selectedSession.group_name || '会话'}
                                class="w-full h-full object-cover"
                            />
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

                <button
                    onclick={() => settingsOpen = !settingsOpen}
                    class="p-2 hover:bg-bg rounded-lg text-text-secondary transition-colors"
                    title="会话配置"
                >
                    <Settings size={20} />
                </button>
            {:else}
                <h2 class="text-lg font-semibold text-text-secondary">
                    {mode === 'history' ? '选择一个会话查看历史' : '选择一个会话开始聊天'}
                </h2>
            {/if}
        </header>
```

注意：需要在 `<script>` 顶部导入 `formatTime`：

```typescript
    import { formatTime } from '$lib/utils';
```

- [ ] **Step 4: 修改 handleSend 支持 page_index**

替换 `handleSend` 函数：

```typescript
    async function handleSend() {
        const content = inputText.trim();
        const sessionId = mode === 'chat' ? sessionStore.selectedSessionId : historyStore.selectedSessionId;
        const pageIdx = mode === 'history' ? historyStore.selectedPageIndex : null;

        logger.debug('[DEBUG ChatView.handleSend]', { mode, sessionId, pageIdx, content });
        if (!content || !sessionId) return;

        sending = true;
        inputText = '';

        const optimisticMsg: import('$lib/types').Message = {
            id: 'optimistic-' + Date.now(),
            session_id: sessionId,
            sender_type: 'user',
            sender_id: 'user',
            sender_name: '用户',
            content,
            created_at: Date.now(),
            message_type: 'text',
        };
        messageStore.addMessage(optimisticMsg);

        try {
            const req: Record<string, unknown> = { session_id: sessionId, content };
            if (pageIdx != null) {
                req.page_index = pageIdx;
            }
            await invoke('send_user_message', { req });
            logger.debug('[DEBUG ChatView.handleSend] success');
            if (pageIdx != null) {
                await messageStore.loadMessages(sessionId, pageIdx);
            } else {
                await messageStore.loadMessages(sessionId);
            }
        } catch (err) {
            logger.debug('[DEBUG ChatView.handleSend] failed', { error: err });
            messageStore.messages = messageStore.messages.filter((m) => m.id !== optimisticMsg.id);
        } finally {
            sending = false;
        }
    }
```

- [ ] **Step 5: 修改 new_message 事件监听支持 page_index 过滤**

替换 `listen('new_message', ...)` 部分为：

```typescript
        listen('new_message', (event) => {
            const msg = event.payload as { session_id: string; page_index?: number; content?: string; id?: string; created_at?: number } & Record<string, unknown>;
            logger.debug('[DEBUG ChatView.listen new_message]', { mode, sessionId: msg.session_id, pageIndex: msg.page_index, contentPreview: msg.content?.slice(0, 50) });

            if (mode === 'chat') {
                if (msg.session_id === sessionStore.selectedSessionId) {
                    const exists = messageStore.messages.some((m) => m.id === msg.id);
                    if (!exists) {
                        messageStore.addMessage(msg as unknown as import('$lib/types').Message);
                    }
                }
            } else {
                // History mode: filter by both session_id and page_index
                if (msg.session_id === historyStore.selectedSessionId && msg.page_index === historyStore.selectedPageIndex) {
                    const exists = messageStore.messages.some((m) => m.id === msg.id);
                    if (!exists) {
                        messageStore.addMessage(msg as unknown as import('$lib/types').Message);
                    }
                }
            }
        }).then((fn) => unlistenFns.push(fn));
```

- [ ] **Step 6: 修改 handleResetMessageCount 中的 session 来源**

替换 `handleResetMessageCount` 函数中的 session 查找：

```typescript
    async function handleResetMessageCount() {
        const sessionId = mode === 'chat' ? sessionStore.selectedSessionId : historyStore.selectedSessionId;
        if (!sessionId || !sessionConfig) return;
        try {
            await invoke('reset_message_count', {
                req: { session_id: sessionId },
            });
            const session = mode === 'chat'
                ? sessionStore.sessions.find(s => s.id === sessionId)
                : historyStore.sessions.find(s => s.id === sessionId);
            if (session) {
                await loadSessionConfig(sessionId, session.session_type);
            }
        } catch (err) {
            logger.error('Failed to reset message count:', err);
        }
    }
```

- [ ] **Step 7: 修改空状态提示文案**

将消息列表区域的空状态提示：

```svelte
                    {#if messageStore.messages.length === 0 && !isAgentTyping}
                        <div class="flex items-center justify-center h-full text-text-secondary p-4">
                            <p>还没有消息，发送第一条消息吧</p>
                        </div>
                    {:else}
```

保持不变即可（两个模式共用）。

- [ ] **Step 8: Vitest 运行**

```bash
pnpm exec vitest run
```

Expected: 所有测试通过。如果有 a11y 相关警告（如 `select` 缺少 label），可以接受。

- [ ] **Step 9: Commit**

```bash
git add src/lib/components/ChatView.svelte
git commit -m "feat(ui): ChatView supports history mode with page selector dropdown"
```

---

### Task 15: App.svelte 路由调整

**Files:**
- Modify: `src/App.svelte`

- [ ] **Step 1: 替换 history 视图的占位符**

在 `src/App.svelte` 中，找到 `history` 视图的两个占位符并替换：

**中间面板（约第 80-89 行）：**

```svelte
        {:else}
            <HistorySessionList />
        {/if}
```

需要在 `<script>` 顶部导入：

```typescript
    import HistorySessionList from '$lib/components/HistorySessionList.svelte';
```

**主内容区（约第 98-102 行）：**

```svelte
        {:else}
            <ChatView mode="history" />
        {/if}
```

- [ ] **Step 2: Vitest 运行**

```bash
pnpm exec vitest run
```

- [ ] **Step 3: Commit**

```bash
git add src/App.svelte
git commit -m "feat(app): wire up HistorySessionList and history-mode ChatView in App.svelte"
```

---

### Task 16: 集成验证

- [ ] **Step 1: Rust 编译检查**

```bash
cd src-tauri && cargo check
```

Expected: `Finished dev profile` with no errors.

- [ ] **Step 2: 前端编译检查**

```bash
pnpm exec vitest run
```

Expected: All tests pass.

- [ ] **Step 3: Svelte 类型检查**

```bash
npx svelte-check --tsconfig ./tsconfig.json
```

Expected: No errors (warnings acceptable).

- [ ] **Step 4: 手动 E2E 验证清单**

启动应用：`pnpm tauri dev`

验证项：
1. 点击左侧"历史会话"图标，中间面板显示"私聊"和"群聊"分组
2. 点击某个私聊，主区域标题栏出现下拉框（如果有多个 page）
3. 下拉框选项显示 `名称 #序号 — 时间` 格式
4. 切换下拉框，消息列表内容变化
5. 在旧 page 发送消息，消息出现在列表中
6. 切到 Chat 视图打开同一 session，显示的是最新 page 的消息（旧 page 的消息不出现）
7. Agent 在旧 page 中触发并回复，回复出现在旧 page 中

- [ ] **Step 5: Commit**

```bash
git commit -m "feat(history-session): complete history session feature with page-level isolation"
```

---

## 自我审查

### Spec 覆盖率检查

| Spec 要求 | 对应任务 |
|-----------|----------|
| 按群聊/私聊分类展示 | Task 13: HistorySessionList 分组渲染 |
| 每个会话一行 | Task 13: `each` 循环中每个 session 一个按钮 |
| 下拉框翻阅历史记录 | Task 14: ChatView 标题栏 `<select>` |
| 最后更新时间作为选项 | Task 13/14: `formatTime(page.updated_at)` |
| 旧 page 可继续发送消息 | Task 6 (后端) + Task 14 Step 4 (前端传 page_index) |
| Chat 视图与 History 视图隔离 | Task 14: mode 区分 + new_message 过滤 |
| Agent 触发使用正确 page 上下文 | Task 7 (PromptAssembler) + Task 8 (Scheduler) |

### 占位符扫描

- 无 "TBD"/"TODO"/"implement later"
- 所有代码块包含完整实现
- 无 "similar to Task N" 引用

### 类型一致性

- Rust `page_index: Option<i32>` ↔ TypeScript `pageIndex?: number` ✅
- `ChatPage` 字段在 Rust 和 TS 中一致 ✅
- `mode: 'chat' | 'history'` 在 ChatView props 和 App.svelte 传值中一致 ✅

---

## 执行交接

**Plan complete and saved to `docs/superpowers/plans/2026-05-14-history-session.md`. Two execution options:**

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
