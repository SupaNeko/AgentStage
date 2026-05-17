# AGT-14：角色关系设定及维护 — 设计文档

*日期：2026-05-17*
*对应计划：2026-05-17-agent-relationship.md*

---

## 1. 功能概述

为每个 Agent 维护其对**其他 Agent** 和**用户人设**的**单向关系描述**（主观看法）。该描述在 Prompt 的【你认识的参与者】层中注入，影响 Agent 的对话态度和行为。

区别于客观「简易人设」，关系描述是**主观认知**：
- 简易人设："远坂凛，Fate/stay night 的魔术师，擅长宝石魔术"
- 关系描述："她是我的竞争对手，但关键时刻值得信赖"

提供 `update_relationship` Tool，允许 Agent 在会话过程中自主更新关系。

---

## 2. 数据模型

### 2.1 新表：`agent_relationships`

```sql
CREATE TABLE IF NOT EXISTS agent_relationships (
    observer_id TEXT NOT NULL,                    -- 观察者的 agent_id
    target_id TEXT NOT NULL,                      -- 目标 id（agent_id 或 user_persona_id）
    target_type TEXT NOT NULL CHECK(target_type IN ('agent', 'user_persona')),
    relationship_text TEXT NOT NULL DEFAULT '',   -- 关系描述（最多 200 字）
    updated_at INTEGER NOT NULL,
    
    PRIMARY KEY (observer_id, target_id, target_type),
    FOREIGN KEY (observer_id) REFERENCES agents(id) ON DELETE CASCADE
);
```

**设计说明：**
- `observer_id` 必须是 agent，因为**只有 Agent 需要关系描述**（用户不需要看自己怎么看别人）
- `target_id` + `target_type` 组成联合目标标识，支持指向 agent 或 user_persona
- 不设 FK 到 `agents(id)` 或 `user_personas(id)`，因为 target_type 是动态的，SQLite 不支持跨表条件 FK
- 关系文本默认为空字符串 `''`，表示"尚无主观认定"
- 当 `observer_id` 对应的 agent 被删除时，级联删除所有 observer 关系
- 当 `target_type='user_persona'` 的目标人设被删除时，由 application 层手动级联删除

### 2.2 索引

```sql
CREATE INDEX idx_agent_relationships_observer ON agent_relationships(observer_id);
CREATE INDEX idx_agent_relationships_target ON agent_relationships(target_id, target_type);
```

---

## 3. 后端 API

### 3.1 Tauri Commands

```rust
// 获取某个 agent 对所有关联对象的关系列表（用于前端"关系设定"标签页）
#[tauri::command]
pub async fn list_agent_relationships(
    state: State<'_, DbState>,
    agent_id: String,
) -> Result<Vec<RelationshipItem>, String>;

// 更新某个关系描述（用户在前端手动编辑）
#[tauri::command]
pub async fn update_agent_relationship(
    state: State<'_, DbState>,
    observer_id: String,
    target_id: String,
    target_type: String,
    relationship_text: String,
) -> Result<(), String>;

// 由 Tool 调用：Agent 自主更新关系（新旧文本匹配）
pub async fn update_relationship_via_tool(
    conn: &Connection,
    observer_id: &str,
    target_id: &str,
    target_type: &str,
    old_text: &str,
    new_text: &str,
) -> Result<String, String>; // 返回成功或错误消息
```

