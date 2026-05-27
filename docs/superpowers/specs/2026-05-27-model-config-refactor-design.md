# 设计文档：角色模型选择重构

**日期**: 2026-05-27
**需求编号**: AGT-19
**状态**: 待审批

---

## 1. 目标

将当前「每个角色独立配置模型 API」的架构重构为「全局模型配置 + 角色引用选择」的架构，简化模型管理，支持多模型复用。

---

## 2. 核心变更

### 2.1 架构对比

| 维度 | 重构前 | 重构后 |
|------|--------|--------|
| 模型配置位置 | 每个角色独立存储 (`agents.model_provider` 等) | 全局 `model_configs` 表，可配置多个 |
| 角色模型选择 | 直接填写 provider/model/base_url/api_key | 从下拉列表选择已配置的全局模型 |
| 链接测试 | 在角色创建/编辑弹窗中 | 移到全局「模型配置」面板中 |
| 思考模式 | 每个角色有开关 (`thinking_mode`) | **直接移除**（未生效） |
| Temperature | 每个角色必填 | **角色层保留，改为可选**。若角色配置了则覆盖全局模型配置的值；若未配置则使用所选模型的默认值 |
| max_tokens/top_p 等 | 每个角色必填 | **移到全局模型配置** |

### 2.2 数据保留策略

- **保留**: 角色的名称、人设、头像、关系、记忆、主动会话配置等所有非模型字段
- **删除**: 角色的模型相关字段（`model_provider`, `model_name`, `base_url`, `api_key_encrypted`, `max_tokens`, `top_p`, `presence_penalty`, `frequency_penalty`, `thinking_mode`）
- **新增**: 角色的 `model_config_id`（外键引用全局配置）+ `temperature`（可选覆盖值，可为 `NULL`）

---

## 3. 数据库变更 (Migration V19)

### 3.1 新建 `model_configs` 表

```sql
CREATE TABLE model_configs (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,           -- 用户自定义名称，如 "GPT-4o 生产环境"
    provider TEXT NOT NULL,       -- openai / anthropic / google / kimi / minimax / custom
    model_name TEXT NOT NULL,     -- 模型ID，如 gpt-4o
    base_url TEXT,                -- 自定义Base URL，为空时使用provider默认值
    api_key_encrypted BLOB,       -- AES-256-GCM加密存储
    temperature REAL,             -- 默认温度，可为NULL（非必填）
    max_tokens INTEGER DEFAULT 2048,
    top_p REAL DEFAULT 1.0,
    presence_penalty REAL DEFAULT 0.0,
    frequency_penalty REAL DEFAULT 0.0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
```

### 3.2 修改 `agents` 表

```sql
-- 新增外键和可选覆盖字段
ALTER TABLE agents ADD COLUMN model_config_id TEXT REFERENCES model_configs(id);
ALTER TABLE agents ADD COLUMN temperature REAL;  -- NULL = 使用模型全局默认值

-- 删除旧模型字段
ALTER TABLE agents DROP COLUMN model_provider;
ALTER TABLE agents DROP COLUMN model_name;
ALTER TABLE agents DROP COLUMN base_url;
ALTER TABLE agents DROP COLUMN api_key_encrypted;
ALTER TABLE agents DROP COLUMN max_tokens;
ALTER TABLE agents DROP COLUMN top_p;
ALTER TABLE agents DROP COLUMN presence_penalty;
ALTER TABLE agents DROP COLUMN frequency_penalty;
ALTER TABLE agents DROP COLUMN thinking_mode;
```

---

## 4. 后端设计

### 4.1 新增模型 (src/models/model_config.rs)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub model_name: String,
    pub base_url: Option<String>,
    pub api_key_encrypted: Option<Vec<u8>>,
    pub temperature: Option<f64>,
    pub max_tokens: i32,
    pub top_p: f64,
    pub presence_penalty: f64,
    pub frequency_penalty: f64,
    pub created_at: i64,
    pub updated_at: i64,
}

