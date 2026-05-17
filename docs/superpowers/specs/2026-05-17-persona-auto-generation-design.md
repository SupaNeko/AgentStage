# AGT-13 人设自生成设计文档

**日期**: 2026-05-17  
**功能编号**: AGT-13 — AI 辅助生成人设  
**状态**: 设计已确认，待实现

---

## 1. 背景与目标

当前 `PersonaGenerateModal` 为占位 UI，本设计实现完整的"人设自生成"功能。

**目标**: 用户通过提供"参考角色"（可选）和"补充信息"（可选，至少填一项），调用该角色配置的 LLM，分两步自动生成角色的结构化字段和人设文本，减少手动编写成本。

---

## 2. 核心设计决策

| 决策点 | 结论 |
|--------|------|
| 外部 websearch 工具 | **本次不实现**，仅通过提示词建议模型使用自身搜索能力（如有）。作为后续独立需求加入功能列表。 |
| 两步流程 | 统一走两步，允许第一步妥协（某些字段留空）。 |
| 多轮对话 | 两步在同一个连贯的对话上下文中执行，区别于原有单轮 agent 触发方式。 |
| 第 1 步结果获取 | **工具调用** `fill_character_fields`，结构化写入数据库预留字段。 |
| 第 2 步结果获取 | **固定格式输出**，`</>` XML-like 标签包裹长文本，避免 JSON 转义问题。 |
| 第二步是否入库 | **不入库**，返回前端填入文本框，用户确认后随保存统一写入。 |
| 取消/打断 | 不回滚第 1 步已写入的字段，视为"已积累的角色素材"。 |
| 新建角色支持 | 不传 `agent_id`，传 `model_config`；第 1 步结果随响应返回，避免壳角色问题。 |
| 字段精简 | 使用 `personality`、`scenario`、`example_messages`、`creator_notes`；`first_message` 和 `tags` 保留但本次不用。 |

---

## 3. 字段定义

### 3.1 启用的预留字段

| 字段 | 含义 | 第一步是否填充 | 可空性 |
|------|------|---------------|--------|
| `creator_notes` | 用户填写的"补充信息"内容 | 是（提前写入） | 是 |
| `personality` | 性格特征关键词/描述（如"傲娇、善良、天然呆"） | 是（AI 生成） | 是 |
| `scenario` | 角色所处世界观/场景（如"型月世界观，冬木市，圣杯战争"） | 是（AI 生成） | 是 |
| `example_messages` | 经典台词或示例对话 | 是（AI 生成） | 是 |

### 3.2 本次不用的预留字段

| 字段 | 处理方式 |
|------|---------|
| `first_message` | 保留在 schema 中，本次不触碰 |
| `tags` | 保留在 schema 中，本次不触碰 |

---

## 4. 整体流程

```
用户打开人设自生成弹窗
    │
    ▼
填写"参考角色"（可选）和"补充信息"（可选，至少一项）
    │
    ▼
点击"生成"按钮
    │
    ▼
【前端校验】
  ├─ 新建角色：检查 model_provider / model_name / api_key 是否已填
  │   └─ 未填 → 提示"请先填写模型信息"
  └─ 已有角色：检查该角色是否配置了模型
    │
    ▼
【前置写入】将"补充信息"写入 creator_notes（已有角色直接 UPDATE；新建角色暂存响应中）
    │
    ▼
【第 1 轮 LLM 调用】信息提取与结构化
  ├─ System Prompt：说明任务、输出要求、字段定义
  ├─ User Message：参考角色 + 补充信息 + 现有字段值（作为参考）
  ├─ Tools：提供 fill_character_fields
  └─ AI 响应：调用 fill_character_fields，传入 personality / scenario / example_messages
    │
    ▼
【后端执行工具】
  ├─ 已有角色：UPDATE agents SET personality=?, scenario=?, example_messages=? WHERE id=?
  └─ 新建角色：暂存到响应结构中
    │
    ▼
【第 2 轮 LLM 调用】人设生成
  ├─ 消息历史包含第 1 轮完整上下文（含工具调用结果）
  ├─ User Message：要求生成 detailed_persona 和 simplified_persona
  └─ AI 响应：直接输出文本，用 </detailed_persona> 和 </simplified_persona> 标签包裹
    │
    ▼
【后端解析】提取标签内容
    │
    ▼
【返回前端】
  ├─ personality / scenario / example_messages / creator_notes
  └─ detailed_persona / simplified_persona
    │
    ▼
前端将结果填入对应文本框
    │
    ▼
用户可手动修改 → 点击"保存" → update_agent / create_agent 统一入库
```

