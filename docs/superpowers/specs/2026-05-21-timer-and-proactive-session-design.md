# CHAT-41 + CHAT-42 设计文档：定时任务工具与主动会话机制

## 变更摘要

| 项目 | 变更内容 |
|------|---------|
| **Migration** | V15：新增 `scheduled_tasks` 表；扩展 `agents` 表（3 字段）；扩展 `settings` 表（2 字段） |
| **后端命令** | `list_agent_timers`, `create_timer`, `update_timer`, `delete_timer`, `toggle_timer`, `update_agent_proactive`, `update_quiet_hours` |
| **LLM 工具** | `create_timer`（角色自主创建）, `delete_timer`（角色自主删除） |
| **Scheduler** | 新增每分钟扫描任务（Timer Scan + Proactive Scan）；新增 `trigger_special` 统一触发流程 |
| **Prompt** | 新增 Layer 0.5「定时事件能力」常驻说明；定时/主动触发时追加特殊 Layer；人设后注入「等待中的定时任务」 |
| **前端** | 角色详情页新增【定时任务】标签页（CRUD + 暂停恢复）；角色配置增加主动会话开关+区间；全局设置增加安静时段 |

---

## 1. 架构总览

```
┌─────────────────────────────────────────────────────────────┐
│                        Scheduler                            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │ Background   │  │ Timer Scan   │  │ Proactive Scan   │  │
│  │ Scan (5s)    │  │ (60s)        │  │ (60s)            │  │
│  └──────────────┘  └──────┬───────┘  └────────┬─────────┘  │
│                           │                    │            │
│                           ▼                    ▼            │
│                    ┌─────────────┐     ┌─────────────┐     │
│                    │ scheduled_  │     │ proactive_  │     │
│                    │ tasks 表    │     │ timers 内存 │     │
│                    └──────┬──────┘     └──────┬──────┘     │
│                           │                    │            │
│                           └────────┬───────────┘            │
│                                    ▼                        │
│                           ┌─────────────────┐               │
│                           │ trigger_special │               │
│                           │ (无视CD + 特殊  │               │
│                           │  Prompt 注入)   │               │
│                           └─────────────────┘               │
└─────────────────────────────────────────────────────────────┘
```

**核心设计原则**：
- 复用已有的 `Scheduler` 架构，新增每分钟扫描任务
- `trigger_special` 复用 `PromptAssembler` 构建原有 Prompt 各 Layer，仅追加特殊上下文
- 两个需求共享定时器扫描基础设施，但数据存储和触发逻辑独立

---

## 2. 数据模型（Migration V15）

### 2.1 `scheduled_tasks` 表（CHAT-41）

```sql
CREATE TABLE scheduled_tasks (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    description TEXT NOT NULL,          -- 事件描述，如"提醒起床"
    task_type TEXT NOT NULL,            -- 'single' | 'recurring'
    trigger_mode TEXT,                  -- 'after_minutes' | 'datetime'（单次时必填）
    after_minutes INTEGER,              -- 多少分钟后（trigger_mode=after_minutes）
    year INTEGER,                       -- 具体时间点（trigger_mode=datetime）
    month INTEGER,
    day INTEGER,
    hour INTEGER,
    minute INTEGER,
    interval_minutes INTEGER,           -- 循环间隔分钟数（task_type=recurring）
    next_trigger_at INTEGER NOT NULL,   -- 下次触发时间戳（毫秒）
    created_at INTEGER NOT NULL,
    is_active INTEGER DEFAULT 1,        -- 0=暂停/完成, 1=活跃
    target_session_id TEXT,             -- 期望会话（可为空）
    FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE CASCADE
);

CREATE INDEX idx_scheduled_tasks_next_trigger 
ON scheduled_tasks(next_trigger_at) WHERE is_active = 1;
```

**字段说明**：
- `task_type` + `trigger_mode` 组合：
  - `single` + `after_minutes`：N 分钟后触发一次
  - `single` + `datetime`：指定年月日时分触发一次
  - `recurring` + NULL：按 `interval_minutes` 循环触发
- `target_session_id`：可为空，触发时以提示词形式给出，不做强制约束
- 单次任务触发后 `is_active = 0`（保留记录供查看历史）
- 循环任务每次触发后更新 `next_trigger_at += interval_minutes * 60 * 1000`

