# 模型用量监测设计文档

## 1. 背景与目标

为 AgentStage 增加模型用量监测功能，追踪每次 LLM API 调用的 Token 消耗，支持多维度统计分析。

### 追踪指标
- **Token 数**：prompt_tokens、completion_tokens、total_tokens
- **API 调用次数**：每次成功的 LLM 调用计为 1 次

### 分析维度
1. **按模型**：每个 model_config 的总用量
2. **按角色**：每个 agent 的总用量，可下钻到该角色在各模型下的用量
3. **按会话**：每个会话的总用量，支持 3 个子维度（按角色 / 按模型 / 角色×模型矩阵）
4. **按用途**：按 trigger_type 分类统计（用户消息触发、后台扫描、定时任务、主动会话、人设生成）
5. **明细**：原始调用记录列表

### 设计原则
- 所有统计**仅**在专门的"用量监控"页面展示，不在消息气泡或其他页面显示
- 数据记录采用"混合粒度"：最细粒度记录每轮 LLM 调用，通过查询聚合出任意维度
- 只记录**成功**的 LLM 调用，失败的/重试的不记录
- 不弹出新窗口，所有下钻交互在页面内完成

---

## 2. 数据库设计

### 2.1 新增表 `llm_usage_records`

记录每轮成功的 LLM API 调用。

```sql
CREATE TABLE llm_usage_records (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    model_config_id TEXT NOT NULL,
    session_id TEXT,                       -- nullable: persona_generation 无固定会话
    trigger_type TEXT NOT NULL
        CHECK(trigger_type IN (
            'user_message',               -- 用户消息触发
            'background_scan',            -- 后台扫描触发
            'timer',                      -- 定时任务触发
            'proactive',                  -- 主动会话触发
            'persona_generation'          -- 角色人设生成
        )),
    call_round INTEGER NOT NULL DEFAULT 1, -- 第几轮 LLM 调用（1-based）
    prompt_tokens INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    message_id TEXT,                       -- 关联最终生成的消息（nullable）
    created_at INTEGER NOT NULL,

    FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE CASCADE,
    FOREIGN KEY (model_config_id) REFERENCES model_configs(id) ON DELETE CASCADE,
    FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
    FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE SET NULL
);
```

### 2.2 索引

```sql
CREATE INDEX idx_llm_usage_agent ON llm_usage_records(agent_id);
CREATE INDEX idx_llm_usage_model ON llm_usage_records(model_config_id);
CREATE INDEX idx_llm_usage_session ON llm_usage_records(session_id);
CREATE INDEX idx_llm_usage_time ON llm_usage_records(created_at);
CREATE INDEX idx_llm_usage_agent_model ON llm_usage_records(agent_id, model_config_id);
CREATE INDEX idx_llm_usage_session_agent ON llm_usage_records(session_id, agent_id);
CREATE INDEX idx_llm_usage_session_model ON llm_usage_records(session_id, model_config_id);
CREATE INDEX idx_llm_usage_trigger ON llm_usage_records(trigger_type);
```

### 2.3 设计说明

- `session_id` 为 nullable：角色人设生成（`persona_generation`）没有固定会话。
- `message_id` 为 nullable：某些触发类型可能没有对应消息（如后台扫描未成功触发、或工具调用中间轮次）。一个触发事件的所有 LLM 调用轮次共享同一个 `message_id`（即最终那条 agent 回复消息）。
- `call_round`：记录第几轮调用。例如一次 tool calling 可能先调用一轮生成工具参数（round=1），再调用一轮生成最终回复（round=2）。
- `trigger_type` 增加 `persona_generation`：人设生成也调 LLM 且消耗不小，一并追踪。

### 2.4 数据库迁移

作为 V21 迁移添加到 `src/db/migration.rs` 和 `src/db/schema.rs`：
- `schema.rs`：`MIGRATION_V21`
- `BASE_SCHEMA` 中直接包含 `llm_usage_records` 表和索引

---

## 3. 数据流设计

### 3.1 Conversation 层收集

修改 `conversation.run()`，在每轮 LLM 调用后收集 usage。

**新增结构：**

```rust
pub struct LlmCallUsage {
    pub call_round: i32,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}
```

**修改 `ConversationResult`：**

```rust
pub struct ConversationResult {
    pub final_content: Option<String>,
    pub executed_tool_calls: Vec<ExecutedToolCall>,
    pub messages: Vec<Message>,
    pub total_rounds: usize,
    pub usage_records: Vec<LlmCallUsage>,  // 新增
}
```

**收集逻辑**（在 `conversation.run()` 的每轮循环中）：