---

## 5. 后端设计

### 5.1 新增命令

```rust
#[tauri::command]
async fn generate_persona(
    db_state: tauri::State<'_, DbState>,
    req: GeneratePersonaRequest,
) -> Result<GeneratePersonaResponse, String>
```

#### 请求结构

```rust
struct ModelConfig {
    model_provider: String,
    model_name: String,
    base_url: Option<String>,
    api_key: String,
    temperature: f64,
    max_tokens: i32,
    thinking_mode: bool,
}

struct GeneratePersonaRequest {
    agent_id: Option<String>,        // 已有角色时传
    model_config: Option<ModelConfig>, // 新建角色时传
    reference_character: Option<String>, // 参考角色，如"Fate/stay night 中的 Saber"
    supplement: Option<String>,      // 补充信息
}
```

**校验规则**:
- `agent_id` 和 `model_config` 必须且只能传一个
- `reference_character` 和 `supplement` 至少有一个非空
- 传 `agent_id` 时，该角色必须存在且未删除，且配置了 `model_name` + `api_key`

#### 响应结构

```rust
struct GeneratePersonaResponse {
    personality: Option<String>,
    scenario: Option<String>,
    example_messages: Option<String>,
    creator_notes: Option<String>,
    detailed_persona: String,
    simplified_persona: String,
}
```

### 5.2 多轮对话实现

区别于原有 `Scheduler::call_llm` 的单轮调用（system + messages → response），人设自生成需要显式维护多轮消息历史。

```rust
// 第 1 轮
let mut messages = vec![user_msg_step1];
let response1 = provider.chat(&system_prompt, messages.clone(), tools_step1).await?;

// 解析工具调用，执行 fill_character_fields
let tool_results = execute_fill_character_fields(&response1.tool_calls, ...)?;

// 将 assistant 回复和工具结果追加到消息历史
messages.push(assistant_msg(response1));
for result in tool_results {
    messages.push(tool_result_msg(result));
}

// 第 2 轮
messages.push(user_msg_step2);
let response2 = provider.chat(&system_prompt, messages, vec![],).await?;
// 解析 </detailed_persona> 和 </simplified_persona> 标签
```

**关键**: 第 2 轮不传入任何 tools（`tools: vec![]`），强制 AI 直接输出文本而非工具调用。

### 5.3 第 1 步工具定义

```json
{
  "type": "function",
  "function": {
    "name": "fill_character_fields",
    "description": "将分析提取到的角色信息填入对应字段。如果某项信息无法确定或该角色为原创角色不在你的知识库中，可将对应字段设为空字符串。",
    "parameters": {
      "type": "object",
      "properties": {
        "personality": {
          "type": "string",
          "description": "角色的性格特征描述，如'傲娇、善良、有些天然呆'。可空。"
        },
        "scenario": {
          "type": "string",
          "description": "角色所处的世界观、场景或背景设定。可空。"
        },
        "example_messages": {
          "type": "string",
          "description": "角色的经典台词或代表性对话示例。可空。"
        }
      },
      "required": ["personality", "scenario", "example_messages"]
    }
  }
}
```

**注意**: 三个参数均必填但允许空字符串，确保模型明确表达"无法确定"而非遗漏参数。

### 5.4 第 2 步输出格式约束

System Prompt 中明确要求：

```
请基于以上分析结果，生成该角色的"详细人设"和"简易人设"。

输出格式要求：
<detailed_persona>
（详细人设内容，直接注入 System Prompt 的完整设定，2000字以内）
</detailed_persona>

<simplified_persona>
（简易人设内容，给其他角色看的简介，50字以内，以一两句话客观角度简单描述该角色的身份信息）
</simplified_persona>

注意：
1. 必须包含 <detailed_persona> 和 </simplified_persona> 标签
2. 标签之间不要添加其他说明文字
3. 如果参考角色信息不足，可基于补充信息和你的知识合理发挥
```

