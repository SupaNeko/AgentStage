# CHAT-41 + CHAT-42 实现计划：定时任务工具与主动会话机制

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development

**Goal:** 实现 CHAT-41（角色定时任务工具）和 CHAT-42（主动会话机制），包括数据库层、LLM 工具层、Scheduler 定时扫描、后端命令、Prompt 模板和前端 UI。

**Architecture:** 复用现有 Scheduler 架构，新增每分钟扫描任务。`trigger_special` 统一处理定时/主动触发，复用 `PromptAssembler` 构建 Prompt 后追加特殊上下文 Layer。

**Tech Stack:** Rust + SQLite (rusqlite), Tauri v2, Svelte 5, TailwindCSS v4, Tokio

---

## 文件结构总览

| 文件 | 责任 |
|------|------|
| `src-tauri/src/db/schema.rs` | Migration V15 |
| `src-tauri/src/db/scheduled_task.rs` | 定时任务 Repository（新建） |
| `src-tauri/src/db/agent.rs` | 扩展：proactive 字段 CRUD |
| `src-tauri/src/db/settings.rs` | 扩展：quiet_hours CRUD |
| `src-tauri/src/models/scheduled_task.rs` | ScheduledTask 模型（新建） |
| `src-tauri/src/models/agent.rs` | 扩展：proactive 字段 |
| `src-tauri/src/models/settings.rs` | 扩展：quiet_hours 字段 |
| `src-tauri/src/llm/tool.rs` | 新增 create_timer / delete_timer |
| `src-tauri/src/llm/prompt_templates.rs` | 新增模板常量 |
| `src-tauri/src/llm/prompt.rs` | 注入【等待中的定时任务】 |
| `src-tauri/src/scheduler/mod.rs` | trigger_special + 扫描逻辑 |
| `src-tauri/src/commands/timer.rs` | 后端命令（新建） |
| `src-tauri/src/lib.rs` | 注册命令 |
| `src/lib/types.ts` | TypeScript 类型扩展 |
| `src/lib/components/AgentTimerPanel.svelte` | 定时任务标签页（新建） |
| `src/lib/components/TimerEditModal.svelte` | 新建/编辑弹窗（新建） |
| `src/lib/components/AgentDetail.svelte` | 新增标签页入口 |
| `src/lib/components/SettingsPanel.svelte` | 安静时段配置 |

---

## Task 1: Migration V15 + 数据模型

**Files:**
- Modify: `src-tauri/src/db/schema.rs`
- Modify: `src-tauri/src/db/mod.rs`
- Modify: `src-tauri/src/models/mod.rs`
- Create: `src-tauri/src/models/scheduled_task.rs`
- Modify: `src-tauri/src/models/agent.rs`
- Modify: `src-tauri/src/models/settings.rs`

- [ ] **Step 1: Write Migration V15 in schema.rs**

在 `schema.rs` 末尾添加：

```rust
pub const MIGRATION_V15: &str = r#"
-- CHAT-41: Scheduled tasks
CREATE TABLE scheduled_tasks (
    id TEXT PRIMARY KEY,
    agent_id TEXT NOT NULL,
    description TEXT NOT NULL,
    task_type TEXT NOT NULL,
    trigger_mode TEXT,
    after_minutes INTEGER,
    year INTEGER,
    month INTEGER,
    day INTEGER,
    hour INTEGER,
    minute INTEGER,
    interval_minutes INTEGER,
    next_trigger_at INTEGER NOT NULL,
    created_at INTEGER NOT NULL,
    is_active INTEGER DEFAULT 1,
    target_session_id TEXT,
    FOREIGN KEY (agent_id) REFERENCES agents(id) ON DELETE CASCADE
);

CREATE INDEX idx_scheduled_tasks_next_trigger 
ON scheduled_tasks(next_trigger_at) WHERE is_active = 1;

-- CHAT-42: Proactive session config on agents
ALTER TABLE agents ADD COLUMN proactive_enabled INTEGER DEFAULT 0;
ALTER TABLE agents ADD COLUMN proactive_min_minutes INTEGER DEFAULT 90;
ALTER TABLE agents ADD COLUMN proactive_max_minutes INTEGER DEFAULT 180;

-- CHAT-42: Quiet hours in settings
ALTER TABLE settings ADD COLUMN quiet_hours_start INTEGER DEFAULT 0;
ALTER TABLE settings ADD COLUMN quiet_hours_end INTEGER DEFAULT 480;
"#;
```

更新 `run_migrations` 函数，在 V14 后添加 V15。

- [ ] **Step 2: Add ScheduledTask model**