```rust
if let Some(usage_json) = &response.usage {
    let prompt = usage_json["prompt_tokens"].as_i64().unwrap_or(0) as i32;
    let completion = usage_json["completion_tokens"].as_i64().unwrap_or(0) as i32;
    let total = usage_json["total_tokens"].as_i64().unwrap_or(0) as i32;
    usage_records.push(LlmCallUsage {
        call_round: (round + 1) as i32,
        prompt_tokens: prompt,
        completion_tokens: completion,
        total_tokens: total,
    });
}
```

### 3.2 Scheduler 触发路径写入

所有走 `conversation.run()` 的触发路径统一处理。

**`trigger_agent_inner` / `trigger_special`（Timer & Proactive）流程：**

```
conversation.run(...) 
  → 返回 ConversationResult（含 usage_records）
  → 将 final_content 插入 messages 表 → 得到 message_id
  → 遍历 usage_records，逐条写入 llm_usage_records：
      agent_id = 当前 agent
      model_config_id = agent 当前绑定的 model_configs.id
      session_id = 当前会话
      trigger_type = user_message / background_scan / timer / proactive
      message_id = 上一步得到的 message_id
```

**关键点：**
- 一个触发事件的所有 LLM 调用轮次，共享同一个 `message_id`。
- 如果 `final_content` 为空（如 max_rounds 耗尽），则 `message_id` 为 null。
- 失败调用不记录（`conversation.run()` 内部重试失败后会 `return Err(...)`，调用方不会执行写入）。

### 3.3 Persona Generation 写入

人设生成不走 `conversation.run()`，直接调用 `provider.chat()`：

```
Step1: provider.chat(...) → 得到 LlmResponse → 提取 usage → 写入 llm_usage_records
Step2: provider.chat(...) → 得到 LlmResponse → 提取 usage → 写入 llm_usage_records
```

字段赋值：
- `agent_id` = 正在生成的角色 ID
- `model_config_id` = 生成时使用的配置
- `session_id` = null
- `trigger_type` = `persona_generation`
- `message_id` = null
- `call_round` = 1（Step1）/ 2（Step2）

---

## 4. Rust 后端变更

### 4.1 修改文件列表

| 文件 | 变更内容 |
|------|---------|
| `src/llm/conversation.rs` | `ConversationResult` 新增 `usage_records`；每轮调用后收集 usage |
| `src/llm/tool.rs` | 新增 `LlmCallUsage` 结构体 |
| `src/scheduler/mod.rs` | 所有调用 `conversation.run()` 的地方收集 usage 并写入 DB；`trigger_special` 和 `trigger_agent_inner` 新增写入逻辑 |
| `src/llm/persona_generation.rs` | Step1/Step2 调用后提取 usage 写入 DB |
| `src/db/schema.rs` | 新增 `llm_usage_records` 表和索引到 `BASE_SCHEMA`；新增 `MIGRATION_V21` |
| `src/db/migration.rs` | 注册 V21 迁移 |
| `src/db/usage.rs` | **新增**：`llm_usage_records` 的 Repository 层（插入 + 各维度查询） |
| `src/models/usage.rs` | **新增**：Usage 相关的 DTO 结构体 |
| `src/commands/usage.rs` | **新增**：Tauri Commands（各维度查询接口） |
| `src/lib.rs` | 注册新的 Command handlers |

### 4.2 新增 Repository 方法（`src/db/usage.rs`）

```rust
pub async fn insert_usage_record(conn: &DbState, record: &LlmUsageRecord) -> Result<(), String>
pub async fn get_usage_overview(conn: &DbState, time_range: TimeRange) -> Result<UsageOverview, String>
pub async fn get_usage_by_model(conn: &DbState, time_range: TimeRange) -> Result<Vec<ModelUsageItem>, String>
pub async fn get_usage_by_agent(conn: &DbState, time_range: TimeRange) -> Result<Vec<AgentUsageItem>, String>
pub async fn get_agent_model_breakdown(conn: &DbState, agent_id: &str, time_range: TimeRange) -> Result<Vec<AgentModelUsageItem>, String>
pub async fn get_usage_by_session(conn: &DbState, time_range: TimeRange) -> Result<Vec<SessionUsageItem>, String>
pub async fn get_session_agent_breakdown(conn: &DbState, session_id: &str, time_range: TimeRange) -> Result<Vec<SessionAgentUsageItem>, String>
pub async fn get_session_model_breakdown(conn: &DbState, session_id: &str, time_range: TimeRange) -> Result<Vec<SessionModelUsageItem>, String>
pub async fn get_session_agent_model_breakdown(conn: &DbState, session_id: &str, time_range: TimeRange) -> Result<Vec<SessionAgentModelUsageItem>, String>
pub async fn get_usage_by_trigger(conn: &DbState, time_range: TimeRange) -> Result<Vec<TriggerUsageItem>, String>
pub async fn get_usage_records(conn: &DbState, time_range: TimeRange, page: i32, page_size: i32, filters: UsageFilters) -> Result<PaginatedUsageRecords, String>
```