**解析逻辑**:
- 使用正则提取 `<detailed_persona>(.*?)</detailed_persona>` 和 `<simplified_persona>(.*?)</simplified_persona>`
- 如果标签缺失或内容为空，返回明确的解析错误
- 提取后 `trim()` 去除首尾空白

### 5.5 第一步的现有值参考

已有角色调用时，后端先从数据库读取当前的 `personality`、`scenario`、`example_messages`，在第 1 轮的 User Message 中追加：

```
【该角色当前已设定的信息（供参考，你可选择保留、修改或清空）】
性格特征: {current_personality}
所处场景: {current_scenario}
经典台词: {current_example_messages}
```

AI 回传新值时，直接覆盖数据库中的旧值（包括空字符串）。

### 5.6 错误处理

| 错误场景 | 处理方式 |
|---------|---------|
| 模型配置缺失 | 命令返回 `Err("该角色未配置模型信息，请先填写")` |
| LLM 调用失败 | 返回 `Err("人设生成失败: {具体错误}")`，前端 Toast 报错 |
| 第 1 步 AI 未调用工具 | 视为失败，返回错误提示 |
| 第 2 步标签解析失败 | 返回 `Err("AI 返回格式异常，请重试")` |
| 第 2 步内容为空 | 返回 `Err("AI 未生成有效人设内容")` |

---

## 6. 前端设计

### 6.1 PersonaGenerateModal 重构

```
┌─────────────────────────────┐
│  人设自生成              [X]  │
├─────────────────────────────┤
│  参考角色（可选）            │
│  ┌─────────────────────────┐│
│  │ 如：Fate/stay night 中的 ││
│  │ Saber                   ││
│  └─────────────────────────┘│
│  补充信息（可选）            │
│  ┌─────────────────────────┐│
│  │ 可填写任意相关内容：设定 ││
│  │ 要求、台词、聊天记录等   ││
│  └─────────────────────────┘│
│                             │
│  [至少填写一项才能生成]      │
│                             │
│  [取消]    [生成中... ⏳]   │
└─────────────────────────────┘
```

### 6.2 状态管理

```typescript
let generating = $state(false);
let referenceCharacter = $state('');
let supplement = $state('');
```

**按钮状态**:
- 禁用条件：`!referenceCharacter.trim() && !supplement.trim()` 或 `generating`
- 新建角色额外检查：`!form.model_name || !form.api_key`

### 6.3 生成中退出提示

`PersonaGenerateModal` 的 `onClose` 回调需要判断 `generating` 状态：

```typescript
function handleClose() {
    if (generating) {
        if (!confirm('退出将会打断生成，确定要退出吗？')) {
            return;
        }
        // TODO: 需要一种机制通知后端取消正在进行的 LLM 调用
        // 当前实现限制：HTTP 请求发出后无法中断，但前端可以忽略响应
    }
    onClose();
}
```

**注意**: 由于 LLM 调用是后端通过 HTTP 发出的，Tauri 命令一旦执行无法被前端"取消"。前端能做的只是在用户确认退出后忽略响应，不更新表单。后端 LLM 调用会继续执行完成，但不会返回给已关闭弹窗的前端。

### 6.4 结果回填

生成成功后，通过事件或回调将结果传回 `AgentDetail`：

```typescript
// PersonaGenerateModal 的 Props 扩展
interface Props {
    open: boolean;
    onClose: () => void;
    onGenerated: (result: {
        detailed_persona: string;
        simplified_persona: string;
        personality?: string;
        scenario?: string;
        example_messages?: string;
        creator_notes?: string;
    }) => void;
}
```

`AgentDetail` 中：
```typescript
function handleGenerated(result: ...) {
    form.detailed_persona = result.detailed_persona;
    form.simplified_persona = result.simplified_persona;
    // 如果有返回 personality/scenario/example_messages（新建角色时）
    // 这些字段目前前端没有输入框，但可作为未来扩展保留
    showGenerateModal = false;
    toastStore.show('人设生成完成，请检查并保存', 'success', 3000);
}
```

---

