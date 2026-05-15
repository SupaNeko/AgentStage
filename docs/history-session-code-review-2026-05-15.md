# 历史消息会话实现代码审查报告

**日期**: 2026-05-15  
**审查范围**: 前端群聊成员列表闪烁、会话列表预览错误、History 模式前后端实现一致性  
**方法**: 代码走读 + 调用链追踪 + 与需求规格对比

---

## 一、当前实现的调用逻辑

### 1.1 后端链路

#### Chat 模式（当前会话）
```
用户发送消息
  → send_user_message
    → insert_message (page_index = current_chat_page)
    → update_session_last_message (更新 sessions.last_message_preview)
    → scheduler.on_new_message
      → distribute_message → unread queue
      → try_trigger_agent → LLM
      → emit("new_message")
```

#### History 模式（历史会话）
```
用户发送消息
  → send_history_message (传入 session_id + page_index)
    → insert_message (page_index = 指定的历史 page)
    → update_session_last_message (更新 sessions.last_message_preview)  ← 问题点
    → get_messages_by_session (limit=1000, 查询该 page 所有消息)
    → resolve_history_target_agents (私聊=对方Agent, 群聊=所有Agent成员)
    → HistoryPromptAssembler::assemble
      → 获取 Agent detailed_persona
      → 格式化传入的 history_messages 为时间+发送者+内容字符串
      → 拼接 SYSTEM_PROMPT + 人设 + 消息历史 + 引导语
    → 创建 OpenAiCompatibleProvider
    → provider.chat (不提供 tools, prompt 作为 user message)
    → insert_message (Agent 回复, 同一 page_index)
    → 返回 Vec<Message>
```

#### list_chat_pages（历史 page 列表）
```
查询 chat_pages 表
  → LEFT JOIN messages 聚合 msg_count 和 last_msg_at
  → WHERE page_index < current_chat_page (当前已排除 current_chat_page)
  → ORDER BY page_index DESC
```

### 1.2 前端链路

#### Chat 模式
```
ChatView mode="chat"
  → $effect 监听 sessionStore.selectedSessionId
    → messageStore.loadMessages(sessionId) (不传 page_index, 查 current_chat_page)
    → 如果是群聊: invoke("get_group_members") → members state
  → handleSend
    → invoke("send_user_message")
    → messageStore.loadMessages(sessionId)
  → onMount 监听 new_message / agent_typing / agent_completed / agent_error
    → new_message: messageStore.addMessage(msg)  ← 问题点
```

#### History 模式
```
ChatView mode="history"
  → $effect 监听 historyStore.selectedSessionId + historyStore.selectedPageIndex
    → messageStore.loadMessages(sessionId, pageIdx)
    → 如果是群聊: invoke("get_group_members") → members state  ← 问题点
  → handleSend
    → invoke("send_history_message", { session_id, content, page_index })
    → messageStore.loadMessages(sessionId, pageIdx)
  → onMount 不监听任何事件
```

#### SessionList（会话列表）
```
遍历 sessionStore.sessions
  → 显示 session.last_message_preview 和 session.last_message_at
  → 预览来源: 后端 sessions 表 或 messageStore.loadMessages 中的同步更新
```

#### HistorySessionList（历史会话列表）
```
遍历 historyStore.sessions
  → groupedSessions 按 session_type === 'private' / 'group' 分组
  → 显示 session.last_message_preview 和 session.last_message_at  ← 问题点
```

---

## 二、Bug 分析

### Bug 1: 前端群聊成员列表一直在闪烁

**现象**: 在 Chat 模式的群聊中，右侧成员列表反复重新渲染，表现为闪烁。

**根因追踪**:

`ChatView.svelte` 中存在一个 `$effect`，其内部为了获取当前会话的 `session_type`，通过 `.find()` 访问了 `sessionStore.sessions` 数组。该 `$effect` 的核心逻辑是：当选中的 session 是群聊时，调用 `get_group_members` 获取成员列表并写入 `members` state。

问题在于 `$effect` 的依赖追踪粒度。Svelte 5 的 `$effect` 会自动追踪其内部读取的所有 `$state` 变量。当 `sessionStore.sessions` 数组中任何一个元素的任何一个字段发生变化时（例如 `last_message_preview` 被 `updateSessionPreview` 更新，这会通过 `map` 创建一个新的数组引用），`$effect` 判定其依赖发生了变化，从而重新执行。

重新执行时：
1. `id`（selectedSessionId）没有变化；
2. 但 `sessionStore.sessions` 数组已经变化；
3. `.find()` 重新查找 session；
4. `session_type === 'group'` 仍然成立；
5. `get_group_members` 再次被调用；
6. 返回的新数组即使内容相同，引用也不同；
7. `members` state 被赋新值，触发 Svelte 重新渲染成员列表 DOM；
8. 这就表现为"闪烁"。