### 2.2 `agents` 表扩展（CHAT-42）

```sql
ALTER TABLE agents ADD COLUMN proactive_enabled INTEGER DEFAULT 0;
ALTER TABLE agents ADD COLUMN proactive_min_minutes INTEGER DEFAULT 90;
ALTER TABLE agents ADD COLUMN proactive_max_minutes INTEGER DEFAULT 180;
```

**说明**：
- `proactive_enabled`：开关，默认关闭
- `proactive_min_minutes` / `proactive_max_minutes`：触发时间区间，默认 90-180 分钟
- **不持久化 `next_proactive_at`**：纯内存计时，应用启动时初始化

### 2.3 `settings` 表扩展（CHAT-42 全局安静时段）

```sql
ALTER TABLE settings ADD COLUMN quiet_hours_start INTEGER DEFAULT 0;   -- 分钟数，0=0:00
ALTER TABLE settings ADD COLUMN quiet_hours_end INTEGER DEFAULT 480;   -- 分钟数，480=8:00
```

**说明**：
- `-1` 表示未配置（但默认值为 0 和 480，即默认开启 0:00-8:00）
- 支持跨午夜：如 `quiet_hours_start=1320` (22:00), `quiet_hours_end=480` (8:00)

### 2.4 Scheduler 内存状态

```rust
pub struct Scheduler {
    // ... 已有字段 ...
    
    // CHAT-42: agent_id -> next_proactive_at (timestamp_ms)
    proactive_timers: Arc<Mutex<HashMap<String, i64>>>,
}
```

---

## 3. CHAT-41 定时任务工具 — 详细设计

### 3.1 LLM 工具定义

角色通过 Tool Calling 自主创建/删除定时任务。

**`create_timer`**

```json
{
  "name": "create_timer",
  "description": "创建一个定时任务。你可以设定一个未来事件或循环事件，到时间后会收到一次特殊调用。支持两种方式：1. 多少分钟后触发（单次）；2. 指定具体日期时间触发（单次）；3. 按固定间隔循环触发。",
  "parameters": {
    "type": "object",
    "properties": {
      "description": {
        "type": "string",
        "description": "事件描述，如'提醒起床'、'检查每日任务'"
      },
      "task_type": {
        "type": "string",
        "enum": ["single", "recurring"],
        "description": "任务类型：single=单次，recurring=循环"
      },
      "trigger_mode": {
        "type": "string",
        "enum": ["after_minutes", "datetime"],
        "description": "单次任务的触发方式（task_type=single 时必填）"
      },
      "after_minutes": {
        "type": "integer",
        "description": "多少分钟后触发（trigger_mode=after_minutes 时必填）"
      },
      "year": { "type": "integer", "description": "年（trigger_mode=datetime 时必填）" },
      "month": { "type": "integer", "description": "月（trigger_mode=datetime 时必填）" },
      "day": { "type": "integer", "description": "日（trigger_mode=datetime 时必填）" },
      "hour": { "type": "integer", "description": "时（trigger_mode=datetime 时必填）" },
      "minute": { "type": "integer", "description": "分（trigger_mode=datetime 时必填）" },
      "interval_minutes": {
        "type": "integer",
        "description": "循环间隔分钟数（task_type=recurring 时必填），如 60=每小时, 1440=每天"
      }
    },
    "required": ["description", "task_type"]
  }
}
```

**`delete_timer`**

```json
{
  "name": "delete_timer",
  "description": "删除一个你创建的定时任务。你可以在'等待中的定时任务'中查看任务ID。",
  "parameters": {
    "type": "object",
    "properties": {
      "task_id": {
        "type": "string",
        "description": "要删除的任务ID"
      }
    },
    "required": ["task_id"]
  }
}
```

### 3.2 工具执行逻辑

**`execute_create_timer`**

1. 参数校验：
   - `task_type="single"` 时：`trigger_mode` 必填，且 `after_minutes > 0` 或 `datetime` 有效
   - `task_type="recurring"` 时：`interval_minutes > 0`
2. 计算 `next_trigger_at`：
   - `single` + `after_minutes`：`now + after_minutes * 60 * 1000`
   - `single` + `datetime`：转换为时间戳
   - `recurring`：`now + interval_minutes * 60 * 1000`
