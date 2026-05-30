# 会话重置成员快照设计文档

## 背景与目标

当前系统支持会话重置（`reset_session`），重置后旧 page 的消息保留在历史记录中，用户可在历史会话模式下继续对话。但存在以下问题：

1. **成员状态不固定**：重置后如果群聊成员被删除/移除，历史会话中查看旧 page 时，目标角色和消息发送者信息依赖当前 session 成员，导致显示错误
2. **人设切换无隔离**：用户在不同人设下参与的同一历史 page，角色看到的用户身份可能混乱

本需求目标：
- 重置会话时，固定当前成员状态（快照）到 `chat_page`
- 历史会话查看和继续对话时，以快照中的成员状态为准
- 已删除成员在历史会话中显示为"未知角色"，不再被调用

## 数据库设计

### 新建表：`chat_page_participants`

```sql
CREATE TABLE chat_page_participants (
    chat_page_id TEXT NOT NULL,
    participant_id TEXT NOT NULL,
    participant_type TEXT NOT NULL CHECK(participant_type IN ('user', 'agent')),
    participant_name TEXT NOT NULL,
    participant_avatar TEXT,
    participant_simplified_persona TEXT,
    PRIMARY KEY (chat_page_id, participant_id, participant_type),
    FOREIGN KEY (chat_page_id) REFERENCES chat_pages(id) ON DELETE CASCADE
);
```

### 迁移：V22

```sql
CREATE TABLE chat_page_participants (
    chat_page_id TEXT NOT NULL,
    participant_id TEXT NOT NULL,
    participant_type TEXT NOT NULL CHECK(participant_type IN ('user', 'agent')),
    participant_name TEXT NOT NULL,
    participant_avatar TEXT,
    participant_simplified_persona TEXT,
    PRIMARY KEY (chat_page_id, participant_id, participant_type),
    FOREIGN KEY (chat_page_id) REFERENCES chat_pages(id) ON DELETE CASCADE
);
```

## 数据流

### 重置时生成快照

```
reset_session(session_id)
  ├─ 创建新 chat_page (page_index + 1)
  ├─ 查询 session 当前成员
  │    ├─ 私聊: private_sessions (participant_1, participant_2)
  │    └─ 群聊: group_members
  ├─ 查询成员名称/头像/人设摘要
  │    ├─ agent: agents.name, agents.avatar_path, agents.simplified_persona
  │    └─ user: user_personas.name, user_personas.avatar_path
  ├─ 插入 chat_page_participants 快照
  └─ 触发 spawn_session_summary / spawn_generate_page_title
```

### 历史会话发送消息

```
send_history_message(session_id, page_index)
  ├─ resolve_history_target_agents(session_id, page_index)
  │    ├─ 获取该 page 对应的 chat_page_id
  │    ├─ 查询 chat_page_participants
  │    │    WHERE chat_page_id = ? AND participant_type = 'agent'
  │    ├─ JOIN agents 过滤 is_deleted = 0
  │    └─ 返回 agent_ids
  ├─ 为每个 agent 组装 HistoryPromptAssembler
  │    └─ 参与者注入使用 chat_page_participants 快照
  └─ 执行 LLM 调用
```

### 历史消息渲染

```
MessageBubble (history mode)
  ├─ 获取 sender_id, sender_type
  ├─ 查询 chat_page_participants
  │    WHERE chat_page_id = ? AND participant_id = sender_id
  ├─ 如果找到：显示 participant_name / participant_avatar
  └─ 如果未找到：显示"未知角色" + 默认头像
```

## API 变更

### `resolve_history_target_agents`

**当前逻辑：**
```rust
fn resolve_history_target_agents(conn, session_id) -> Vec<String> {
    // 查当前 session 成员
    if private: 查 private_sessions
    if group: 查 group_members WHERE participant_type = 'agent'
}
```