创建 `src-tauri/src/models/scheduled_task.rs`：

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub id: String,
    pub agent_id: String,
    pub description: String,
    pub task_type: String, // "single" | "recurring"
    pub trigger_mode: Option<String>, // "after_minutes" | "datetime"
    pub after_minutes: Option<i32>,
    pub year: Option<i32>,
    pub month: Option<i32>,
    pub day: Option<i32>,
    pub hour: Option<i32>,
    pub minute: Option<i32>,
    pub interval_minutes: Option<i32>,
    pub next_trigger_at: i64,
    pub created_at: i64,
    pub is_active: i32,
    pub target_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTimerRequest {
    pub description: String,
    pub task_type: String,
    pub trigger_mode: Option<String>,
    pub after_minutes: Option<i32>,
    pub year: Option<i32>,
    pub month: Option<i32>,
    pub day: Option<i32>,
    pub hour: Option<i32>,
    pub minute: Option<i32>,
    pub interval_minutes: Option<i32>,
    pub target_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTimerRequest {
    pub id: String,
    pub description: Option<String>,
    pub task_type: Option<String>,
    pub trigger_mode: Option<String>,
    pub after_minutes: Option<i32>,
    pub year: Option<i32>,
    pub month: Option<i32>,
    pub day: Option<i32>,
    pub hour: Option<i32>,
    pub minute: Option<i32>,
    pub interval_minutes: Option<i32>,
    pub next_trigger_at: Option<i64>,
    pub target_session_id: Option<String>,
}
```

- [ ] **Step 3: Extend Agent model**

在 `src-tauri/src/models/agent.rs` 的 `Agent` struct 中添加：

```rust
pub proactive_enabled: i32,
pub proactive_min_minutes: i32,
pub proactive_max_minutes: i32,
```

- [ ] **Step 4: Extend Settings model**

在 `src-tauri/src/models/settings.rs` 的 `Settings` struct 中添加：

```rust
pub quiet_hours_start: i32,
pub quiet_hours_end: i32,
```

- [ ] **Step 5: Update mod.rs exports**

在 `src-tauri/src/db/mod.rs` 添加：
```rust
pub mod scheduled_task;
```

在 `src-tauri/src/models/mod.rs` 添加：
```rust
pub mod scheduled_task;
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/db/schema.rs src-tauri/src/db/mod.rs src-tauri/src/models/mod.rs src-tauri/src/models/scheduled_task.rs src-tauri/src/models/agent.rs src-tauri/src/models/settings.rs
git commit -m "feat(db): Migration V15 - scheduled_tasks, proactive config, quiet_hours"
```

---

## Task 2: scheduled_tasks Repository + 测试

**Files:**
- Create: `src-tauri/src/db/scheduled_task.rs`

- [ ] **Step 1: Create repository with CRUD**

```rust
use rusqlite::{Connection, params};
use crate::models::scheduled_task::{ScheduledTask, CreateTimerRequest};
use uuid::Uuid;

pub fn insert_task(conn: &Connection, req: &CreateTimerRequest, agent_id: &str) -> Result<String, rusqlite::Error> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO scheduled_tasks (id, agent_id, description, task_type, trigger_mode, after_minutes, year, month, day, hour, minute, interval_minutes, next_trigger_at, created_at, is_active, target_session_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, 1, ?15)",
        params![
            &id, agent_id, &req.description, &req.task_type, &req.trigger_mode,
            req.after_minutes, req.year, req.month, req.day, req.hour, req.minute,
            req.interval_minutes, req.next_trigger_at, now, &req.target_session_id
        ],
    )?;
    Ok(id)
}

pub fn list_by_agent(conn: &Connection, agent_id: &str) -> Result<Vec<ScheduledTask>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, agent_id, description, task_type, trigger_mode, after_minutes, year, month, day, hour, minute, interval_minutes, next_trigger_at, created_at, is_active, target_session_id
         FROM scheduled_tasks WHERE agent_id = ?1 ORDER BY next_trigger_at ASC"
    )?;
    let rows = stmt.query_map([agent_id], |row| {
        Ok(ScheduledTask {
            id: row.get(0)?,
            agent_id: row.get(1)?,
            description: row.get(2)?,
            task_type: row.get(3)?,
            trigger_mode: row.get(4)?,
            after_minutes: row.get(5)?,
            year: row.get(6)?,
            month: row.get(7)?,
            day: row.get(8)?,
            hour: row.get(9)?,
            minute: row.get(10)?,
            interval_minutes: row.get(11)?,
            next_trigger_at: row.get(12)?,
            created_at: row.get(13)?,
            is_active: row.get(14)?,
            target_session_id: row.get(15)?,
        })
    })?;
    rows.collect()
}

pub fn get_due_tasks(conn: &Connection, now: i64) -> Result<Vec<ScheduledTask>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, agent_id, description, task_type, trigger_mode, after_minutes, year, month, day, hour, minute, interval_minutes, next_trigger_at, created_at, is_active, target_session_id
         FROM scheduled_tasks WHERE is_active = 1 AND next_trigger_at <= ?1 ORDER BY next_trigger_at ASC"
    )?;
    let rows = stmt.query_map([now], |row| {
        Ok(ScheduledTask {
            id: row.get(0)?,
            agent_id: row.get(1)?,
            description: row.get(2)?,
            task_type: row.get(3)?,
            trigger_mode: row.get(4)?,
            after_minutes: row.get(5)?,
            year: row.get(6)?,
            month: row.get(7)?,
            day: row.get(8)?,
            hour: row.get(9)?,
            minute: row.get(10)?,
            interval_minutes: row.get(11)?,
            next_trigger_at: row.get(12)?,
            created_at: row.get(13)?,
            is_active: row.get(14)?,
            target_session_id: row.get(15)?,
        })
    })?;
    rows.collect()
}

pub fn deactivate_task(conn: &Connection, task_id: &str) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE scheduled_tasks SET is_active = 0 WHERE id = ?1",
        [task_id],
    )?;
    Ok(())
}