## 7. 新建角色 vs 已有角色差异

| 维度 | 已有角色 | 新建角色 |
|------|---------|---------|
| 请求参数 | 传 `agent_id` | 传 `model_config`（从前端表单读取） |
| 模型配置来源 | 数据库读取 | 前端传入 |
| creator_notes 写入 | `UPDATE agents` | 随响应返回，不入库 |
| 第 1 步结果写入 | `UPDATE agents` | 随响应返回，不入库 |
| 前端回填 | 只回填 detailed/simplified_persona | 回填所有字段（含 personality 等） |
| 保存时机 | `update_agent` | `create_agent`（含所有字段） |

---

## 8. Prompt 设计

### 8.1 第 1 步 System Prompt 模板

```
你是一个专业的角色设定分析师。你的任务是根据用户提供的参考角色信息和补充内容，提取并结构化角色的核心设定信息。

如果你具备网络搜索能力，建议先搜索该参考角色的详细信息（尤其是性格设定、世界观背景和经典台词），以提高分析准确性。

你需要调用 fill_character_fields 工具，将分析结果填入以下字段：
- personality: 性格特征描述
- scenario: 所处世界观/场景
- example_messages: 经典台词或代表性对话

如果该角色不在你的知识库中（如原创角色），或某项信息无法确定，可将对应字段设为空字符串。
```

### 8.2 第 1 步 User Message 模板

```
【参考角色】
{reference_character}

【补充信息】
{supplement}

{cursor_existing_values}

请分析以上信息，调用 fill_character_fields 工具填写角色设定字段。
```

其中 `cursor_existing_values` 只在已有角色且字段非空时追加：
```
【该角色当前已设定的信息（供参考）】
性格特征: {personality}
所处场景: {scenario}
经典台词: {example_messages}
```

### 8.3 第 2 步 User Message 模板

```
基于以上分析提取的角色信息，请生成该角色的"详细人设"和"简易人设"。

已提取的信息：
- 性格特征: {personality}
- 所处场景: {scenario}
- 经典台词: {example_messages}
- 补充说明: {creator_notes}

输出格式要求：
<detailed_persona>
（详细人设，直接注入 System Prompt 的完整设定，2000字以内）
</detailed_persona>

<simplified_persona>
（简易人设，给其他角色看的简介，50字以内，以一两句话客观角度简单描述该角色的身份信息）
</simplified_persona>
```

---

## 9. 数据库变更

**本次无需 migration**。`first_message` 和 `tags` 保留但不用；`personality`、`scenario`、`example_messages`、`creator_notes` 已在 V1 schema 中存在。

**需要修复的后端代码**:
- `db/agent.rs` 的 `create_agent`：目前只写了 `personality` 和 `scenario`，需要把 `example_messages` 和 `creator_notes` 也加入 INSERT 语句
- `db/agent.rs` 的 `update_agent`：目前只更新了 `personality` 和 `scenario`，需要把 `example_messages` 和 `creator_notes` 也加入 UPDATE 语句
- `models/agent.rs` 的 `CreateAgentRequest` 和 `UpdateAgentRequest`：已包含这些字段，但 repository 层未使用

---

## 10. 功能列表更新

实现完成后，更新 `docs/feature_list.md`：
- AGT-13 "AI 辅助生成人设" 状态改为 ✅ 已实现
- 新增 CHAT-23 子项或独立项："websearch 工具支持"（P2，待实现）

---

## 11. 风险与限制

1. **LLM 调用不可中断**: 前端关闭弹窗无法中断后端已发出的 HTTP 请求，后端会继续执行完成，结果会被丢弃。
2. **模型搜索能力不确定**: 提示词中建议搜索，但模型是否实际搜索完全取决于模型自身能力，后端无法验证。
3. **原创角色生成质量**: 对于完全原创且不在任何模型训练数据中的角色，第一步可能全部留空，第二步只能基于补充信息生成，质量依赖补充信息的丰富程度。
4. **标签解析容错**: 第 2 步的 `</>` 标签解析需具备一定容错（如处理多余的空格、换行），但仍可能因模型输出格式异常而失败。

---

*设计确认日期: 2026-05-17*  
*确认人: 用户 + OpenCode*
