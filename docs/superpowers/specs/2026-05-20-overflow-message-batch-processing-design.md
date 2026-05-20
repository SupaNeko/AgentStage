# CHAT-40 超长消息批量处理 — 设计文档

> 文档版本：V1.0  
> 编写日期：2026-05-20  
> 关联需求：CHAT-37（重置会话时AI总结）、CHAT-36（记忆功能）

---

## 1. 背景与目标

当前系统中，Prompt 注入的历史消息受 `history_limit` 限制（默认 50 条）。如果一个群聊持续活跃，早期的消息会不断被挤出 Prompt 的可见范围。如果这些早期消息包含有价值的信息（如角色的喜好、重要事件），而角色又没有在对话过程中主动使用 `update_memory` 工具维护，信息就会丢失。

CHAT-40 提供一个**自动兜底机制**：当"已超出 `history_limit` 且尚未被处理过"的消息累计达到一定数量时，自动触发一次后台 AI 总结，将其中有价值的信息提取并保存到记忆中。

---

## 2. 核心概念

### 2.1 溢出消息（Overflow Messages）

按时间顺序排列的消息流中，超出 `history_limit` 的 oldest 部分称为"溢出消息"。这些消息不在 Prompt 注入范围内。

### 2.2 累计触发

不针对单条消息触发，而是**批量累计**：每隔固定数量（`overflow_summary_threshold`）的溢出消息，触发一次总结。

### 2.3 处理位置追踪

通过 `last_overflow_summary_index` 记录已经处理到第几条消息，确保同一条消息不会被重复总结。

---

## 3. 数据模型

### 3.1 Schema 变更（Migration V14）

```sql
-- session_settings 表增加溢出总结相关字段
ALTER TABLE session_settings ADD COLUMN overflow_summary_threshold INTEGER DEFAULT 50;
ALTER TABLE session_settings ADD COLUMN last_overflow_summary_index INTEGER DEFAULT 0;
```

### 3.2 字段说明

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `overflow_summary_threshold` | INTEGER | 50 | 溢出总结阈值。当已产生但尚未处理的溢出消息达到此数量时触发 AI 总结。设为 0 表示关闭该功能。 |
| `last_overflow_summary_index` | INTEGER | 0 | 已处理消息的全局索引（按 created_at 排序）。初始为 0，每次触发后增加 threshold。 |

---

## 4. 触发机制

### 4.1 触发位置

在 **Scheduler 的后台扫描循环**（`start_background_scan`，每 5 秒）中增加一个检测步骤。

### 4.2 触发条件

对每个活跃会话（`is_deleted = 0`）：

```
IF overflow_summary_threshold > 0
   AND total_messages - last_overflow_summary_index >= overflow_summary_threshold
THEN 触发
```

其中 `total_messages` 是当前活跃 page（`page_index = current_chat_page`）的消息总数。

### 4.3 触发后状态更新

触发并完成后：
```sql
UPDATE session_settings
SET last_overflow_summary_index = last_overflow_summary_index + overflow_summary_threshold
WHERE session_id = ?;
```

---

## 5. 消息查询与总结逻辑

### 5.1 消息范围

查询当前 page 中，按 `created_at ASC` 排序，取：
- `OFFSET = last_overflow_summary_index`
- `LIMIT = overflow_summary_threshold`

SQL：
```sql
SELECT ... FROM messages
WHERE session_id = ?1 AND page_index = ?2 AND is_deleted = 0
ORDER BY created_at ASC
LIMIT ?3 OFFSET ?4
```

### 5.2 总结流程

`run_overflow_summary(session_id, page_index, last_index, threshold)`：

1. 按上述 SQL 查询消息（`LIMIT = threshold`, `OFFSET = last_index`）
2. 将会话中的所有 agent 参与者查出（复用 CHAT-37 逻辑）
3. 对每个 `memory_enabled=true` 且 LLM 配置完整的 agent：
   - 构建专用 Prompt（复用 `SUMMARY_SYSTEM_PROMPT`）
   - 传入的消息文本为本次查询到的 `threshold` 条消息
   - 调用 LLM（仅提供 `update_memory` + `update_relationship` 工具）
   - 执行返回的工具调用
4. 更新 `last_overflow_summary_index += threshold`

### 5.3 与 CHAT-37 的关系