pub fn update_next_trigger(conn: &Connection, task_id: &str, next_trigger_at: i64) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE scheduled_tasks SET next_trigger_at = ?1 WHERE id = ?2",
        params![next_trigger_at, task_id],
    )?;
    Ok(())
}

pub fn delete_task(conn: &Connection, task_id: &str) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM scheduled_tasks WHERE id = ?1", [task_id])?;
    Ok(())
}

pub fn toggle_task(conn: &Connection, task_id: &str, is_active: i32) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE scheduled_tasks SET is_active = ?1 WHERE id = ?2",
        params![is_active, task_id],
    )?;
    Ok(())
}

pub fn update_task(conn: &Connection, task_id: &str, description: Option<&str>, next_trigger_at: Option<i64>, target_session_id: Option<&str>) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE scheduled_tasks SET description = COALESCE(?1, description), next_trigger_at = COALESCE(?2, next_trigger_at), target_session_id = COALESCE(?3, target_session_id) WHERE id = ?4",
        params![description, next_trigger_at, target_session_id, task_id],
    )?;
    Ok(())
}
```

- [ ] **Step 2: Write repository tests**

在 `src-tauri/src/db/scheduled_task.rs` 底部添加 `#[cfg(test)]` 模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::connection::Connection;
    use crate::db::schema::run_migrations;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();
        conn
    }

    #[test]
    fn test_insert_and_list() {
        let conn = setup();
        let req = CreateTimerRequest {
            description: "提醒起床".to_string(),
            task_type: "recurring".to_string(),
            trigger_mode: None,
            after_minutes: None,
            year: None,
            month: None,
            day: None,
            hour: None,
            minute: None,
            interval_minutes: Some(1440),
            target_session_id: None,
        };
        // 需要先插入 agent
        conn.execute("INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES ('a1', 'Test', '', '', 0, 0)", []).unwrap();
        let id = insert_task(&conn, &req, "a1").unwrap();
        let tasks = list_by_agent(&conn, "a1").unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].description, "提醒起床");
    }

    #[test]
    fn test_get_due_tasks() {
        let conn = setup();
        conn.execute("INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES ('a1', 'Test', '', '', 0, 0)", []).unwrap();
        let req = CreateTimerRequest {
            description: "test".to_string(),
            task_type: "single".to_string(),
            trigger_mode: Some("after_minutes".to_string()),
            after_minutes: Some(1),
            year: None, month: None, day: None, hour: None, minute: None,
            interval_minutes: None,
            target_session_id: None,
        };
        let id = insert_task(&conn, &req, "a1").unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        let due = get_due_tasks(&conn, now + 120000).unwrap();
        assert_eq!(due.len(), 1);
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cd src-tauri
cargo test scheduled_task --no-run
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db/scheduled_task.rs
git commit -m "feat(db): add scheduled_tasks repository with CRUD and tests"
```

---

## Task 3: 扩展 Agent / Settings Repository

**Files:**
- Modify: `src-tauri/src/db/agent.rs`
- Modify: `src-tauri/src/db/settings.rs`

- [ ] **Step 1: Extend agent repository**

在 `src-tauri/src/db/agent.rs` 中添加/修改：

```rust
pub fn update_proactive_config(
    conn: &Connection,
    agent_id: &str,
    enabled: i32,
    min_minutes: i32,
    max_minutes: i32,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE agents SET proactive_enabled = ?1, proactive_min_minutes = ?2, proactive_max_minutes = ?3, updated_at = ?4 WHERE id = ?5",
        params![enabled, min_minutes, max_minutes, chrono::Utc::now().timestamp_millis(), agent_id],
    )?;
    Ok(())
}
```

更新 `get_agent` 和 list 查询，包含新的 3 个字段。

- [ ] **Step 2: Extend settings repository**

在 `src-tauri/src/db/settings.rs` 中添加：

```rust
pub fn update_quiet_hours(
    conn: &Connection,
    start: i32,
    end: i32,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE settings SET quiet_hours_start = ?1, quiet_hours_end = ?2",
        params![start, end],
    )?;
    Ok(())
}
```

更新 `get_or_create_settings` 查询，包含新的 2 个字段。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/db/agent.rs src-tauri/src/db/settings.rs
git commit -m "feat(db): extend agent and settings repo for proactive/quiet_hours"
```

---

## Task 4: LLM 工具（create_timer / delete_timer）

**Files:**
- Modify: `src-tauri/src/llm/tool.rs`

- [ ] **Step 1: Add tool schemas**

在 `tool.rs` 的 `get_all_tool_schemas` 函数中添加：