3. 生成 `task_id = uuid()`
4. 插入 `scheduled_tasks` 表
5. 返回成功信息（含 `task_id` 和格式化后的下次触发时间）

**`execute_delete_timer`**

1. 验证该 `task_id` 存在且属于当前 `agent_id`
2. `DELETE FROM scheduled_tasks WHERE id = ?`
3. 返回成功/失败

### 3.3 定时器扫描与触发

Scheduler 每分钟扫描：

```rust
async fn scan_scheduled_tasks(&self) {
    let conn = self.db_state.0.lock().await;
    let now = chrono::Utc::now().timestamp_millis();
    
    // 查询所有到期的活跃任务
    let tasks = scheduled_task_repo::get_due_tasks(&conn, now)?;
    drop(conn);
    
    for task in tasks {
        // 1. 更新任务状态（在独立连接中）
        {
            let conn = self.db_state.0.lock().await;
            if task.task_type == "single" {
                scheduled_task_repo::deactivate_task(&conn, &task.id)?;
            } else {
                let new_next = task.next_trigger_at + task.interval_minutes * 60 * 1000;
                scheduled_task_repo::update_next_trigger(&conn, &task.id, new_next)?;
            }
        }
        
        // 2. 异步触发（不阻塞扫描）
        let scheduler = self.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = scheduler.trigger_special(
                &task.agent_id,
                SpecialTriggerContext::Timer {
                    description: task.description,
                    target_session_id: task.target_session_id,
                }
            ).await {
                log::error!("[TimerTrigger] failed: {}", e);
            }
        });
    }
}
```

### 3.4 后端命令（供前端调用）

| 命令 | 功能 | 参数 |
|------|------|------|
| `list_agent_timers` | 获取角色的所有定时任务 | `agent_id` |
| `create_timer` | 用户手动创建定时任务 | `agent_id`, `description`, `task_type`, ... |
| `update_timer` | 用户编辑定时任务 | `task_id`, ...（部分更新） |
| `delete_timer` | 用户删除定时任务 | `task_id` |
| `toggle_timer` | 暂停/恢复任务 | `task_id` |

**用户手动创建 vs 角色自主创建**：
- 两者最终都写入 `scheduled_tasks` 表
- 角色自主创建通过 `create_timer` Tool
- 用户手动创建通过前端表单 + `create_timer` 命令
- 前端表单支持快捷选择：每天 / 每小时 / 自定义分钟

### 3.5 前端 UI

**角色详情页 ——【定时任务】标签页**

```
┌─────────────────────────────────────────┐
│  角色配置 │ 关系设定 │ 记忆 │ 【定时任务】 │
├─────────────────────────────────────────┤
│                                         │
│  [+ 新建定时任务]                       │
│                                         │
│  ┌─────────────────────────────────┐   │
│  │ 描述       类型      下次触发  状态 │   │
│  ├─────────────────────────────────┤   │
│  │ 提醒起床   循环(每天)  08:30   ●活跃 │   │
│  │ 检查邮件   单次      14:00   ●活跃 │   │
│  │ 每日复盘   循环(2小时) 10:00   ⏸暂停 │   │
│  ├─────────────────────────────────┤   │
│  │ [编辑] [删除] [暂停/恢复]        │   │
│  └─────────────────────────────────┘   │
│                                         │
│  注：定时任务可由角色自主创建，也可由    │
│      你手动添加、编辑或删除。            │
│                                         │
└─────────────────────────────────────────┘
```

**新建/编辑弹窗**：
- 描述输入框
- 类型选择：单次 / 循环
- 单次模式：
  - 触发方式：多少分钟后 / 指定时间
  - 多少分钟后：数字输入框（分钟）
  - 指定时间：日期时间选择器
- 循环模式：
  - 快捷选择：每天 / 每小时 / 每 30 分钟
  - 自定义：数字输入框（分钟）
- 期望会话：下拉选择（可为空）

---

## 4. CHAT-42 主动会话机制 — 详细设计

### 4.1 计时维护

**唯一维护入口**：角色**发送实际消息**时。

在 `ToolExecutor::execute_send_message` 成功后：

