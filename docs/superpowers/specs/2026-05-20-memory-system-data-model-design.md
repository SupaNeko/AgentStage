# AGT-18 记忆系统数据模型与 UI 配置 — 设计文档

> 文档版本：V1.0  
> 编写日期：2026-05-20  
> 关联需求：CHAT-36（记忆功能—长期任务维护）、CHAT-39-ext（Prompt 注入重构）

---

## 1. 背景与目标

当前系统注入的历史聊天信息是有限的。如果一个群聊持续进行，早期的重要信息（如某角色提到过喜欢吃苹果）会被后续消息淹没，导致角色"遗忘"。

AGT-18 为整个记忆机制提供**基础数据层和 UI 配置层**，包含：
- 每个角色的**长期记忆**（关于自身的长文本）
- 每个角色对**其他参与者**的记忆（按目标分组的短文本）
- 记忆功能的全局开关与重置机制
- 角色配置页的**记忆标签页**

> **范围边界**：AGT-18 只负责"数据怎么存"和"用户怎么配置"。Prompt 中如何注入记忆（措辞、格式、位置）属于 **CHAT-39-ext**；角色如何通过工具更新记忆属于 **CHAT-37-ext**；重置/超长时的 AI 自动总结属于 **CHAT-37 / CHAT-40**。

---

## 2. 核心概念：关系 vs 记忆

这是后续所有设计和提示词的基础：

| 维度 | 关系（Relationship） | 记忆（Memory） |
|------|---------------------|----------------|
| **性质** | 静态的、稳定的 | 动态的、灵活的 |
| **内容** | 关系定位与态度（如"亲子""好友""讨厌"） | 事实、事件、喜好、规律（如"他喜欢吃苹果""上周一起去了游乐园"） |
| **变动频率** | 很少变动，有些不可变（如亲子关系） | 经常更新，可以添加、修改、删除 |
| **维护方式** | `update_relationship` 工具（200字限制） | `update_memory` 工具（500字限制） |
| **注入位置** | 【你认识的参与者】→ [印象] | 【你认识的参与者】→ [记忆] |

---

## 3. 数据模型

### 3.1 Schema 变更（Migration V13）

```sql
-- agents 表：角色自身的长期记忆 + 记忆功能总开关
ALTER TABLE agents ADD COLUMN long_term_memory TEXT DEFAULT '';
ALTER TABLE agents ADD COLUMN memory_enabled INTEGER DEFAULT 1 CHECK(memory_enabled IN (0, 1));

-- agent_relationships 表：对他人的记忆（表合并方案）
ALTER TABLE agent_relationships ADD COLUMN memory_text TEXT NOT NULL DEFAULT '';
```

### 3.2 设计理由：表合并

对他人的记忆与关系描述共享同一套目标对象（用户、好友、群友）。采用表合并而非新建独立表，原因：
1. **对象一致性**：`list_relationships_by_observer` 一次查询即可同时返回关系和记忆，无需 JOIN 或二次查询
2. **实现简洁**：复用现有 Repository 和前端加载逻辑，减少维护成本
3. **更新隔离**：`update_relationship` 只更新 `relationship_text`，`update_memory` 只更新 `memory_text`，互不干扰

> 关系解除（删除好友）时，`friendships` 记录被删除，但 `agent_relationships` 中的 `relationship_text` 和 `memory_text` **均保留**（当前系统已实现此行为）。重新建立关系后，原有内容自动"复活"。

---

## 4. 模型变更（Rust）

### 4.1 Agent / AgentResponse / CreateAgentRequest / UpdateAgentRequest

新增字段：
- `long_term_memory: Option<String>` — 角色自身的长期记忆
- `memory_enabled: bool` — 记忆功能总开关（默认 `true`）

### 4.2 RelationshipItem

新增字段：
- `memory_text: String` — 对该目标角色的记忆内容（默认空字符串）

---

## 5. Repository 层变更

### 5.1 `db/agent_relationship.rs`

- **`list_relationships_by_observer`**：SELECT 语句增加 `COALESCE(ar.memory_text, '') as memory_text`
- **`upsert_memory`**（新增）：
  ```sql
  INSERT INTO agent_relationships (observer_id, target_id, target_type, memory_text, updated_at)
  VALUES (?1, ?2, ?3, ?4, ?5)
  ON CONFLICT(observer_id, target_id, target_type) DO UPDATE SET
      memory_text = excluded.memory_text,
      updated_at = excluded.updated_at
  ```
  注意：此操作**只更新 `memory_text`**，不影响 `relationship_text`。
- **`clear_memories_by_observer`**（新增）：清空该角色作为 observer 的所有 `memory_text`
  ```sql
  UPDATE agent_relationships SET memory_text = '' WHERE observer_id = ?1
  ```