```rust
pub fn create_timer_tool_schema() -> serde_json::Value {
    serde_json::json!({
        "name": "create_timer",
        "description": "创建一个定时任务。你可以设定一个未来事件或循环事件，到时间后会收到一次特殊调用。支持两种方式：1. 多少分钟后触发（单次）；2. 指定具体日期时间触发（单次）；3. 按固定间隔循环触发。",
        "parameters": {
            "type": "object",
            "properties": {
                "description": { "type": "string", "description": "事件描述，如'提醒起床'" },
                "task_type": { "type": "string", "enum": ["single", "recurring"] },
                "trigger_mode": { "type": "string", "enum": ["after_minutes", "datetime"] },
                "after_minutes": { "type": "integer" },
                "year": { "type": "integer" },
                "month": { "type": "integer" },
                "day": { "type": "integer" },
                "hour": { "type": "integer" },
                "minute": { "type": "integer" },
                "interval_minutes": { "type": "integer", "description": "循环间隔分钟数" }
            },
            "required": ["description", "task_type"]
        }
    })
}

pub fn delete_timer_tool_schema() -> serde_json::Value {
    serde_json::json!({
        "name": "delete_timer",
        "description": "删除一个你创建的定时任务。你可以在'等待中的定时任务'中查看任务ID。",
        "parameters": {
            "type": "object",
            "properties": {
                "task_id": { "type": "string" }
            },
            "required": ["task_id"]
        }
    })
}
```

更新 `get_all_tool_schemas` 返回的数组，加入这两个 schema。

- [ ] **Step 2: Add tool execution in ToolExecutor**

在 `ToolExecutor` 中添加：

```rust
async fn execute_create_timer(&self, args: &serde_json::Value) -> Result<String, String> {
    let agent_id = /* 从上下文中获取当前 agent_id */;
    let description = args["description"].as_str().ok_or("missing description")?;
    let task_type = args["task_type"].as_str().ok_or("missing task_type")?;
    
    let now = chrono::Utc::now().timestamp_millis();
    let next_trigger_at = if task_type == "single" {
        let trigger_mode = args["trigger_mode"].as_str().ok_or("missing trigger_mode for single task")?;
        if trigger_mode == "after_minutes" {
            let minutes = args["after_minutes"].as_i64().ok_or("missing after_minutes")?;
            now + minutes * 60 * 1000
        } else if trigger_mode == "datetime" {
            let year = args["year"].as_i32().ok_or("missing year")?;
            let month = args["month"].as_u32().ok_or("missing month")?;
            let day = args["day"].as_u32().ok_or("missing day")?;
            let hour = args["hour"].as_u32().ok_or("missing hour")?;
            let minute = args["minute"].as_u32().ok_or("missing minute")?;
            let dt = chrono::Local.with_ymd_and_hms(year, month, day, hour, minute, 0)
                .single().ok_or("invalid datetime")?;
            dt.timestamp_millis()
        } else {
            return Err("invalid trigger_mode".to_string());
        }
    } else if task_type == "recurring" {
        let interval = args["interval_minutes"].as_i64().ok_or("missing interval_minutes")?;
        if interval <= 0 { return Err("interval_minutes must be > 0".to_string()); }
        now + interval * 60 * 1000
    } else {
        return Err("invalid task_type".to_string());
    };
    
    let req = CreateTimerRequest {
        description: description.to_string(),
        task_type: task_type.to_string(),
        trigger_mode: args["trigger_mode"].as_str().map(|s| s.to_string()),
        after_minutes: args["after_minutes"].as_i64().map(|v| v as i32),
        year: args["year"].as_i64().map(|v| v as i32),
        month: args["month"].as_i64().map(|v| v as i32),
        day: args["day"].as_i64().map(|v| v as i32),
        hour: args["hour"].as_i64().map(|v| v as i32),
        minute: args["minute"].as_i64().map(|v| v as i32),
        interval_minutes: args["interval_minutes"].as_i64().map(|v| v as i32),
        target_session_id: None,
    };
    
    let conn = self.db_state.0.lock().await;
    let task_id = scheduled_task_repo::insert_task(&conn, &req, agent_id)
        .map_err(|e| e.to_string())?;
    
    Ok(format!("定时任务创建成功，任务ID: {}，下次触发时间: {}", task_id, 
        chrono::Local::timestamp_millis(next_trigger_at).format("%Y-%m-%d %H:%M")))
}

async fn execute_delete_timer(&self, args: &serde_json::Value) -> Result<String, String> {
    let agent_id = /* 从上下文中获取 */;
    let task_id = args["task_id"].as_str().ok_or("missing task_id")?;
    
    let conn = self.db_state.0.lock().await;
    let tasks = scheduled_task_repo::list_by_agent(&conn, agent_id)
        .map_err(|e| e.to_string())?;
    if !tasks.iter().any(|t| t.id == task_id) {
        return Err("任务不存在或不属于你".to_string());
    }
    
    scheduled_task_repo::delete_task(&conn, task_id)
        .map_err(|e| e.to_string())?;
    Ok("定时任务已删除".to_string())
}
```

注意：`ToolExecutor` 需要获取当前 `agent_id`。查看现有代码，`execute_send_message` 等方法如何获取 `agent_id`？需要在 `ToolExecutor` 的调用上下文中传递。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/llm/tool.rs
git commit -m "feat(tool): add create_timer and delete_timer LLM tools"
```

---

## Task 5: Scheduler - trigger_special + 扫描逻辑

**Files:**
- Modify: `src-tauri/src/scheduler/mod.rs`

- [ ] **Step 1: Add SpecialTriggerContext enum and proactive_timers**

在 `Scheduler` struct 中添加：

```rust
pub struct Scheduler {
    // ... 已有字段 ...
    proactive_timers: Arc<Mutex<HashMap<String, i64>>>,
}