| 维度 | CHAT-37（重置时总结） | CHAT-40（溢出时总结） |
|------|----------------------|----------------------|
| 触发时机 | 会话重置后 | 消息数量达到阈值时 |
| 消息来源 | 旧 page（重置前的 page） | 当前 page |
| 消息范围 | 最近 `history_limit` 条 | `OFFSET=last_index, LIMIT=threshold` |
| 系统提示词 | `SUMMARY_SYSTEM_PROMPT` | 同一模板 |
| Agent 遍历逻辑 | 查询会话中所有 agent | 同一逻辑 |
| LLM 调用方式 | 使用 agent 自身 API 配置 | 同一方式 |
| 工具列表 | `update_memory` + `update_relationship` | 同一列表 |
| 是否占用 CD | 否 | 否 |
| 前端感知 | 否 | 否 |

---

## 6. 前端配置

### 6.1 配置位置

在**会话配置面板**（`SessionConfigPanel`）中，在 `history_limit` 输入框下方增加一个配置项。

### 6.2 UI 设计

- **标签**："溢出总结阈值"
- **输入框**：number input，min=0，max=500
- **默认值**：50
- **说明文字**："当超出历史消息限制的消息累计达到此数量时，自动触发 AI 总结。设为 0 关闭该功能。"

### 6.3 后端接口

`update_session_config` 和 `get_session_config` 已支持部分更新，只需在前端传入 `overflow_summary_threshold` 字段即可。

---

## 7. 边界情况

### 7.1 threshold = 0

当 `overflow_summary_threshold = 0` 时，跳过该会话的检测。这是关闭功能的正式方式。

### 7.2 重置会话时

`reset_session` 创建新 page 后，`last_overflow_summary_index` **不清零**。因为新 page 的消息是从 0 开始计数的，旧 page 的消息已经被归档。但如果用户希望重置后重新计数，可以手动在配置中重置 `last_overflow_summary_index`（暂不提供 UI）。

### 7.3 消息删除

如果消息被软删除，`total_messages` 会减少，但 `last_overflow_summary_index` 不会回退。这可能导致 `total_messages - last_index < threshold`，暂时不触发，直到新消息补足差额。这是可接受的（删除消息意味着信息可能已失效）。

### 7.4 多触发并发

后台扫描每 5 秒一次，如果一次总结执行时间超过 5 秒，下一次扫描会再次检测同一会话。但由于 `last_overflow_summary_index` 在总结完成后才更新，未完成的总结不会导致重复触发（因为条件仍然满足，但同一会话的多次触发应该被防止）。

**防护措施**：在 `run_overflow_summary` 开始时，检查是否已有同一会话的总结任务在运行。可以通过一个内存中的 `HashSet<String>`（运行中会话集合）来防止并发。

---

## 8. 测试要点

### 8.1 Repository 层测试

- `test_overflow_summary_fields_default`：验证新建会话时 `overflow_summary_threshold = 50`，`last_overflow_summary_index = 0`
- `test_overflow_summary_index_increments_on_trigger`：验证触发后 `last_overflow_summary_index` 正确增加

### 8.2 触发逻辑测试

- `test_overflow_trigger_when_threshold_met`：total=130, last=0, threshold=50 → 触发
- `test_overflow_no_trigger_when_below_threshold`：total=120, last=0, threshold=50 → 不触发
- `test_overflow_no_trigger_when_disabled`：threshold=0 → 不触发

### 8.3 消息查询测试

- `test_overflow_message_range`：验证 SQL 查询返回正确的 OFFSET/LIMIT 范围

---

## 9. 变更清单

| 文件 | 变更类型 | 说明 |
|------|----------|------|
| `src/db/schema.rs` | 新增 | MIGRATION_V14 |
| `src/db/migration.rs` | 修改 | 注册 V14 |
| `src/models/session.rs` | 修改 | SessionConfig / UpdateSessionConfigRequest 增加字段 |
| `src/db/session.rs` | 修改 | `get_session_config` / `update_session_config` 支持新字段 |
| `src/scheduler/mod.rs` | 修改 | `start_background_scan` 增加溢出检测；新增 `run_overflow_summary` |
| `src/lib/components/SessionConfigPanel.svelte` | 修改 | 增加 overflow_summary_threshold 输入框 |
| `src/lib/types.ts` | 修改 | SessionConfig 类型增加新字段 |

---

## 10. 后续需求衔接

| 需求 | 依赖 CHAT-40 的什么 | 衔接点 |
|------|-------------------|--------|
| **CHAT-41**（定时任务） | 复用后台异步调用基础设施 | `run_overflow_summary` 的模式可作为定时任务调用的参考 |
| **CHAT-42**（主动会话） | 复用后台异步调用基础设施 | 同理 |