// Response DTO：解密后的api_key用于前端显示（编辑时回填）
#[derive(Debug, Clone, Serialize)]
pub struct ModelConfigResponse {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub model_name: String,
    pub base_url: Option<String>,
    pub api_key: String,           // 解密后的明文，仅用于编辑回填
    pub temperature: Option<f64>,
    pub max_tokens: i32,
    pub top_p: f64,
    pub presence_penalty: f64,
    pub frequency_penalty: f64,
    pub created_at: i64,
    pub updated_at: i64,
}
```

### 4.2 修改 Agent 模型

```rust
// Agent 结构体移除：model_provider, model_name, base_url, api_key_encrypted, 
// max_tokens, top_p, presence_penalty, frequency_penalty, thinking_mode
// Agent 结构体新增：model_config_id: Option<String>, temperature: Option<f64>

// AgentResponse 需要内联模型配置信息（或前端独立查询）
// 方案：AgentResponse 保留 model_name（用于列表展示），通过JOIN model_configs获取
```

### 4.3 新增命令 (src/commands/model_config.rs)

| 命令 | 功能 |
|------|------|
| `list_model_configs` | 列出所有全局模型配置 |
| `create_model_config` | 创建新的全局模型配置（含API Key加密） |
| `update_model_config` | 修改全局模型配置 |
| `delete_model_config` | 删除全局模型配置（需检查是否有角色引用） |
| `test_model_config_connection` | 测试指定模型配置的连通性（从原 `test_api_connection` 迁移） |

### 4.4 修改命令

| 命令 | 变更 |
|------|------|
| `create_agent` | 移除 `model_provider`, `model_name`, `base_url`, `api_key`, `max_tokens`, `top_p`, `presence_penalty`, `frequency_penalty`, `thinking_mode` 参数；新增 `model_config_id` + 可选 `temperature` |
| `update_agent` | 同上 |
| `get_agent` / `list_agents` | 返回结果中通过 JOIN `model_configs` 提供 `model_name` 用于展示；`temperature` 返回角色层的值（可能为NULL） |
| `test_api_connection` | **删除**，功能由 `test_model_config_connection` 替代 |

### 4.5 LLM 调用层适配

当前调用 LLM 时从 `Agent` 结构体直接读取模型参数。重构后：

1. 根据 `agent.model_config_id` 查询 `model_configs`
2. 使用模型配置的 `provider`, `model_name`, `base_url`, `api_key_encrypted`
3. **Temperature 优先级**：
   - `agent.temperature`（若不为 NULL）→ 传入 LLM 请求
   - `model_config.temperature`（若不为 NULL）→ 传入 LLM 请求
   - **两者皆为 NULL → 不传入 temperature 参数**，由 LLM 提供商自行决定默认值
4. max_tokens/top_p 等直接使用 `model_config` 的值

### 4.6 引用完整性约束

- 删除 `model_config` 时，若存在引用的 `agents`，拒绝删除并提示用户先解除引用或迁移角色。
- 可选：级联设为 NULL（让角色失去模型配置，但这样角色无法工作，不如直接拒绝删除）。

**决策**: 拒绝删除（更安全，避免意外导致角色失效）。

---

## 5. 前端设计

### 5.1 新增：全局「模型配置」标签页 (SettingsPanel)

在 `SettingsPanel` 的 Tab 栏新增「模型」Tab：

- **模型列表**: 卡片/列表展示所有已配置模型（名称、provider、模型名）
- **新增模型**: 弹窗表单，字段：
  - 配置名称（必填）
  - Provider 下拉选择（openai/anthropic/google/kimi/minimax/custom）
  - 模型名称（必填）
  - Base URL（可选，provider切换时自动填充默认值）
  - API Key（密码框，必填）
  - Temperature（数值输入，**可为空**，placeholder提示"使用默认值"）
  - max_tokens / top_p / presence_penalty / frequency_penalty（高级折叠区，默认值）
  - **测试连接** 按钮
- **编辑模型**: 同新增，但回填数据
- **删除模型**: 确认弹窗，若被角色引用则提示无法删除

### 5.2 修改：角色创建/编辑弹窗

**CreateAgentModal** 和 **AgentDetail** 的「模型配置」区域大幅简化：

- 移除：provider选择、model_name输入、base_url输入、api_key输入、max_tokens、top_p、presence_penalty、frequency_penalty、思考模式开关
- 移除：「导入其他角色模型配置」按钮和弹窗（`ImportModelConfigModal` 可删除）
- 保留：Temperature（数值输入，**可为空**，placeholder提示"使用模型默认值"）
- 新增：「选择模型」下拉框，选项来源 `list_model_configs`，显示 `name (provider / model_name)`
- 新增：若未选择模型，保存时提示错误

### 5.3 修改：AgentList 展示

- `agent.model_name` 不再直接存在，需通过 `agent.model_config_id` 关联查询
- 方案：`list_agents` 后端 JOIN `model_configs` 返回 `model_name`，前端无需改动展示逻辑

### 5.4 删除组件

- `ImportModelConfigModal.svelte` — 全局模型配置后不再需要「从其他角色导入」

---

## 6. 数据流：LLM 调用时如何解析 Temperature

```
调用 LLM(agent_id)
  ├── 查询 Agent → model_config_id + agent.temperature
  ├── 查询 ModelConfig → model_config.temperature + 其他参数
  └── 解析最终 temperature:
        IF agent.temperature IS NOT NULL → 传入 agent.temperature
        ELSE IF model_config.temperature IS NOT NULL → 传入 model_config.temperature
        ELSE → 不传入 temperature 参数（由 LLM 提供商决定默认值）