### 4.3 新增 Tauri Commands（`src/commands/usage.rs`）

| Command | 参数 | 返回 |
|---------|------|------|
| `get_usage_overview` | `{ timeRange: string }` | `{ totalCalls, totalPromptTokens, totalCompletionTokens, totalTokens, dailyTrend: [{date, calls, tokens}] }` |
| `get_usage_by_model` | `{ timeRange: string }` | `ModelUsageItem[]` |
| `get_usage_by_agent` | `{ timeRange: string }` | `AgentUsageItem[]` |
| `get_agent_model_breakdown` | `{ agentId: string, timeRange: string }` | `AgentModelUsageItem[]` |
| `get_usage_by_session` | `{ timeRange: string }` | `SessionUsageItem[]` |
| `get_session_agent_breakdown` | `{ sessionId: string, timeRange: string }` | `SessionAgentUsageItem[]` |
| `get_session_model_breakdown` | `{ sessionId: string, timeRange: string }` | `SessionModelUsageItem[]` |
| `get_session_agent_model_breakdown` | `{ sessionId: string, timeRange: string }` | `SessionAgentModelUsageItem[]` |
| `get_usage_by_trigger` | `{ timeRange: string }` | `TriggerUsageItem[]` |
| `get_usage_records` | `{ timeRange: string, page: number, pageSize: number, filters? }` | `PaginatedUsageRecords` |

---

## 5. 前端设计

### 5.1 导航入口

`LeftNav.svelte` 新增第 5 个导航项（位于"历史会话"下方）：

```typescript
{ id: 'usage' as const, label: '用量监控', icon: BarChart3 }
```

`appState.currentView` 扩展为 `'agents' | 'chat' | 'history' | 'profile' | 'usage'`。

`App.svelte` 处理 `usage` view：**不显示中间面板**（类似 `profile`），主内容区占满全宽。

### 5.2 页面整体布局