### 5.2 `db/agent.rs`

- `get_agent_by_id`：返回 `long_term_memory` 和 `memory_enabled`
- `update_agent`：支持更新 `long_term_memory` 和 `memory_enabled`
- `clear_long_term_memory`（新增）：清空 `long_term_memory`
  ```sql
  UPDATE agents SET long_term_memory = '' WHERE id = ?1
  ```

---

## 6. Tauri 命令

### 6.1 新增命令

| 命令 | 参数 | 返回 | 作用 |
|------|------|------|------|
| `update_agent_memory` | `observer_id`, `target_id`, `target_type`, `memory_text` | `Result<(), String>` | 更新角色对某参与者的记忆。校验 `memory_text` 不超过 500 字。 |
| `reset_agent_memory` | `agent_id` | `Result<(), String>` | 原子操作：① 清空 `agents.long_term_memory`；② 清空该角色的所有 `agent_relationships.memory_text`。 |

### 6.2 已有命令扩展

- `get_agent`：返回的 `AgentResponse` 包含 `long_term_memory` 和 `memory_enabled`
- `update_agent`：`UpdateAgentRequest` 支持接收 `long_term_memory` 和 `memory_enabled` 字段（部分更新）
- `list_agent_relationships`：返回的每个 `RelationshipItem` 包含 `memory_text`

---

## 7. UI 布局

### 7.1 AgentDetail 标签页扩展

`activeTab` 从 `'config' | 'relationships'` 扩展为 `'config' | 'relationships' | 'memory'`，新增第三个标签：

```svelte
<button onclick={() => activeTab = 'memory'} class="...">
    记忆
</button>
```

### 7.2 AgentMemoryPanel 组件（新建）

`AgentMemoryPanel.svelte`：接收 `agentId: string`。

#### 顶部控制栏
- **启用记忆**开关（checkbox toggle）：
  - 绑定 `memory_enabled`
  - 切换后立即调用 `update_agent({ id: agentId, memory_enabled })`
  - 关闭时，下方所有 textarea **disabled**，并显示提示文案："记忆功能已关闭，当前内容不会被使用"
- **重置记忆**按钮：
  - 红色/警示样式
  - 点击弹出 `ConfirmResetMemoryModal` 二次确认
  - 确认后调用 `reset_agent_memory`，成功后 toast 提示"记忆已重置"

#### 第一区块：角色长期记忆
- 标题："长期记忆"
- 大型 textarea（`rows=8`），绑定 `long_term_memory`
- 占位符："和该角色有关的记忆"
- 实时字数统计：`{longTermMemory.length}/3000`
- 自动保存：debounce **1000ms**，调用 `update_agent({ id: agentId, long_term_memory })`

#### 第二区块：对他人的记忆
- 标题："对他人的记忆"
- 复用 `list_agent_relationships` 的查询结果（对象列表与关系设定完全一致）
- 对每个 `RelationshipItem` 展示卡片：
  - 左侧：头像 + 名称 + 标签（好友/群友/用户）
  - 右侧：textarea（`rows=3`），绑定 `item.memory_text`
  - 占位符："关于此人的重要信息，如喜好、习惯、共同经历..."
  - 字数统计：`{item.memory_text.length}/500`
  - 自动保存：debounce **500ms** + `onblur`，调用 `update_agent_memory`
- 空状态：若该角色无任何关系对象，显示：
  > "该角色尚未与其他参与者建立关联，在群聊或私聊中会自动显示"

### 7.3 保存机制总结

| 内容 | 保存命令 | 触发方式 | 延迟 |
|------|----------|----------|------|
| `memory_enabled` | `update_agent` | 切换开关 | 立即 |
| `long_term_memory` | `update_agent` | 停止输入后 | 1000ms debounce |
| `memory_text`（对他人） | `update_agent_memory` | 停止输入后 | 500ms debounce + onblur |

---

## 8. Prompt 注入位置（数据流定义）

> **注意**：Prompt 的措辞优化和格式重构属于 **CHAT-39-ext**。AGT-18 只定义"记忆内容放在哪里"，为后续实现提供数据契约。

### 8.1 长期记忆

注入位置：【你的角色设定】区块之后，新增【关于你的记忆】区块。

```text
【你的角色设定】
{detailed_persona}

【关于你的记忆】
{long_term_memory}
```

- **注入条件**：`memory_enabled = true` 且 `long_term_memory` 非空

### 8.2 对他人记忆

注入位置：【你认识的参与者】区块中，每个参与者条目内结构化输出。