```rust
// 检查该角色是否启用了主动会话
if agent.proactive_enabled {
    let min_ms = agent.proactive_min_minutes * 60 * 1000;
    let max_ms = agent.proactive_max_minutes * 60 * 1000;
    let random_ms = rand::random_range(min_ms..=max_ms);
    let next_at = now + random_ms;
    
    scheduler.set_proactive_timer(&agent_id, next_at).await;
}
```

**关键规则**：
- 仅 `send_message` 成功时重置计时（总结/溢出/记忆更新都不算）
- 纯内存计时，不持久化到数据库
- 应用启动时，为所有 `proactive_enabled=true` 的角色初始化一个随机计时

### 4.2 应用启动初始化

```rust
pub async fn init_proactive_timers(&self) {
    let conn = self.db_state.0.lock().await;
    let agents = agent_repo::list_proactive_agents(&conn)?;
    drop(conn);
    
    let now = chrono::Utc::now().timestamp_millis();
    let mut timers = self.proactive_timers.lock().await;
    
    for agent in agents {
        let min_ms = agent.proactive_min_minutes * 60 * 1000;
        let max_ms = agent.proactive_max_minutes * 60 * 1000;
        let random_ms = rand::random_range(min_ms..=max_ms);
        timers.insert(agent.id, now + random_ms);
    }
}
```

### 4.3 扫描触发逻辑

Scheduler 每分钟扫描：

```rust
async fn scan_proactive_timers(&self) {
    let now = chrono::Utc::now().timestamp_millis();
    let timers = self.proactive_timers.lock().await.clone();
    
    for (agent_id, next_at) in timers {
        if next_at > now {
            continue;
        }
        
        // 1. 检查安静时段
        if self.is_in_quiet_hours().await {
            // 重新随机，跳过本次
            self.reset_proactive_timer(&agent_id).await;
            continue;
        }
        
        // 2. 触发（异步 spawn）
        let scheduler = self.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = scheduler.trigger_special(
                &agent_id,
                SpecialTriggerContext::Proactive
            ).await {
                log::error!("[ProactiveTrigger] failed: {}", e);
            }
        });
        
        // 3. 触发后重新随机（为下一次做准备）
        self.reset_proactive_timer(&agent_id).await;
    }
}
```

### 4.4 安静时段检查

```rust
async fn is_in_quiet_hours(&self) -> bool {
    let conn = self.db_state.0.lock().await;
    let settings = settings_repo::get_or_create_settings(&conn)?;
    drop(conn);
    
    if settings.quiet_hours_start < 0 || settings.quiet_hours_end < 0 {
        return false;
    }
    
    let now = chrono::Local::now();
    let current_minutes = now.hour() * 60 + now.minute();
    
    if settings.quiet_hours_start <= settings.quiet_hours_end {
        // 正常区间，如 0:00-8:00
        current_minutes >= settings.quiet_hours_start 
            && current_minutes < settings.quiet_hours_end
    } else {
        // 跨午夜区间，如 22:00-8:00
        current_minutes >= settings.quiet_hours_start 
            || current_minutes < settings.quiet_hours_end
    }
}
```

### 4.5 后端命令

| 命令 | 功能 | 参数 |
|------|------|------|
| `update_agent_proactive` | 更新角色主动会话配置 | `agent_id`, `proactive_enabled`, `proactive_min_minutes`, `proactive_max_minutes` |
| `update_quiet_hours` | 更新全局安静时段 | `quiet_hours_start`, `quiet_hours_end` |
| `get_quiet_hours` | 获取全局安静时段 | - |

### 4.6 前端 UI

**角色详情页 ——【角色配置】扩展**

```
主动会话机制
━━━━━━━━━━━━━━━━━━━━
[✓] 启用主动会话

  触发时间区间（分钟）
  ┌─────────┐ ~ ┌─────────┐
  │  90     │   │  180    │
  └─────────┘   └─────────┘
  角色每次发消息后，会在此区间内随机
  一个时间，若期间未再发言则触发一次。

━━━━━━━━━━━━━━━━━━━━
```

**全局设置：左下角齿轮 —— 安静时段**