#[derive(Clone)]
pub enum SpecialTriggerContext {
    Timer {
        description: String,
        target_session_id: Option<String>,
    },
    Proactive,
}
```

更新 `Scheduler::new` 初始化 `proactive_timers`。

- [ ] **Step 2: Implement trigger_special**

```rust
pub async fn trigger_special(
    &self,
    agent_id: &str,
    context: SpecialTriggerContext,
) -> Result<(), String> {
    // 1. 设置 is_triggering
    self.set_triggering_flag(agent_id).await?;
    
    // 2. 获取 agent 配置和 provider
    let conn = self.db_state.0.lock().await;
    let agent = agent_repo::get_agent(&conn, agent_id).map_err(|e| e.to_string())?;
    let provider = OpenAiCompatibleProvider::from_agent(&agent)?;
    drop(conn);
    
    // 3. 构建 Prompt（复用 PromptAssembler，无特定会话）
    let conn = self.db_state.0.lock().await;
    let base_prompt = PromptAssembler::assemble(
        &conn, agent_id, None, None, &[], &std::collections::HashSet::new()
    ).map_err(|e| e.to_string())?;
    drop(conn);
    
    // 4. 追加特殊上下文
    let special_layer = match &context {
        SpecialTriggerContext::Timer { description, target_session_id } => {
            let mut s = format!("【定时任务触发】\n本次调用由定时任务发起。\n定时事件：{}", description);
            if let Some(sid) = target_session_id {
                s.push_str(&format!("\n你之前期望在指定会话中处理此事。"));
            }
            s
        }
        SpecialTriggerContext::Proactive => {
            "【主动会话触发】\n本次调用由主动会话机制触发。\n你可以选择一个会话开始话题、延续之前的话题，或保持沉默。如果决定发起话题，请使用 send_message 工具；如果保持沉默，无需操作。".to_string()
        }
    };
    
    let full_prompt = format!("{}\n\n{}", base_prompt, special_layer);
    
    // 5. 调用 LLM
    let messages = vec![];
    let response = Self::call_llm(&provider, &full_prompt, messages).await?;
    
    // 6. 执行工具调用
    let executor = ToolExecutor::new(self.db_state.clone(), self.clone());
    for tool_call in response.tool_calls {
        if let Err(e) = executor.execute(&tool_call).await {
            crate::logger::backend("ERROR", &format!("[trigger_special] tool execution failed: {}", e));
        }
    }
    
    // 7. 清除 is_triggering
    self.clear_triggering_flag(agent_id).await?;
    
    Ok(())
}
```

- [ ] **Step 3: Implement timer scan + proactive scan**

```rust
pub async fn start_timer_scan(self) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
    loop {
        interval.tick().await;
        self.scan_scheduled_tasks().await;
        self.scan_proactive_timers().await;
    }
}

async fn scan_scheduled_tasks(&self) {
    let conn = self.db_state.0.lock().await;
    let now = chrono::Utc::now().timestamp_millis();
    let tasks = match scheduled_task_repo::get_due_tasks(&conn, now) {
        Ok(t) => t,
        Err(e) => {
            crate::logger::backend("ERROR", &format!("[TimerScan] query failed: {}", e));
            return;
        }
    };
    drop(conn);
    
    for task in tasks {
        {
            let conn = self.db_state.0.lock().await;
            if task.task_type == "single" {
                if let Err(e) = scheduled_task_repo::deactivate_task(&conn, &task.id) {
                    crate::logger::backend("ERROR", &format!("[TimerScan] deactivate failed: {}", e));
                }
            } else {
                let new_next = task.next_trigger_at + (task.interval_minutes.unwrap_or(60) as i64) * 60 * 1000;
                if let Err(e) = scheduled_task_repo::update_next_trigger(&conn, &task.id, new_next) {
                    crate::logger::backend("ERROR", &format!("[TimerScan] update next trigger failed: {}", e));
                }
            }
        }
        
        let scheduler = self.clone();
        let task_clone = task.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = scheduler.trigger_special(
                &task_clone.agent_id,
                SpecialTriggerContext::Timer {
                    description: task_clone.description,
                    target_session_id: task_clone.target_session_id,
                }
            ).await {
                crate::logger::backend("ERROR", &format!("[TimerTrigger] failed: {}", e));
            }
        });
    }
}

async fn scan_proactive_timers(&self) {
    let now = chrono::Utc::now().timestamp_millis();
    let timers = self.proactive_timers.lock().await.clone();
    
    for (agent_id, next_at) in timers {
        if next_at > now {
            continue;
        }
        
        if self.is_in_quiet_hours().await {
            self.reset_proactive_timer(&agent_id).await;
            continue;
        }
        
        let scheduler = self.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = scheduler.trigger_special(
                &agent_id,
                SpecialTriggerContext::Proactive
            ).await {
                crate::logger::backend("ERROR", &format!("[ProactiveTrigger] failed: {}", e));
            }
        });
        
        self.reset_proactive_timer(&agent_id).await;
    }
}