**新逻辑：**
```rust
fn resolve_history_target_agents(conn, session_id, page_index) -> Vec<String> {
    // 1. 获取 chat_page_id
    let chat_page_id = get_chat_page_id(conn, session_id, page_index)?;
    
    // 2. 查快照成员
    SELECT cpp.participant_id 
    FROM chat_page_participants cpp
    JOIN agents a ON cpp.participant_id = a.id
    WHERE cpp.chat_page_id = ? 
      AND cpp.participant_type = 'agent'
      AND a.is_deleted = 0
}
```

### `HistoryPromptAssembler::get_participants`

**当前逻辑：**
```rust
fn get_participants(conn, agent_id) {
    // 查当前 active_persona_id 的关系
    list_relationships_by_observer(conn, agent_id)
}
```

**新逻辑（历史模式）：**
```rust
fn get_participants_for_page(conn, agent_id, chat_page_id) {
    // 1. 查快照中的所有参与者（名称、头像、人设摘要）
    SELECT participant_id, participant_type, participant_name, participant_simplified_persona
    FROM chat_page_participants
    WHERE chat_page_id = ?
    
    // 2. 对 agent 类型的参与者，查 relationship_text / memory_text（实时）
    // 3. 对 user 类型的参与者，使用快照中的 participant_name
    // 4. 标签（好友/群友）实时推导
}
```

## 前端渲染变更

### `ChatView` / `MessageBubble`

历史模式下，消息渲染不再依赖 `sender_name` 字段（该字段是实时 JOIN agents 表获取的，无法反映快照状态），而是：

1. 加载消息时，同时加载该 page 的 `chat_page_participants` 快照
2. 渲染每条消息时，用 `sender_id` 查快照 Map
3. 如果找到：显示 `participant_name` + `participant_avatar`
4. 如果未找到：显示"未知角色" + 默认占位头像

### `HistorySessionList`

历史会话列表中显示的成员信息，也应使用快照数据而非当前 session 成员。

## 兼容性考虑

### 已有 chat_page 无快照

迁移前已存在的 chat_pages（page_index = 0 的初始 page）没有快照。处理方案：

- `resolve_history_target_agents`：如果查不到快照，fallback 到当前 session 成员（向后兼容）
- 消息渲染：如果查不到快照，fallback 到 `sender_name` 字段

### 已删除成员（情况1 vs 情况2）

| 场景 | 快照中是否存在 | 历史会话中显示 | 是否被调用 |
|------|--------------|--------------|----------|
| 重置前已被移除/删除 | 否 | "未知角色" | 否 |
| 重置时还在，之后被删除 | 是 | 快照名称/头像 | 否（被 `agents.is_deleted = 0` 过滤）|

## 边界情况

1. **群聊重置后新增成员**：新成员只属于新 page，旧 page 快照不包含
2. **私聊中用户人设切换**：快照记录重置时的 user_persona 名称/头像
3. **快照成员名称后续修改**：历史会话中仍显示快照中的旧名称（符合预期：历史应保持原貌）
4. **page_index = 0 的初始 page**：迁移后无快照，fallback 到当前成员

## 实现范围

### 必须实现
- [ ] Schema + Migration V22
- [ ] `reset_session` 插入快照逻辑
- [ ] `resolve_history_target_agents` 历史模式改查快照
- [ ] `HistoryPromptAssembler` 参与者注入使用快照
- [ ] 前端历史消息渲染使用快照

### 可选增强
- [ ] 为已有 chat_pages（page_index=0）补充快照（数据迁移脚本）
- [ ] `HistorySessionList` 显示快照成员列表

## 与后续记忆归属需求的衔接

本需求完成后，`chat_page` 将拥有独立的成员快照。后续处理记忆归属时：
- `spawn_session_summary` 可以获取该 page 对应的 `chat_page_id`
- 从快照中确定该 page 中用户的人设身份（`user_persona_id`）
- 总结生成的记忆归属到正确的 `(agent, user_persona)` 组合