```
安静时段设置
━━━━━━━━━━━━━━━━━━━━
[✓] 启用安静时段

  不触发时间段
  ┌─────────┐ ~ ┌─────────┐
  │  00:00  │   │  08:00  │
  └─────────┘   └─────────┘
  在此期间，所有主动会话和定时任务
  均不会触发（到达后顺延）。

━━━━━━━━━━━━━━━━━━━━
```

---

## 5. 统一触发流程 `trigger_special`

### 5.1 设计目标

定时任务和主动会话共享统一的特殊触发流程：
- 无视全局最小触发间隔（CD）
- 不依赖未读消息队列
- 复用 `PromptAssembler` 构建原有 Prompt 结构
- 仅追加特殊上下文 Layer

### 5.2 流程

```rust
enum SpecialTriggerContext {
    Timer {
        description: String,
        target_session_id: Option<String>,
    },
    Proactive,
}

async fn trigger_special(
    &self,
    agent_id: &str,
    context: SpecialTriggerContext,
) -> Result<(), String> {
    // 1. 设置 is_triggering（防止并发）
    self.set_triggering_flag(agent_id).await?;
    
    // 2. 获取 agent 配置和模型
    let conn = self.db_state.0.lock().await;
    let agent = agent_repo::get_agent(&conn, agent_id)?;
    let provider = OpenAiCompatibleProvider::from_agent(&agent)?;
    drop(conn);
    
    // 3. 构建 Prompt（复用 PromptAssembler）
    // 注意：没有未读消息，所以不传入 pending messages
    let mut assembler = PromptAssembler::new(self.db_state.clone(), agent_id);
    let system_prompt = assembler.build_system_prompt().await?;
    let special_layer = match &context {
        SpecialTriggerContext::Timer { description, target_session_id } => {
            format!("【定时任务触发】\n本次调用由定时任务发起。\n定时事件：{}\n{}",
                description,
                target_session_id.as_ref()
                    .map(|id| format!("你之前期望在指定会话中处理此事。"))
                    .unwrap_or_default()
            )
        }
        SpecialTriggerContext::Proactive => {
            "【主动会话触发】\n本次调用由主动会话机制触发。\n你可以选择一个会话开始话题、延续之前的话题，或保持沉默。如果决定发起话题，请使用 send_message 工具；如果保持沉默，无需操作。".to_string()
        }
    };
    
    // 4. 组装完整 Prompt
    let full_prompt = format!("{}\n\n{}", system_prompt, special_layer);
    
    // 5. 调用 LLM
    let messages = vec![]; // 无 user message，纯 system prompt
    let response = Self::call_llm(&provider, &full_prompt, messages).await?;
    
    // 6. 执行工具调用（复用 ToolExecutor）
    let executor = ToolExecutor::new(self.db_state.clone(), self.clone());
    for tool_call in response.tool_calls {
        executor.execute(&tool_call).await?;
    }
    
    // 7. 清除 is_triggering
    self.clear_triggering_flag(agent_id).await?;
    
    Ok(())
}
```

### 5.3 与正常触发的区别

| 特性 | 正常触发 (`trigger_agent`) | 特殊触发 (`trigger_special`) |
|------|--------------------------|----------------------------|
| CD 检查 | ✅ 检查 `global_min_trigger_interval` | ❌ 无视 CD |
| 未读消息 | ✅ 基于 `pending_messages` | ❌ 无未读消息 |
| Prompt 结构 | System + 人设 + 关系 + 历史 + 最新消息 | System + 人设 + 关系 + 历史 + **特殊 Layer** |
| 会话选择 | 由未读消息隐含确定 | 角色通过 `send_message` 自选 |
| 允许沉默 | ❌ 必须处理未读消息 | ✅ 可以不发送消息 |

---

## 6. Prompt 模板设计

### 6.1 常驻 Layer 0.5：定时事件能力

在 System Prompt 中增加常驻说明（所有调用都包含）：

```text
【定时事件能力】
你拥有设定定时事件的能力：
- 当你需要记住某个未来的约定、事件或提醒时，可以使用 create_timer 工具设定一个定时任务。
- 支持单次触发（指定时间或多少分钟后）和循环触发（按固定间隔重复）。
- 到时间后，你会收到一次特殊调用，Prompt 中会标注【定时任务触发】及事件内容。
- 你可以在【等待中的定时任务】中查看当前已设定但未触发的任务。
```