### 3.2 DTO

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipItem {
    pub target_id: String,
    pub target_type: String, // "agent" | "user_persona"
    pub target_name: String,
    pub target_avatar: Option<String>,
    pub target_label: String, // "用户" | "好友" | "群友"
    pub relationship_text: String,
    pub updated_at: i64,
}
```

**`target_label` 的判定规则（后端计算，前端直接显示）：**
1. `target_type == 'user_persona'` → `"用户"`
2. `target_type == 'agent'` 且存在 friendships 记录 → `"好友"`
3. `target_type == 'agent'` 且不存在 friendships 记录 → `"群友"`

---

## 4. Prompt 变更

### 4.1 Layer 3：参与者简介格式

当前格式：
```
- {name}（{relation}）：{persona}
```

新格式（关系描述非空时）：
```
- {name}（{relation}）：{persona}。主观关系认定：{relationship_text}
```

关系描述为空时，维持原格式（不追加句号和空白）：
```
- {name}（{relation}）：{persona}
```

### 4.2 获取参与者逻辑（`get_participants`）

修改 `prompt.rs` 中的 `get_participants()`：

1. 查询 `friendships` 确定关联对象列表（保持现有逻辑）
2. 对每一个关联对象，额外查询 `agent_relationships` 获取 `relationship_text`
3. 对于用户参与者（Layer 3 中的用户条目）：
   - 查询当前激活的 `user_persona_id`
   - 用 `(observer_id, target_id=persona_id, target_type='user_persona')` 查询关系描述
   - 如果未激活任何人设（默认模式），关系描述为空字符串

### 4.3 模板常量增加

在 `prompt_templates.rs` 中新增：

```rust
pub const RELATIONSHIP_SUFFIX_PREFIX: &str = "。主观关系认定：";
```

---

## 5. Tool 设计：`update_relationship`

### 5.1 JSON Schema

```rust
pub fn update_relationship_tool_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "update_relationship",
            "description": "更新你对某个参与者的主观关系描述。这用于记录你对对方的整体定位（如朋友/同事/竞争对手）和基本态度（如喜欢/讨厌/尊敬），不是记忆具体事件。请遵守以下规则：\n1. 只更新整体关系定位，不要记录日常琐事（如\"他今天吃了汉堡\"）\n2. 描述控制在 200 字以内\n3. 必须提供 old_text（当前关系描述的完整内容），系统会匹配替换\n4. 如果 old_text 不匹配（说明你记错了当前关系），系统会返回错误，请重新查询后再修改",
            "parameters": {
                "type": "object",
                "properties": {
                    "target_id": { "type": "string", "description": "目标参与者的 ID（agent_id 或 user_persona_id）" },
                    "target_type": { "type": "string", "enum": ["agent", "user_persona"], "description": "目标类型" },
                    "old_text": { "type": "string", "description": "当前关系描述的完整文本（空字符串表示尚无描述）" },
                    "new_text": { "type": "string", "description": "新的关系描述文本（200字以内）" }
                },
                "required": ["target_id", "target_type", "old_text", "new_text"]
            }
        }
    })
}
```

### 5.2 执行逻辑

```rust
async fn execute_update_relationship(
    &self,
    tc: &ToolCall,
) -> Result<ToolCallResponse, String> {
    let args = tc.arguments.clone();
    let target_id = args["target_id"].as_str().unwrap_or("");
    let target_type = args["target_type"].as_str().unwrap_or("");
    let old_text = args["old_text"].as_str().unwrap_or("");
    let new_text = args["new_text"].as_str().unwrap_or("");
    
    // 1. 校验长度
    if new_text.chars().count() > 200 {
        return Ok(ToolCallResponse {
            content: format!("更新失败：关系描述超过 200 字限制（当前 {} 字）", new_text.chars().count()),
            ..Default::default()
        });
    }
    
    // 2. 获取当前值进行匹配
    let current = get_relationship(&conn, &self.agent_id, target_id, target_type)?;
    if current != old_text {
        return Ok(ToolCallResponse {
            content: format!(
                "更新失败：old_text 不匹配。当前关系描述为：\"{}\"，请基于这个内容重新提交修改。",
                current
            ),
            ..Default::default()
        });
    }
    
    // 3. 更新
    update_relationship(&conn, &self.agent_id, target_id, target_type, new_text)?;
    
    Ok(ToolCallResponse {
        content: "关系描述已更新".to_string(),
        ..Default::default()
    })
}
```

### 5.3 Tool 注入时机

在 `PromptAssembler` 组装 Tools 时，`update_relationship` 对所有 Agent 始终可用（不需要条件判断）。

---

## 6. 前端 UI 设计

### 6.1 AgentDetail 重构为标签页

当前 `AgentDetail.svelte` 是单页长表单。重构为横向标签页：

```
[角色配置] [关系设定]
```

**标签页切换逻辑**：
- 使用简单的 `$state` 变量 `activeTab: 'config' | 'relationships'`
- 标签页按钮横向排列在头像下方
- 切换时不需要保存，只是切换视图

### 6.2 "角色配置"标签页

包含现有的所有表单内容：
- 基本信息（名称）
- 人设配置（详细人设、简易人设）
- 模型配置（提供商、模型、Base URL、API Key、温度、Max Tokens、thinking_mode）
- 底部操作栏（人设自生成、取消、保存）

**保存逻辑**：仅在"角色配置"标签页有"保存"按钮。关系设定的保存是即时/独立的。

### 6.3 "关系设定"标签页

**布局结构**：

```
顶部说明：以下关系描述会注入到该角色的 Prompt 中，影响其对话态度。

列表（按顺序）：
  ┌─────────────────────────────────────────────────────┐
  │ [头像] 用户-职场人设    [用户]    [输入框：关系描述]  │  ← 当前激活的用户人设
  ├─────────────────────────────────────────────────────┤
  │ [头像] 测试-远坂凛      [好友]    [输入框：关系描述]  │
  ├─────────────────────────────────────────────────────┤
  │ [头像] 测试-Saber       [群友]    [输入框：关系描述]  │
  └─────────────────────────────────────────────────────┘