**当前格式（待 CHAT-39-ext 重构）：**
```text
- {name}（{label}）：{simplified_persona}。[主观关系]：{relationship_text}
```

**目标格式（CHAT-39-ext 实现）：**
```text
- {name}（{label}）：{simplified_persona}
  [印象]：{relationship_text}
  [记忆]：{memory_text}
```

- **注入条件**：`memory_enabled = true` 且该参与者的 `memory_text` 非空时，才显示 `[记忆]` 行
- `[印象]` 对应 `relationship_text`（静态关系），`[记忆]` 对应 `memory_text`（动态事实）

### 8.3 PromptAssembler 数据流

1. 查询 `agents.memory_enabled` → 为 `false` 时**跳过所有记忆注入**
2. 查询 `agents.long_term_memory` → 注入到角色设定下方
3. `list_relationships_by_observer` 已返回 `memory_text` → 在参与者列表中按需注入

---

## 9. 测试要点

### 9.1 Repository 层测试

- `test_upsert_memory_only_updates_memory_text`：验证 `upsert_memory` 不会覆盖 `relationship_text`
- `test_clear_memories_by_observer`：验证 `clear_memories_by_observer` 只清空 `memory_text`，保留 `relationship_text`
- `test_list_relationships_includes_memory_text`：验证查询结果包含 `memory_text`
- `test_reset_agent_memory_clears_both`：验证 `reset_agent_memory` 同时清空长期记忆和所有他人记忆

### 9.2 命令层测试

- `test_update_agent_memory_enforces_500_char_limit`：验证 500 字限制
- `test_update_agent_with_memory_fields`：验证 `update_agent` 支持部分更新 `long_term_memory` 和 `memory_enabled`

### 9.3 前端测试

- 记忆标签页渲染：长期记忆 textarea、对他人记忆卡片列表
- 开关关闭时：textarea disabled，提示文案显示
- 重置弹窗：二次确认后调用命令
- 字数限制：长期记忆 3000 字、他人记忆 500 字

---

## 10. 后续需求衔接

| 需求 | 依赖 AGT-18 的什么 | 衔接点 |
|------|-------------------|--------|
| **CHAT-39-ext** | `long_term_memory`、`memory_text`、`memory_enabled` 的数据结构 | Prompt 重构：在已定义的位置上注入记忆内容，优化措辞和格式 |
| **CHAT-37-ext** | `update_agent_memory` 命令 | 新增 `update_memory` Tool，供角色在对话中实时更新记忆 |
| **CHAT-37** | 完整的记忆数据模型 + `update_memory` Tool | 重置会话时异步调用 AI，使用 `update_memory` 工具保存总结结果 |
| **CHAT-40** | 同上 | 超长消息时异步调用 AI，批量处理旧消息并更新记忆 |
| **CHAT-41** | 记忆数据模型（读取记忆注入 Prompt） | 定时任务触发时，记忆的注入逻辑复用已有 PromptAssembler |
| **CHAT-42** | 同上 | 主动会话触发时，记忆的注入逻辑复用已有 PromptAssembler |

---

## 11. 变更清单

| 文件 | 变更类型 | 说明 |
|------|----------|------|
| `src/db/schema.rs` | 新增 | MIGRATION_V13 |
| `src/db/migration.rs` | 修改 | 注册 V13 |
| `src/models/agent.rs` | 修改 | Agent / AgentResponse / CreateAgentRequest / UpdateAgentRequest 增加字段 |
| `src/models/agent_relationship.rs` | 修改 | RelationshipItem 增加 `memory_text` |
| `src/db/agent.rs` | 修改 | `get_agent_by_id`、`update_agent` 支持新字段；新增 `clear_long_term_memory` |
| `src/db/agent_relationship.rs` | 修改 | `list_relationships_by_observer` 返回 `memory_text`；新增 `upsert_memory`、`clear_memories_by_observer` |
| `src/commands/agent_relationship.rs` | 新增 | `update_agent_memory` 命令 |
| `src/commands/agent.rs` | 修改 | `update_agent` 支持新字段；新增 `reset_agent_memory` 命令 |
| `src/lib.rs` | 修改 | 注册新命令到 `generate_handler!` |
| `src/lib/components/AgentDetail.svelte` | 修改 | 增加 `memory` 标签页 |
| `src/lib/components/AgentMemoryPanel.svelte` | 新建 | 记忆标签页核心组件 |
| `src/lib/components/ConfirmResetMemoryModal.svelte` | 新建 | 重置记忆二次确认弹窗 |
| `src/lib/types.ts` | 修改 | Agent / RelationshipItem 类型增加新字段 |
| `docs/feature_list.md` | 修改 | AGT-18 状态更新 |