```

---

## 7. 边界情况处理

| 场景 | 处理 |
|------|------|
| 角色未选择模型配置 | 保存时校验报错；列表中显示「未配置模型」 |
| 删除被引用的模型配置 | 拒绝删除，Toast 提示「该配置正被 N 个角色使用」 |
| 模型配置的 API Key 为空 | 允许保存，但连接测试会失败；LLM 调用时若Key为空则报错 |
| 全局模型配置 temperature 为空 | 允许保存；LLM 调用时不传入 temperature，由提供商决定默认值 |
| 角色 temperature 为空 | 允许保存；LLM 调用时回退到模型配置的 temperature；若模型配置也为空则不传入 |
| 已有数据（Migration V19） | agents 表旧模型列直接 DROP；model_config_id 为 NULL；用户需重新进入全局设置创建模型配置并给角色分配 |

---

## 8. 测试策略

1. **Schema 兼容性**: Migration V19 在全新数据库和已有数据库上均能通过
2. **CRUD 测试**: 模型配置的增删改查 + 引用完整性（删除被引用配置）
3. **Temperature 优先级**: agent.temperature > model_config.temperature > 默认值
4. **连接测试**: `test_model_config_connection` 成功/失败/超时场景
5. **角色创建/编辑**: 无模型配置选择时保存阻断；选择后正常创建
6. **LLM 调用**: 验证从 model_config 正确读取参数（provider/model/base_url/key）

---

## 9. 风险与回退

| 风险 | 缓解措施 |
|------|----------|
| Migration V19 DROP COLUMN 在旧 SQLite 版本不支持 | rusqlite 捆绑的 SQLite ≥ 3.35 支持 DROP COLUMN；AgentStage 已要求较新版本 |
| 现有角色突然失去模型配置无法工作 | 预期行为（用户已确认接受）；在 AgentList 中「未配置模型」醒目标识引导用户去设置 |
| API Key 加密迁移到新表 | 复用现有 `crypto.rs` 的 encrypt/decrypt，无新加密逻辑 |

---

*设计完成，等待审批。*