### 6.2 定时任务触发时追加

```text
【定时任务触发】
本次调用由定时任务发起。
定时事件：{description}
{target_session_id 提示：你之前期望在指定会话中处理此事。}
```

### 6.3 主动会话触发时追加

```text
【主动会话触发】
本次调用由主动会话机制触发。
你可以选择一个会话开始话题、延续之前的话题，或保持沉默。
如果决定发起话题，请使用 send_message 工具；如果保持沉默，无需操作。
```

### 6.4 【等待中的定时任务】（人设之后注入）

在 PromptAssembler 组装时，查询该角色的活跃定时任务，在人设 Layer 之后注入：

```text
【等待中的定时任务】
{task_id}: {description}（下次触发：{time}）
{task_id}: {description}（下次触发：{time}）
```

- 如果没有等待中的定时任务，此节不显示
- 位置：人设 Layer 之后，关系/记忆 Layer 之前

---

## 7. 边界情况与错误处理

### 7.1 CHAT-41 边界情况

| 场景 | 处理 |
|------|------|
| 角色删除 | `ON DELETE CASCADE` 自动删除关联任务 |
| 任务到期时角色已被删除 | 扫描时跳过，更新任务状态为 `is_active=0` |
| `datetime` 设定过去的时间 | 工具执行时校验，返回错误 |
| 应用重启时任务已过期 | 启动扫描会立即触发，然后更新/删除 |
| 循环任务间隔过大 | 无上限限制，用户/角色自行负责 |
| 并发触发同一任务 | `is_active` 更新 + 异步 spawn，可能有重复触发风险（低概率，可接受） |

### 7.2 CHAT-42 边界情况

| 场景 | 处理 |
|------|------|
| 应用重启 | 启动时重新初始化随机计时 |
| 角色被删除 | 内存中计时自然消失 |
| 角色在安静时段内一直不发消息 | 每次触发检查安静时段，在时段内则顺延 |
| 角色发送消息后立刻被触发 | 发送消息会重置计时，所以不会立刻触发 |
| 禁言会话 | 角色通过 `send_message` 工具选择会话时，如果目标会话被禁言，工具执行会失败（现有逻辑） |
| 角色无活跃会话 | `trigger_special` 中会话列表为空，角色可能选择沉默 |

---

## 8. 测试策略

### 8.1 CHAT-41 测试

| 测试项 | 类型 |
|--------|------|
| `scheduled_tasks` CRUD（Repo 层） | Rust 单元测试 |
| `create_timer` Tool 执行（after_minutes / datetime / recurring） | Rust 单元测试 |
| `delete_timer` Tool 执行（验证归属权） | Rust 单元测试 |
| 定时器扫描：到期任务触发 | Rust 集成测试（mock 时间） |
| 单次任务触发后 `is_active=0` | Rust 集成测试 |
| 循环任务触发后 `next_trigger_at` 更新 | Rust 集成测试 |
| 前端：列表展示、新建、编辑、删除、暂停恢复 | E2E / 手动测试 |

### 8.2 CHAT-42 测试

| 测试项 | 类型 |
|--------|------|
| 发送消息后计时重置 | Rust 单元测试 |
| 安静时段检查（正常区间 + 跨午夜） | Rust 单元测试 |
| 启动时初始化随机计时 | Rust 集成测试 |
| 触发后重新随机 | Rust 集成测试 |
| 安静时段内触发顺延 | Rust 集成测试 |
| 前端：开关、区间配置、安静时段设置 | E2E / 手动测试 |

---

## 9. 依赖与复用

| 需求 | 复用/依赖 |
|------|----------|
| **CHAT-41** | 复用 `PromptAssembler`、`OpenAiCompatibleProvider`、`ToolExecutor`、`trigger_special` |
| **CHAT-42** | 复用 `PromptAssembler`、`OpenAiCompatibleProvider`、`ToolExecutor`、`trigger_special`、安静时段检查 |
| **CHAT-38**（禁言优化） | 定时任务和主动会话触发时，若角色调用 `send_message` 到禁言会话，工具层会自然失败（现有逻辑） |

---

*文档版本：V1.0*  
*编写日期：2026-05-21*  
*状态：待实现*