```

**排序规则**：
1. 用户人设（如果当前有激活）排第一
2. 好友（按名称字母/创建时间排序）
3. 群友（按名称字母/创建时间排序）

**行内交互**：
- 头像 + 名称：只读展示
- 标签（用户/好友/群友）：small badge
- 输入框：`textarea` 或 `input`，支持多行（但限制200字）
- 字数统计：输入框右下角显示 `"0/200"`
- 自动保存：输入框 blur 或 debounce（500ms）后自动调用 `update_agent_relationship`
- 空状态：如果该 agent 没有任何关联对象（没有好友、没有群友、用户未激活人设），显示提示："该角色尚未与其他参与者建立关联，在群聊或私聊中会自动显示"

### 6.4 用户人设的特殊处理

- 只显示**当前激活**的用户人设
- 如果用户未激活任何人设（使用默认），"关系设定"列表中**不显示用户行**
- 用户切换人设后，列表自动刷新（显示新的人设行）
- 用户删除某个人设时，后端级联删除所有 agent 对该人设的关系记录

---

## 7. 边界情况与错误处理

| 场景 | 处理方案 |
|------|---------|
| 新建 Agent 无关联对象 | 关系设定页显示空状态提示 |
| Agent 被删除 | 级联删除 `agent_relationships` 中所有 `observer_id` 记录 |
| 用户人设被删除 | 后端手动删除 `target_type='user_persona'` 且 `target_id=该人设id` 的所有记录 |
| 用户切换人设 | 前端重新加载关系列表，显示新的人设关系 |
| 默认人设（无激活） | 不显示用户行，Prompt 中用户的关系描述为空 |
| Tool 中 old_text 不匹配 | 返回当前真实内容，让 Agent 重新调用 |
| 关系文本超过 200 字 | Tool 返回错误；前端输入框限制 maxlength |
| 目标 Agent 被删除 | 关系记录保留（observer 还在），但 target_name 显示时标注"已删除" |
| 群聊中新加入 Agent | 初始关系为空，由 Agent 自己维护 |

---

## 8. 与现有系统的联动

### 8.1 与 `friendships` 表的关系

- `friendships` 继续作为"关系存在"的标记（谁认识谁）
- `agent_relationships` 作为"关系质量/描述"的载体
- 两者独立：`friendships` 没有 `relationship_text` 字段，避免数据冗余

### 8.2 与 `user_personas` 的联动

- 删除人设时，在 `user_persona.rs` repository 的 `delete_user_persona` 中增加：
  ```rust
  conn.execute(
      "DELETE FROM agent_relationships WHERE target_type = 'user_persona' AND target_id = ?1",
      [&id],
  )?;
  ```

### 8.3 与 Prompt 的联动

- `PromptAssembler::get_participants()` 增加查询 `agent_relationships`
- 对每一条参与者记录，尝试获取 `(observer_id=当前agent, target_id=参与者id, target_type)` 的关系描述
- 注入格式：`{persona}。主观关系认定：{relationship_text}`

---

## 9. 迁移策略

新建 Migration V12：

```sql
-- 1. 创建 agent_relationships 表
CREATE TABLE IF NOT EXISTS agent_relationships (
    observer_id TEXT NOT NULL,
    target_id TEXT NOT NULL,
    target_type TEXT NOT NULL CHECK(target_type IN ('agent', 'user_persona')),
    relationship_text TEXT NOT NULL DEFAULT '',
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (observer_id, target_id, target_type)
);

-- 2. 创建索引
CREATE INDEX idx_agent_relationships_observer ON agent_relationships(observer_id);
CREATE INDEX idx_agent_relationships_target ON agent_relationships(target_id, target_type);
```

无需回填数据（初始为空）。

---

## 10. 测试策略（TDD）

### 10.1 后端测试

1. **Repository 测试**（手动 runner）：
   - `create_relationship` → 能正确插入记录
   - `get_relationship` → 空时返回 `""`，非空时返回正确文本
   - `update_relationship` → old_text 匹配时更新成功，不匹配时报错
   - `list_relationships_by_observer` → 返回正确的关联对象列表和标签
   - `delete_user_persona` → 级联删除相关关系记录

2. **Prompt 测试**：
   - `get_participants` 对无关系的参与者返回空关系描述（原格式）
   - `get_participants` 对有关系的参与者追加关系描述（新格式）
   - 用户未激活人设时，用户条目不追加关系描述

3. **Tool 测试**：
   - `update_relationship` 工具正常更新
   - `old_text` 不匹配时返回错误
   - 超过 200 字时返回错误

### 10.2 前端测试

1. **AgentDetail 标签页切换**：能正确切换"角色配置"和"关系设定"
2. **关系列表渲染**：按"用户→好友→群友"顺序渲染
3. **关系输入自动保存**：输入后 blur 调用 API
4. **空状态**：无关联对象时显示提示

---

*文档结束*