**触发场景**: 每当 `messageStore.loadMessages` 成功返回并调用 `sessionStore.updateSessionPreview` 时，或者任何其他代码更新了 `sessionStore.sessions` 时，都会触发该 `$effect` 重新加载群成员。

### Bug 2: 会话列表中标签的最后一句话显示依旧错误

**现象**: `SessionList` 中当前会话的消息预览在某些场景下显示为旧消息（特别是 Agent 回复后预览未更新，以及刚重置对话后仍显示旧预览）。

**根因追踪**:

`SessionList` 中显示的消息预览绑定的是 `session.last_message_preview`。该字段有两个更新来源：

**来源 A（后端）**: `send_user_message` 在插入用户消息后调用 `update_session_last_message`，将 `sessions.last_message_preview` 设置为用户消息内容。Agent 回复时，`scheduler` 中的 `distribute_message` 和 `ToolExecutor` 也会触发 `update_session_last_message`，将预览更新为 Agent 消息内容。但这里的更新发生在后端 DB，前端 `sessionStore.sessions` 不会自动感知这个变化，除非重新调用 `loadSessions()`。

**来源 B（前端）**: `messageStore.loadMessages` 在加载当前 page（`pageIndex === undefined`）成功后，会调用 `sessionStore.updateSessionPreview`，将最后一条非系统消息的内容同步到 `sessionStore.sessions`。

**问题所在**:

`ChatView.svelte` 中 Chat 模式的 `new_message` 事件处理逻辑如下：
1. 判断 `msg.session_id === sessionStore.selectedSessionId`；
2. 如果消息不存在于 `messageStore.messages`，则调用 `messageStore.addMessage(msg)` 追加到列表。

注意：这里**没有调用** `messageStore.loadMessages`，也没有调用 `sessionStore.updateSessionPreview`。

因此，当 Agent 回复消息并通过 `new_message` 事件推送到前端时：
1. `messageStore.messages` 被追加了一条新消息（通过 `addMessage`）；
2. 但 `sessionStore` 中的 `last_message_preview` 仍然是旧值；
3. `SessionList` 中该会话的预览显示为旧消息。

同样，在"刚重置对话"的场景中：
1. `resetSession` 后端命令不会清空 `sessions.last_message_preview`；
2. `sessionStore.resetSession` 前端方法确实手动将预览设置为空字符串；
3. 但 `messageStore.loadMessages` 加载新 page（空）后也会调用 `updateSessionPreview(sessionId, '', 0)`；
4. 如果用户切换视图后重新加载会话列表（`loadSessions`），后端返回的 `last_message_preview` 可能仍然是旧值（因为后端 `reset_session` 没有清空它）。

---

## 三、历史会话实现与预期的不一致之处

### 3.1 `send_history_message` 更新了 `sessions.last_message_preview`

**预期**: History 模式是完全独立的链路，不应该影响当前会话的任何状态。

**现状**: `send_history_message` 在插入用户消息后调用了 `update_session_last_message`，这会修改 `sessions` 表的 `last_message_preview` 和 `last_message_at`。虽然前端 `HistorySessionList` 可以将预览置空不显示，但后端仍然写入了脏数据。更重要的是，如果此时前端 Chat 模式的 `SessionList` 被刷新（`loadSessions`），它会读到被 History 模式污染后的 `last_message_preview`。

### 3.2 `HistoryPromptAssembler` 使用了全局 `SYSTEM_PROMPT`

**预期**: History 模式的 prompt 应该只包含当前 session + page 的消息和 Agent 人设。

**现状**: `HistoryPromptAssembler::assemble` 拼接了 `SYSTEM_PROMPT`（来自 `prompt_templates.rs`）+ Agent `detailed_persona` + 消息历史 + 引导语。这与 `PromptAssembler` 的 Layer 1（System Prompt）和 Layer 2（Persona）结构相同，本身不是错误。但需要确认 `SYSTEM_PROMPT` 的内容是否包含与"跨 session 工具调用"相关的指令，如果包含，则 History 模式下不应注入这些指令（因为 History 模式不提供 tools）。

### 3.3 前端 HistorySessionList 仍显示 `last_message_preview`

**预期**: "前端显示的历史会话中，不用在会话列表的标签页显示最后一次的会话内容，永远置空即可。"

**现状**: `HistorySessionList.svelte` 中私聊和群聊的会话卡片都显示了 `{session.last_message_preview || '暂无消息'}`。

### 3.4 `messageStore.loadMessages` 的同步更新机制有副作用