async fn is_in_quiet_hours(&self) -> bool {
    let conn = self.db_state.0.lock().await;
    let settings = match settings_repo::get_or_create_settings(&conn) {
        Ok(s) => s,
        Err(_) => return false,
    };
    drop(conn);
    
    if settings.quiet_hours_start < 0 || settings.quiet_hours_end < 0 {
        return false;
    }
    
    let now = chrono::Local::now();
    let current_minutes = (now.hour() * 60 + now.minute()) as i32;
    
    if settings.quiet_hours_start <= settings.quiet_hours_end {
        current_minutes >= settings.quiet_hours_start && current_minutes < settings.quiet_hours_end
    } else {
        current_minutes >= settings.quiet_hours_start || current_minutes < settings.quiet_hours_end
    }
}

pub async fn set_proactive_timer(&self, agent_id: &str, next_at: i64) {
    self.proactive_timers.lock().await.insert(agent_id.to_string(), next_at);
}

async fn reset_proactive_timer(&self, agent_id: &str) {
    let conn = self.db_state.0.lock().await;
    let agent = match agent_repo::get_agent(&conn, agent_id) {
        Ok(a) => a,
        Err(_) => return,
    };
    drop(conn);
    
    if agent.proactive_enabled == 0 {
        self.proactive_timers.lock().await.remove(agent_id);
        return;
    }
    
    let min_ms = agent.proactive_min_minutes as i64 * 60 * 1000;
    let max_ms = agent.proactive_max_minutes as i64 * 60 * 1000;
    let random_ms = rand::random_range(min_ms..=max_ms);
    let next = chrono::Utc::now().timestamp_millis() + random_ms;
    self.proactive_timers.lock().await.insert(agent_id.to_string(), next);
}

pub async fn init_proactive_timers(&self) {
    let conn = self.db_state.0.lock().await;
    let agents = match agent_repo::list_all_agents(&conn) {
        Ok(a) => a.into_iter().filter(|a| a.proactive_enabled != 0).collect::<Vec<_>>(),
        Err(_) => return,
    };
    drop(conn);
    
    let now = chrono::Utc::now().timestamp_millis();
    let mut timers = self.proactive_timers.lock().await;
    
    for agent in agents {
        let min_ms = agent.proactive_min_minutes as i64 * 60 * 1000;
        let max_ms = agent.proactive_max_minutes as i64 * 60 * 1000;
        let random_ms = rand::random_range(min_ms..=max_ms);
        timers.insert(agent.id, now + random_ms);
    }
}
```

- [ ] **Step 4: Wire up timer scan in lib.rs**

在 `src-tauri/src/lib.rs` 中，应用启动后启动 timer scan：

```rust
// 在 setup 中，scheduler recover 之后
scheduler.init_proactive_timers().await;
let scheduler_clone = scheduler.clone();
tauri::async_runtime::spawn(async move {
    scheduler_clone.start_timer_scan().await;
});
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/scheduler/mod.rs src-tauri/src/lib.rs
git commit -m "feat(scheduler): add trigger_special, timer scan, proactive scan"
```

---

## Task 6: 后端命令

**Files:**
- Create: `src-tauri/src/commands/timer.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create timer commands**

```rust
use tauri::State;
use crate::db::connection::DbState;
use crate::db::scheduled_task as scheduled_task_repo;
use crate::models::scheduled_task::{ScheduledTask, CreateTimerRequest, UpdateTimerRequest};

#[tauri::command]
pub async fn list_agent_timers(
    db_state: State<'_, DbState>,
    agent_id: String,
) -> Result<Vec<ScheduledTask>, String> {
    let conn = db_state.0.lock().await;
    scheduled_task_repo::list_by_agent(&conn, &agent_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_timer(
    db_state: State<'_, DbState>,
    agent_id: String,
    req: CreateTimerRequest,
) -> Result<String, String> {
    let conn = db_state.0.lock().await;
    let now = chrono::Utc::now().timestamp_millis();
    let next_trigger_at = /* 计算逻辑同 ToolExecutor */;
    let mut req_with_next = req;
    req_with_next.next_trigger_at = Some(next_trigger_at);
    let id = scheduled_task_repo::insert_task(&conn, &req_with_next, &agent_id).map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub async fn update_timer(
    db_state: State<'_, DbState>,
    req: UpdateTimerRequest,
) -> Result<(), String> {
    let conn = db_state.0.lock().await;
    scheduled_task_repo::update_task(&conn, &req.id, req.description.as_deref(), req.next_trigger_at, req.target_session_id.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_timer_command(
    db_state: State<'_, DbState>,
    task_id: String,
) -> Result<(), String> {
    let conn = db_state.0.lock().await;
    scheduled_task_repo::delete_task(&conn, &task_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn toggle_timer(
    db_state: State<'_, DbState>,
    task_id: String,
    is_active: i32,
) -> Result<(), String> {
    let conn = db_state.0.lock().await;
    scheduled_task_repo::toggle_task(&conn, &task_id, is_active).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_agent_proactive(
    db_state: State<'_, DbState>,
    agent_id: String,
    proactive_enabled: i32,
    proactive_min_minutes: i32,
    proactive_max_minutes: i32,
) -> Result<(), String> {
    let conn = db_state.0.lock().await;
    agent_repo::update_proactive_config(&conn, &agent_id, proactive_enabled, proactive_min_minutes, proactive_max_minutes)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_quiet_hours(
    db_state: State<'_, DbState>,
    quiet_hours_start: i32,
    quiet_hours_end: i32,
) -> Result<(), String> {
    let conn = db_state.0.lock().await;
    settings_repo::update_quiet_hours(&conn, quiet_hours_start, quiet_hours_end)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_quiet_hours(
    db_state: State<'_, DbState>,
) -> Result<(i32, i32), String> {
    let conn = db_state.0.lock().await;
    let settings = settings_repo::get_or_create_settings(&conn).map_err(|e| e.to_string())?;
    Ok((settings.quiet_hours_start, settings.quiet_hours_end))
}
```