```
┌─────────────────────────────────────────────────────────────┐
│  模型用量监控          [今日 ▼] [近7天 ▼] [近30天 ▼] [全部 ▼] │
├─────────────────────────────────────────────────────────────┤
│  [概览] [按模型] [按角色] [按会话] [按用途] [明细]           │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│                    ← 内容区 →                               │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**时间筛选器**：全局生效，所有子页面共享同一时间范围。选项：今日 / 近7天 / 近30天 / 本月 / 全部。

### 5.3 各维度详细设计

#### 5.3.1 概览

**统计卡片（顶部一行 4 个）**：
- 总调用次数
- 总 Prompt Tokens
- 总 Completion Tokens
- 总 Tokens

**趋势图**：折线图展示选定时间范围内**每日**的调用次数 + Token 数（双 Y 轴）。

#### 5.3.2 按模型

**主表格**（所有模型，可点击列头排序）：
| 模型名称 | 供应商 | 调用次数 | Prompt Tokens | Completion Tokens | Total Tokens |

**下钻交互**：点击某模型行 → 该行**向下展开**（Expandable Row），展示该模型下各角色的用量，无需弹窗或页面切换：
| 角色名称 | 调用次数 | Prompt Tokens | Completion Tokens | Total Tokens |

#### 5.3.3 按角色

**顶部**：角色选择下拉框（全部角色列表，默认选中第一个有数据的角色）。

**选定后展示**：
- 统计卡片：该角色的总调用次数 / Prompt / Completion / Total
- 子 Tab：[按模型分布]
  - 表格：该角色在各模型下的用量
    | 模型名称 | 调用次数 | Prompt Tokens | Completion Tokens | Total Tokens |

> 一次只展示一个角色的数据，通过下拉框切换。

#### 5.3.4 按会话

**顶部**：会话选择下拉框（全部会话列表，按名称排序，默认选中第一个有数据的会话）。

**选定后展示**：
- 统计卡片：该会话的总调用次数 / Prompt / Completion / Total

**子 Tab（4 个）**：
- **[概览]**：该会话的基础统计
- **[按角色]**：该会话中各角色的用量表格
- **[按模型]**：该会话中各模型的用量表格
- **[角色×模型]**：矩阵表格
  | 角色名称 | 模型名称 | 调用次数 | Total Tokens |

> 一次只展示一个会话的数据，通过下拉框切换。

#### 5.3.5 按用途

**饼图/环形图**：各 trigger_type 的调用次数占比 和 Token 占比。

**表格**（所有用途，可排序）：
| 用途 | 调用次数 | Prompt Tokens | Completion Tokens | Total Tokens | 占比 |

用途名称映射：
- `user_message` → 用户消息触发
- `background_scan` → 后台扫描
- `timer` → 定时任务
- `proactive` → 主动会话
- `persona_generation` → 人设生成

#### 5.3.6 明细

**分页表格**（每页 50 条，可排序）：
| 时间 | 角色 | 模型 | 会话 | 用途 | 轮次 | Prompt | Completion | Total |

支持按角色/模型/会话/用途筛选。

### 5.4 新增前端文件

| 文件 | 说明 |
|------|------|
| `src/lib/components/UsageMonitor.svelte` | 用量监控主页面容器 |
| `src/lib/components/usage/UsageOverview.svelte` | 概览子页面 |
| `src/lib/components/usage/UsageByModel.svelte` | 按模型子页面 |
| `src/lib/components/usage/UsageByAgent.svelte` | 按角色子页面 |
| `src/lib/components/usage/UsageBySession.svelte` | 按会话子页面 |
| `src/lib/components/usage/UsageByTrigger.svelte` | 按用途子页面 |
| `src/lib/components/usage/UsageDetail.svelte` | 明细子页面 |
| `src/lib/stores/usageStore.svelte.ts` | 用量数据 Store |
| `src/lib/types/usage.ts` | 用量相关 TypeScript 类型 |

---

## 6. 测试策略

### 6.1 Rust 后端测试

- **Repository 层测试**：`src/db/usage.rs` 的各查询方法，使用内存数据库验证 GROUP BY 聚合逻辑正确
- **Conversation 层测试**：`conversation.rs` 的测试中，验证 `usage_records` 正确收集（MockProvider 返回含 usage 的 LlmResponse）
- **数据流集成测试**：验证一次完整的 `trigger_agent_inner` 调用后，`llm_usage_records` 中插入了预期数量的记录

### 6.2 前端测试

- **E2E 测试**：用量监控页面各 Tab 的渲染和交互（通过 Playwright mock Tauri IPC）
- **Store 测试**：验证时间筛选器切换时，数据正确刷新

---

## 7. 注意事项

### 7.1 数据来源精度

- 依赖 LLM API 响应中的 `usage` 字段。OpenAI-compatible 格式：`prompt_tokens`、`completion_tokens`、`total_tokens`。
- 如果 provider 不返回 usage（某些自定义 provider 可能缺失），则该次调用记录为 0 tokens，但仍计为 1 次调用。
- DeepSeek 和 MiniMax 均兼容 OpenAI 格式，usage 字段通常可用。

### 7.2 历史数据

- 新功能上线后只记录未来的 LLM 调用，**不追溯已有消息**。
- 已有消息的 `generation_info` 字段不用于此功能。

### 7.3 性能考量

- 桌面应用数据量有限（每日几十到几百次调用），实时 SQL GROUP BY 聚合性能足够。
- 明细表支持分页，避免大量数据加载。
- `llm_usage_records` 表有合理的索引覆盖所有查询维度。

### 7.4 时间筛选实现

- `timeRange` 参数在后端解析为 `start_time` 和 `end_time`：
  - `today`：当日 00:00:00 至现在
  - `last_7_days`：过去 7 天（含今天）
  - `last_30_days`：过去 30 天（含今天）
  - `this_month`：本月 1 日至今
  - `all`：无时间限制

---

## 8. 实现顺序建议

1. 数据库：新增 `llm_usage_records` 表和索引（schema + migration）
2. 后端：新增 Repository 和 DTO（`db/usage.rs`、`models/usage.rs`）
3. 后端：修改 `conversation.rs` 收集 usage
4. 后端：修改 `scheduler/mod.rs` 在触发路径中写入 usage
5. 后端：修改 `persona_generation.rs` 写入 usage
6. 后端：新增 Tauri Commands（`commands/usage.rs`）
7. 后端：`lib.rs` 注册 Commands
8. 前端：新增 TypeScript 类型和 Store
9. 前端：新增 `UsageMonitor.svelte` 主页面和各子页面
10. 前端：`LeftNav.svelte` 和 `App.svelte` 新增导航
11. 测试：Rust 单元测试 + Playwright E2E