**现状**: `messageStore.loadMessages` 中添加了逻辑：仅在 `pageIndex === undefined`（加载当前 page）时同步更新 `sessionStore` 预览。这虽然避免了 History 模式污染预览，但将两个 store 的职责耦合在了一起。`MessageStore` 的职责应该是管理消息列表，不应该负责更新 `SessionStore` 的预览字段。

---

## 四、修复方案

### 4.1 Bug 1: 群聊成员列表闪烁

**修复方法**: 将 `$effect` 中对 `sessionStore.sessions` 的读取与 `get_group_members` 的触发解耦。

具体做法：
1. `$effect` 的核心触发条件应该是 `selectedSessionId` 的变化，而不是 `sessions` 数组的变化；
2. 在 `$effect` 中读取 `id`（selectedSessionId）后，使用 `untrack` 来读取 `sessionStore.sessions` 数组进行 `.find()` 查找；
3. 这样，只有当用户真正切换了选中的会话时，`$effect` 才会重新执行并调用 `get_group_members`；
4. `sessions` 数组中其他字段（如 `last_message_preview`）的变化不会触发成员列表重新加载，从而消除闪烁。

### 4.2 Bug 2: 会话列表预览错误

**修复方法**: 在 `new_message` 事件处理中同步更新 `sessionStore` 预览。

具体做法：
1. 在 `ChatView.svelte` 的 `new_message` 事件监听器中，当确认消息应该被添加到当前会话后，不仅调用 `messageStore.addMessage`，还要调用 `sessionStore.updateSessionPreview`；
2. 更新的内容应为新消息的 `content` 和 `created_at`；
3. 这样 Agent 回复推送到达前端时，预览会立即被更新为 Agent 的最新回复；
4. 对于"刚重置对话"的场景，需要确保后端 `reset_session` 命令在事务中也清空 `sessions.last_message_preview`（置为空字符串），这样前端 `loadSessions()` 加载到的数据就是干净的。

### 4.3 `send_history_message` 移除 `update_session_last_message`

**修复方法**: 在 `send_history_message` 中删除对 `session_repo::update_session_last_message` 的调用。

理由：
1. History 模式是独立链路，其消息写入历史 page，不应该更新 session 级别的预览字段；
2. 当前会话的预览应该只反映 `current_chat_page` 的消息活动；
3. 移除后，`sessions.last_message_preview` 只会被 `send_user_message`（当前 page）和 `reset_session`（清空）更新。

### 4.4 前端 HistorySessionList 预览置空

**修复方法**: 在 `HistorySessionList.svelte` 中，将会话卡片的预览显示固定为空字符串，不再读取 `session.last_message_preview`。

理由：
1. 用户明确要求"永远置空"；
2. 历史会话可以随意切换 page，不需要维护"最后一条消息"的预览；
3. 时间显示 `last_message_at` 可以保留（因为时间本身不影响功能，且用户未要求置空时间）。

### 4.5 解耦 `MessageStore` 与 `SessionStore`

**修复方法**: 将 `messageStore.loadMessages` 中的 `sessionStore.updateSessionPreview` 调用移除。

理由：
1. `MessageStore` 的职责边界是管理消息列表，`SessionStore` 的职责边界是管理会话元数据；
2. 预览更新应该由调用方（如 `ChatView` 的 `new_message` 处理、`handleSend` 成功回调）来负责；
3. 解耦后，`MessageStore` 不需要导入 `SessionStore`，避免循环依赖风险。

替代方案：在 `ChatView` 中统一维护预览更新逻辑：
- `handleSend` 成功后，手动更新 `sessionStore` 预览为用户消息内容；
- `new_message` 事件到达后，手动更新 `sessionStore` 预览为新消息内容；
- `resetSession` 成功后，手动清空预览。

---

## 五、总结

| 问题 | 根因 | 修复方法 |
|------|------|----------|
| 群聊成员列表闪烁 | `$effect` 依赖了 `sessionStore.sessions` 数组变化 | 使用 `untrack` 隔离 `sessions` 读取，使 `$effect` 只在 `id` 变化时触发 |
| 会话预览不更新 | `new_message` 事件未更新 `sessionStore` 预览 | 事件处理中追加 `updateSessionPreview` 调用 |
| 重置后预览仍显示旧消息 | 后端 `reset_session` 未清空 `last_message_preview` | 后端 `reset_session` 事务中追加清空 `last_message_preview` |
| History 污染当前预览 | `send_history_message` 调用了 `update_session_last_message` | 移除该调用 |
| History 列表显示旧预览 | `HistorySessionList` 绑定了 `session.last_message_preview` | 固定显示空字符串 |
| Store 职责耦合 | `MessageStore.loadMessages` 直接修改 `SessionStore` | 移除该逻辑，由调用方（ChatView）统一管理预览更新 |