- [ ] **Step 2: Register commands in lib.rs**

在 `generate_handler!` 中添加新命令。

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/timer.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(commands): add timer, proactive, quiet_hours commands"
```

---

## Task 7: Prompt 模板更新

**Files:**
- Modify: `src-tauri/src/llm/prompt_templates.rs`
- Modify: `src-tauri/src/llm/prompt.rs`

- [ ] **Step 1: Add prompt template constants**

在 `prompt_templates.rs` 中添加：

```rust
pub const TIMER_CAPABILITY: &str = r#"【定时事件能力】
你拥有设定定时事件的能力：
- 当你需要记住某个未来的约定、事件或提醒时，可以使用 create_timer 工具设定一个定时任务。
- 支持单次触发（指定时间或多少分钟后）和循环触发（按固定间隔重复）。
- 到时间后，你会收到一次特殊调用，Prompt 中会标注【定时任务触发】及事件内容。
- 你可以在【等待中的定时任务】中查看当前已设定但未触发的任务。
"#;

pub const TIMER_TRIGGER_TITLE: &str = "【定时任务触发】";
pub const PROACTIVE_TRIGGER_TITLE: &str = "【主动会话触发】";
pub const PENDING_TIMERS_TITLE: &str = "【等待中的定时任务】";
```

- [ ] **Step 2: Update PromptAssembler**

在 `PromptAssembler::assemble` 中：
1. Layer 1 System Prompt 后追加 `TIMER_CAPABILITY`
2. Layer 2 人设之后，查询并注入【等待中的定时任务】

```rust
// Layer 1: System Prompt + Timer Capability
let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
let system_with_timer = format!("{}\n{}", 
    prompt_templates::SYSTEM_PROMPT.replace("{current_time}", &now),
    prompt_templates::TIMER_CAPABILITY
);
layers.push(system_with_timer);

// ... Layer 2: Self Persona ...

// Layer 2.8: Pending Timers（人设之后）
let pending_timers = Self::get_pending_timers(conn, agent_id)?;
if !pending_timers.is_empty() {
    let mut layer = String::from(prompt_templates::PENDING_TIMERS_TITLE);
    layer.push('\n');
    for (task_id, description, next_at) in pending_timers {
        let time_str = chrono::Local::timestamp_millis(next_at).format("%m-%d %H:%M").to_string();
        layer.push_str(&format!("{}: {}（下次触发：{}）\n", task_id, description, time_str));
    }
    layers.push(layer);
}
```

添加 `get_pending_timers` 辅助方法：

```rust
fn get_pending_timers(conn: &Connection, agent_id: &str) -> Result<Vec<(String, String, i64)>, String> {
    let mut stmt = conn.prepare(
        "SELECT id, description, next_trigger_at FROM scheduled_tasks WHERE agent_id = ?1 AND is_active = 1 ORDER BY next_trigger_at ASC"
    ).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([agent_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?))
    }).map_err(|e| e.to_string())?;
    rows.filter_map(|r| r.ok()).collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/llm/prompt_templates.rs src-tauri/src/llm/prompt.rs
git commit -m "feat(prompt): add timer capability, pending timers injection, trigger titles"
```

---

## Task 8: 前端 TypeScript 类型 + API

**Files:**
- Modify: `src/lib/types.ts`

- [ ] **Step 1: Extend TypeScript types**

在 `types.ts` 中添加：

```typescript
export interface ScheduledTask {
    id: string;
    agent_id: string;
    description: string;
    task_type: 'single' | 'recurring';
    trigger_mode?: 'after_minutes' | 'datetime';
    after_minutes?: number;
    year?: number;
    month?: number;
    day?: number;
    hour?: number;
    minute?: number;
    interval_minutes?: number;
    next_trigger_at: number;
    created_at: number;
    is_active: number;
    target_session_id?: string;
}

export interface TimerFormData {
    description: string;
    task_type: 'single' | 'recurring';
    trigger_mode?: 'after_minutes' | 'datetime';
    after_minutes?: number;
    year?: number;
    month?: number;
    day?: number;
    hour?: number;
    minute?: number;
    interval_minutes?: number;
    target_session_id?: string;
}
```

更新 `Agent` 接口：

```typescript
export interface Agent {
    // ... 已有字段 ...
    proactive_enabled: number;
    proactive_min_minutes: number;
    proactive_max_minutes: number;
}
```

更新 `Settings` 接口：

```typescript
export interface Settings {
    // ... 已有字段 ...
    quiet_hours_start: number;
    quiet_hours_end: number;
}
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/types.ts
git commit -m "feat(types): add ScheduledTask, proactive fields, quiet_hours"
```

---

## Task 9: 前端 UI - CHAT-41 定时任务面板

**Files:**
- Create: `src/lib/components/AgentTimerPanel.svelte`
- Create: `src/lib/components/TimerEditModal.svelte`
- Modify: `src/lib/components/AgentDetail.svelte`

- [ ] **Step 1: Create TimerEditModal**

新建弹窗组件，支持：
- 描述输入
- 类型切换（单次/循环）
- 单次：触发方式切换（多少分钟后/指定时间）
- 循环：快捷选择（每天/每小时/自定义分钟）或数字输入
- 期望会话：下拉选择（可选）

- [ ] **Step 2: Create AgentTimerPanel**

展示表格：描述、类型、下次触发时间、状态
操作：编辑、删除、暂停/恢复
顶部：【+ 新建定时任务】按钮

- [ ] **Step 3: Integrate into AgentDetail**

在 `AgentDetail.svelte` 的标签页中新增【定时任务】标签。

- [ ] **Step 4: Commit**

```bash
git add src/lib/components/AgentTimerPanel.svelte src/lib/components/TimerEditModal.svelte src/lib/components/AgentDetail.svelte
git commit -m "feat(ui): add agent timer panel and edit modal"
```

---

## Task 10: 前端 UI - CHAT-42 主动会话 + 安静时段

**Files:**
- Modify: `src/lib/components/AgentDetail.svelte`（角色配置部分）
- Modify: `src/lib/components/SettingsPanel.svelte` 或相关设置组件

- [ ] **Step 1: Update AgentDetail proactive config**

在【角色配置】标签页底部增加：

```svelte
<div class="mt-6 border-t pt-4">
    <h3 class="font-semibold mb-2">主动会话机制</h3>
    <label class="flex items-center gap-2 mb-2">
        <input type="checkbox" bind:checked={agent.proactive_enabled} />
        <span>启用主动会话</span>
    </label>
    {#if agent.proactive_enabled}
        <div class="flex gap-2 items-center">
            <span>触发时间区间（分钟）</span>
            <input type="number" bind:value={agent.proactive_min_minutes} min="1" />
            <span>~</span>
            <input type="number" bind:value={agent.proactive_max_minutes} min="1" />
        </div>
    {/if}
</div>
```

保存时调用 `update_agent_proactive`。

- [ ] **Step 2: Update SettingsPanel quiet hours**

在全局设置中增加安静时段配置：

```svelte
<div class="mt-4">
    <h3 class="font-semibold mb-2">安静时段</h3>
    <label class="flex items-center gap-2 mb-2">
        <input type="checkbox" bind:checked={quietHoursEnabled} />
        <span>启用安静时段</span>
    </label>
    {#if quietHoursEnabled}
        <div class="flex gap-2 items-center">
            <input type="time" bind:value={quietStart} />
            <span>~</span>
            <input type="time" bind:value={quietEnd} />
        </div>
    {/if}
</div>
```

保存时调用 `update_quiet_hours`。

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/AgentDetail.svelte src/lib/components/SettingsPanel.svelte
git commit -m "feat(ui): add proactive session config and quiet hours settings"
```

---

## Task 11: 集成验证

- [ ] **Step 1: Rust 编译检查**

```bash
cd src-tauri
cargo check
```

- [ ] **Step 2: 前端类型检查**

```bash
npx svelte-check --tsconfig ./tsconfig.json
```

- [ ] **Step 3: 功能验证清单**

| 验证项 | 预期结果 |
|--------|---------|
| 创建定时任务（单次 after_minutes） | 任务插入数据库，next_trigger_at 正确 |
| 创建定时任务（单次 datetime） | 同上，时间戳正确 |
| 创建定时任务（循环） | 同上，interval_minutes 正确 |
| 定时任务到期触发 | 角色被调用，Prompt 中包含【定时任务触发】 |
| 角色使用 create_timer Tool | 任务创建成功，返回 task_id |
| 角色使用 delete_timer Tool | 任务删除成功 |
| 前端展示定时任务列表 | 列表正确显示 |
| 前端新建/编辑/删除任务 | 操作成功，数据库同步 |
| 启用主动会话 | 角色发消息后计时重置 |
| 主动会话触发 | 角色被调用，Prompt 中包含【主动会话触发】 |
| 安静时段内不触发 | 触发被跳过，计时重新随机 |
| Prompt 中【等待中的定时任务】 | 正确显示活跃任务 |

- [ ] **Step 4: Commit any fixes**

---

## 依赖关系图

```
Task 1 (Migration + Models)
    │
    ├─► Task 2 (scheduled_task Repo)
    │       │
    │       └─► Task 4 (LLM Tools)
    │
    ├─► Task 3 (Agent/Settings Repo)
    │       │
    │       ├─► Task 5 (Scheduler)
    │       │       │
    │       │       └─► Task 6 (Commands)
    │       │
    │       └─► Task 7 (Prompt)
    │
    └─► Task 8 (Types)
            │
            ├─► Task 9 (Timer UI)
            └─► Task 10 (Proactive UI)
```

**并行执行建议**：
- Task 1 → Task 2, Task 3 可并行
- Task 2 + Task 3 → Task 4, Task 5 可并行
- Task 5 → Task 6, Task 7 可并行
- Task 8 → Task 9, Task 10 可并行
- Task 11 最后串行
