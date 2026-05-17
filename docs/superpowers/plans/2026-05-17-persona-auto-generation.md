# AGT-13 人设自生成实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现人设自生成功能，支持新建角色和已有角色两种模式，通过两步 LLM 多轮对话自动生成 personality/scenario/example_messages 并回填 detailed_persona/simplified_persona。

**Architecture:** 后端新增 `generate_persona` Tauri 命令，内部调用 `llm/persona_generation.rs` 核心模块完成多轮对话（第1步工具调用 + 第2步标签解析）。前端在 `CreateAgentModal`（新建）和 `PersonaGenerateModal`（已有）中分别集成。

**Tech Stack:** Rust (Tauri v2, rusqlite, reqwest), Svelte 5, TypeScript, TailwindCSS v4

---

## 文件结构

| 文件 | 操作 | 职责 |
|------|------|------|
| `src-tauri/src/db/agent.rs` | 修改 | `create_agent`/`update_agent` SQL 加入 `example_messages` 和 `creator_notes` |
| `src-tauri/src/models/generate_persona.rs` | 创建 | `ModelConfig`、`GeneratePersonaRequest`、`GeneratePersonaResponse` |
| `src-tauri/src/models/mod.rs` | 修改 | 导出 `generate_persona` 模块 |
| `src-tauri/src/llm/tool.rs` | 修改 | 添加 `fill_character_fields_tool_schema()` |
| `src-tauri/src/llm/persona_generation.rs` | 创建 | 核心：prompt 构建、工具执行、标签解析、多轮对话编排 |
| `src-tauri/src/llm/mod.rs` | 修改 | 添加 `persona_generation` 模块 |
| `src-tauri/src/commands/generate_persona.rs` | 创建 | Tauri `generate_persona` 命令处理器 |
| `src-tauri/src/commands/mod.rs` | 修改 | 导出 `generate_persona` 模块 |
| `src-tauri/src/lib.rs` | 修改 | 注册 `generate_persona` 命令 |
| `src/lib/types.ts` | 修改 | 添加 `GeneratePersonaResult` 类型 |
| `src/lib/components/PersonaGenerateModal.svelte` | 重构 | 已有角色的自生成弹窗（表单 + 生成状态 + 退出提示） |
| `src/lib/components/CreateAgentModal.svelte` | 修改 | 启用自生成按钮，接入 `generate_persona` 命令 |
| `src/lib/components/AgentDetail.svelte` | 修改 | 已有角色集成 `PersonaGenerateModal`，处理 `onGenerated` 回调 |
| `docs/feature_list.md` | 修改 | AGT-13 标记为已实现 |

---

### Task 1: 修复数据库层 — 让 create/update_agent 写入所有预留字段

**Files:**
- Modify: `src-tauri/src/db/agent.rs`
- Test: `cargo check`

当前 `create_agent` 和 `update_agent` 的 SQL 只处理了 `personality` 和 `scenario`，`example_messages` 和 `creator_notes` 虽然在 `CreateAgentRequest`/`UpdateAgentRequest` 中有定义，但从未写入数据库。必须先修复，否则人设自生成第1步的结果无法持久化。

- [ ] **Step 1: 修改 `create_agent` 的 INSERT 语句加入缺失字段**

```rust
// src-tauri/src/db/agent.rs
// 修改 conn.execute 的第一个参数（SQL）和第二个参数（tuple）

conn.execute(
    r#"INSERT INTO agents (
        id, name, avatar_path, detailed_persona, simplified_persona,
        personality, scenario, example_messages, first_message, creator_notes, tags,
        model_provider, model_name, base_url,
        temperature, max_tokens, api_key_encrypted, thinking_mode, created_at, updated_at
    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)"#,
    (
        &id, &req.name, &req.avatar_path, &req.detailed_persona, &req.simplified_persona,
        &req.personality, &req.scenario, &req.example_messages, &req.first_message, &req.creator_notes, &req.tags,
        &req.model_provider, &req.model_name, &req.base_url,
        req.temperature.unwrap_or(0.7), req.max_tokens.unwrap_or(2048),
        &api_key_encrypted, req.thinking_mode.unwrap_or(false) as i32, now, now,
    ),
)?;
```

- [ ] **Step 2: 修改 `update_agent` 的 UPDATE 语句加入缺失字段**

```rust
// src-tauri/src/db/agent.rs
// 修改 conn.execute 的第一个参数（SQL）和第二个参数（tuple）

conn.execute(
    r#"UPDATE agents SET
        name = COALESCE(?2, name),
        avatar_path = COALESCE(?3, avatar_path),
        detailed_persona = COALESCE(?4, detailed_persona),
        simplified_persona = COALESCE(?5, simplified_persona),
        personality = COALESCE(?6, personality),
        scenario = COALESCE(?7, scenario),
        example_messages = COALESCE(?8, example_messages),
        first_message = COALESCE(?9, first_message),
        creator_notes = COALESCE(?10, creator_notes),
        tags = COALESCE(?11, tags),
        model_provider = COALESCE(?12, model_provider),
        model_name = COALESCE(?13, model_name),
        base_url = COALESCE(?14, base_url),
        temperature = COALESCE(?15, temperature),
        max_tokens = COALESCE(?16, max_tokens),
        api_key_encrypted = COALESCE(?17, api_key_encrypted),
        thinking_mode = COALESCE(?18, thinking_mode),
        updated_at = ?19
    WHERE id = ?1 AND is_deleted = 0"#,
    (
        &req.id, &req.name, &req.avatar_path, &req.detailed_persona, &req.simplified_persona,
        &req.personality, &req.scenario, &req.example_messages, &req.first_message, &req.creator_notes, &req.tags,
        &req.model_provider, &req.model_name, &req.base_url,
        req.temperature, req.max_tokens,
        api_key_encrypted,
        req.thinking_mode.map(|v| v as i32),
        now,
    ),
)?;
```

- [ ] **Step 3: 编译验证**

Run: `cd src-tauri && cargo check`
Expected: PASS（无编译错误）

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db/agent.rs
git commit -m "fix(db): include example_messages and creator_notes in create/update_agent"
```

---

### Task 2: 定义 generate_persona 请求/响应模型

**Files:**
- Create: `src-tauri/src/models/generate_persona.rs`
- Modify: `src-tauri/src/models/mod.rs`
- Test: `cargo check`

- [ ] **Step 1: 创建 generate_persona.rs**

```rust
// src-tauri/src/models/generate_persona.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct ModelConfig {
    pub model_provider: String,
    pub model_name: String,
    pub base_url: Option<String>,
    pub api_key: String,
    pub temperature: f64,
    pub max_tokens: i32,
    pub thinking_mode: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GeneratePersonaRequest {
    pub agent_id: Option<String>,
    pub model_config: Option<ModelConfig>,
    pub reference_character: Option<String>,
    pub supplement: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeneratePersonaResponse {
    pub personality: Option<String>,
    pub scenario: Option<String>,
    pub example_messages: Option<String>,
    pub creator_notes: Option<String>,
    pub detailed_persona: String,
    pub simplified_persona: String,
}
```

- [ ] **Step 2: 修改 models/mod.rs 导出**

```rust
// src-tauri/src/models/mod.rs
pub mod agent;
pub mod agent_relationship;
pub mod chat_page;
pub mod generate_persona;
pub mod message;
pub mod session;
pub mod settings;
pub mod user_persona;
```

- [ ] **Step 3: 编译验证**

Run: `cd src-tauri && cargo check`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/models/generate_persona.rs src-tauri/src/models/mod.rs
git commit -m "feat(models): add GeneratePersonaRequest/Response and ModelConfig"
```

---

### Task 3: 添加 fill_character_fields 工具 schema

**Files:**
- Modify: `src-tauri/src/llm/tool.rs`
- Test: `cargo check`

- [ ] **Step 1: 在 tool.rs 中添加工具 schema 函数**

```rust
// src-tauri/src/llm/tool.rs
// 在 update_relationship_tool_schema 之后添加：

pub fn fill_character_fields_tool_schema() -> serde_json::Value {
    serde_json::json!({
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
    })
}
```

- [ ] **Step 2: 编译验证**

Run: `cd src-tauri && cargo check`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/llm/tool.rs
git commit -m "feat(tool): add fill_character_fields tool schema for persona generation"
```

---

### Task 4: 实现 persona_generation 核心模块

**Files:**
- Create: `src-tauri/src/llm/persona_generation.rs`
- Modify: `src-tauri/src/llm/mod.rs`
- Test: `cargo check`

这是整个功能的核心。模块职责：构建两步 prompt、执行多轮对话、解析工具调用和 XML 标签、与数据库交互。

- [ ] **Step 1: 创建 persona_generation.rs 并添加导入和常量**

```rust
// src-tauri/src/llm/persona_generation.rs
use crate::db::connection::DbState;
use crate::llm::openai::OpenAiCompatibleProvider;
use crate::llm::provider::LlmProvider;
use crate::llm::tool::{fill_character_fields_tool_schema, LlmResponse, ToolCall};
use crate::models::generate_persona::{GeneratePersonaRequest, GeneratePersonaResponse, ModelConfig};
use std::sync::Arc;
use tokio::sync::Mutex;

const SYSTEM_PROMPT_STEP1: &str = r#"你是一个专业的角色设定分析师。你的任务是根据用户提供的参考角色信息和补充内容，提取并结构化角色的核心设定信息。

如果你具备网络搜索能力，建议先搜索该参考角色的详细信息（尤其是性格设定、世界观背景和经典台词），以提高分析准确性。

你需要调用 fill_character_fields 工具，将分析结果填入以下字段：
- personality: 性格特征描述
- scenario: 所处世界观/场景
- example_messages: 经典台词或代表性对话

如果该角色不在你的知识库中（如原创角色），或某项信息无法确定，可将对应字段设为空字符串。"#;

fn build_step1_user_message(
    reference: Option<&str>,
    supplement: Option<&str>,
    existing: &(Option<String>, Option<String>, Option<String>),
) -> String {
    let mut msg = String::new();
    if let Some(r) = reference {
        msg.push_str(&format!("【参考角色】\n{}\n\n", r));
    }
    if let Some(s) = supplement {
        msg.push_str(&format!("【补充信息】\n{}\n\n", s));
    }
    let (ref_p, ref_s, ref_e) = existing;
    let has_existing = ref_p.is_some() || ref_s.is_some() || ref_e.is_some();
    if has_existing {
        msg.push_str("【该角色当前已设定的信息（供参考，你可选择保留、修改或清空）】\n");
        if let Some(p) = ref_p {
            msg.push_str(&format!("性格特征: {}\n", p));
        }
        if let Some(s) = ref_s {
            msg.push_str(&format!("所处场景: {}\n", s));
        }
        if let Some(e) = ref_e {
            msg.push_str(&format!("经典台词: {}\n", e));
        }
        msg.push('\n');
    }
    msg.push_str("请分析以上信息，调用 fill_character_fields 工具填写角色设定字段。");
    msg
}

fn build_step2_user_message(
    personality: &str,
    scenario: &str,
    example_messages: &str,
    creator_notes: &str,
) -> String {
    format!(r#"基于以上分析提取的角色信息，请生成该角色的"详细人设"和"简易人设"。

已提取的信息：
- 性格特征: {}
- 所处场景: {}
- 经典台词: {}
- 补充说明: {}

输出格式要求：
<detailed_persona>
（详细人设，直接注入 System Prompt 的完整设定，2000字以内）
</detailed_persona>

<simplified_persona>
（简易人设，给其他角色看的简介，50字以内，以一两句话客观角度简单描述该角色的身份信息）
</simplified_persona>

注意：
1. 必须包含 <detailed_persona> 和 </simplified_persona> 标签
2. 标签之间不要添加其他说明文字
3. 如果参考角色信息不足，可基于补充信息和你的知识合理发挥"#,
        personality, scenario, example_messages, creator_notes
    )
}

fn parse_persona_tags(content: &str) -> Result<(String, String), String> {
    let detailed_re = regex::Regex::new(r"<detailed_persona>\s*(.*?)\s*</detailed_persona>").unwrap();
    let simplified_re = regex::Regex::new(r"<simplified_persona>\s*(.*?)\s*</simplified_persona>").unwrap();

    let detailed = detailed_re
        .captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .ok_or_else(|| "未找到 <detailed_persona> 标签".to_string())?;

    let simplified = simplified_re
        .captures(content)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .ok_or_else(|| "未找到 <simplified_persona> 标签".to_string())?;

    if detailed.is_empty() {
        return Err("<detailed_persona> 内容为空".to_string());
    }
    if simplified.is_empty() {
        return Err("<simplified_persona> 内容为空".to_string());
    }

    Ok((detailed, simplified))
}

fn provider_from_config(cfg: &ModelConfig) -> OpenAiCompatibleProvider {
    OpenAiCompatibleProvider::new(
        cfg.api_key.clone(),
        cfg.base_url.clone(),
        cfg.model_name.clone(),
        cfg.temperature,
        cfg.max_tokens,
    )
}
```

- [ ] **Step 2: 添加 generate 主函数（上半部分：参数校验和第1轮）**

```rust
// 继续添加到 persona_generation.rs

pub async fn generate(
    db_state: &DbState,
    req: &GeneratePersonaRequest,
) -> Result<GeneratePersonaResponse, String> {
    // 1. 校验参数
    let has_agent = req.agent_id.is_some();
    let has_model = req.model_config.is_some();
    if has_agent == has_model {
        return Err("必须且只能传 agent_id 或 model_config 中的一个".to_string());
    }
    let has_reference = req.reference_character.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
    let has_supplement = req.supplement.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
    if !has_reference && !has_supplement {
        return Err("参考角色和补充信息至少填写一项".to_string());
    }

    // 2. 获取模型配置
    let model_config = if let Some(ref id) = req.agent_id {
        let conn = db_state.0.lock().await;
        let agent = crate::db::agent::get_by_id(&conn, id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "角色不存在".to_string())?;
        if agent.model_name.is_none() || agent.model_provider.is_none() {
            return Err("该角色未配置模型信息".to_string());
        }
        let api_key_encrypted = agent.api_key_encrypted.ok_or("该角色未配置 API Key")?;
        let api_key = crate::crypto::decrypt(&api_key_encrypted)
            .map_err(|e| format!("解密 API Key 失败: {}", e))?;
        ModelConfig {
            model_provider: agent.model_provider.unwrap(),
            model_name: agent.model_name.unwrap(),
            base_url: agent.base_url,
            api_key,
            temperature: agent.temperature,
            max_tokens: agent.max_tokens,
            thinking_mode: agent.thinking_mode,
        }
    } else {
        req.model_config.clone().unwrap()
    };

    let provider = provider_from_config(&model_config);

    // 3. 读取现有值（已有角色）
    let existing: (Option<String>, Option<String>, Option<String>) = if let Some(ref id) = req.agent_id {
        let conn = db_state.0.lock().await;
        let agent = crate::db::agent::get_by_id(&conn, id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "角色不存在".to_string())?;
        (agent.personality, agent.scenario, agent.example_messages)
    } else {
        (None, None, None)
    };

    // 4. 前置写入 creator_notes
    let creator_notes = req.supplement.clone().unwrap_or_default();
    if let Some(ref id) = req.agent_id {
        if !creator_notes.is_empty() {
            let conn = db_state.0.lock().await;
            let update_req = crate::models::agent::UpdateAgentRequest {
                id: id.clone(),
                creator_notes: Some(Some(creator_notes.clone())),
                ..Default::default()
            };
            crate::db::agent::update(&conn, &update_req).map_err(|e| e.to_string())?;
        }
    }

    // 5. 第 1 轮：信息提取
    let step1_user_msg = build_step1_user_message(
        req.reference_character.as_deref(),
        req.supplement.as_deref(),
        &existing,
    );

    let step1_messages = vec![serde_json::json!({
        "role": "user",
        "content": step1_user_msg,
    })];

    let tools = vec![fill_character_fields_tool_schema()];

    let response1 = provider.chat(SYSTEM_PROMPT_STEP1, step1_messages, tools).await?;

    // 6. 解析工具调用
    let (personality, scenario, example_messages) = if let Some(tc) = response1.tool_calls.first() {
        if tc.name != "fill_character_fields" {
            return Err(format!("第1步 AI 调用了未预期的工具: {}", tc.name));
        }
        let args: serde_json::Value = serde_json::from_str(&tc.arguments)
            .map_err(|e| format!("工具参数解析失败: {}", e))?;
        (
            args["personality"].as_str().unwrap_or("").to_string(),
            args["scenario"].as_str().unwrap_or("").to_string(),
            args["example_messages"].as_str().unwrap_or("").to_string(),
        )
    } else {
        return Err("第1步 AI 未调用 fill_character_fields 工具".to_string());
    };

    // 7. 写入数据库（已有角色）
    if let Some(ref id) = req.agent_id {
        let conn = db_state.0.lock().await;
        let update_req = crate::models::agent::UpdateAgentRequest {
            id: id.clone(),
            personality: Some(Some(personality.clone())),
            scenario: Some(Some(scenario.clone())),
            example_messages: Some(Some(example_messages.clone())),
            ..Default::default()
        };
        crate::db::agent::update(&conn, &update_req).map_err(|e| e.to_string())?;
    }

    // ... 第 2 轮将在下一步添加
    Ok(GeneratePersonaResponse {
        personality: Some(personality.clone()),
        scenario: Some(scenario.clone()),
        example_messages: Some(example_messages.clone()),
        creator_notes: Some(creator_notes.clone()),
        detailed_persona: String::new(),
        simplified_persona: String::new(),
    })
}
```

- [ ] **Step 3: 添加 generate 主函数（下半部分：第2轮和标签解析）**

```rust
// 将 Step 2 中最后面的 // ... 第 2 轮将在下一步添加 和 Ok(...) 替换为：

    // 8. 构建第 2 轮消息历史
    let mut step2_messages = vec![
        serde_json::json!({ "role": "user", "content": step1_user_msg }),
        serde_json::json!({
            "role": "assistant",
            "content": None::<String>,
            "tool_calls": [{
                "id": "call_1",
                "type": "function",
                "function": {
                    "name": "fill_character_fields",
                    "arguments": serde_json::json!({
                        "personality": &personality,
                        "scenario": &scenario,
                        "example_messages": &example_messages,
                    }).to_string(),
                }
            }],
        }),
        serde_json::json!({
            "role": "tool",
            "tool_call_id": "call_1",
            "content": "字段已更新",
        }),
        serde_json::json!({
            "role": "user",
            "content": build_step2_user_message(&personality, &scenario, &example_messages, &creator_notes),
        }),
    ];

    // 9. 第 2 轮：人设生成（不传 tools，强制直接输出）
    let response2 = provider.chat(SYSTEM_PROMPT_STEP1, step2_messages, vec![]).await?;

    let content2 = response2.content.ok_or_else(|| "第2步 AI 未返回内容".to_string())?;

    let (detailed_persona, simplified_persona) = parse_persona_tags(&content2)?;

    Ok(GeneratePersonaResponse {
        personality: Some(personality),
        scenario: Some(scenario),
        example_messages: Some(example_messages),
        creator_notes: Some(creator_notes),
        detailed_persona,
        simplified_persona,
    })
```

- [ ] **Step 4: 修改 llm/mod.rs 添加模块**

```rust
// src-tauri/src/llm/mod.rs
pub mod history_prompt;
pub mod openai;
pub mod persona_generation;
pub mod prompt;
pub mod prompt_templates;
pub mod provider;
pub mod tool;
```

- [ ] **Step 5: 添加 regex 依赖并编译验证**

 persona_generation.rs 使用了 `regex` crate，需要确认它已在 Cargo.toml 中。

Run: `cd src-tauri && grep regex Cargo.toml`
如果未找到，添加：
```toml
regex = "1.10"
```
然后运行 `cargo check`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/llm/persona_generation.rs src-tauri/src/llm/mod.rs src-tauri/Cargo.toml
git commit -m "feat(llm): implement persona_generation core module with two-step multi-turn dialog"
```

---

### Task 5: 实现 generate_persona Tauri 命令

**Files:**
- Create: `src-tauri/src/commands/generate_persona.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Test: `cargo check`

- [ ] **Step 1: 创建 generate_persona.rs**

```rust
// src-tauri/src/commands/generate_persona.rs
use crate::db::connection::DbState;
use crate::llm::persona_generation;
use crate::models::generate_persona::{GeneratePersonaRequest, GeneratePersonaResponse};

#[tauri::command]
pub async fn generate_persona(
    db_state: tauri::State<'_, DbState>,
    req: GeneratePersonaRequest,
) -> Result<GeneratePersonaResponse, String> {
    crate::logger::backend("DEBUG", &format!(
        "[DEBUG generate_persona] agent_id={:?}, has_ref={}, has_supp={}",
        req.agent_id,
        req.reference_character.as_ref().map(|s| !s.is_empty()).unwrap_or(false),
        req.supplement.as_ref().map(|s| !s.is_empty()).unwrap_or(false),
    ));

    let result = persona_generation::generate(&db_state, &req).await;

    match &result {
        Ok(r) => crate::logger::backend("DEBUG", &format!(
            "[DEBUG generate_persona] success detailed_len={} simplified_len={}",
            r.detailed_persona.len(),
            r.simplified_persona.len(),
        )),
        Err(e) => crate::logger::backend("ERROR", &format!("[DEBUG generate_persona] failed: {}", e)),
    }

    result
}
```

- [ ] **Step 2: 修改 commands/mod.rs**

```rust
// src-tauri/src/commands/mod.rs
pub mod agent;
pub mod agent_relationship;
pub mod generate_persona;
pub mod log;
pub mod message;
pub mod session;
pub mod settings;
pub mod upload;
pub mod user_persona;
```

- [ ] **Step 3: 编译验证**

Run: `cd src-tauri && cargo check`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/generate_persona.rs src-tauri/src/commands/mod.rs
git commit -m "feat(commands): add generate_persona tauri command"
```

---

### Task 6: 注册命令并编译验证

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Test: `cargo check --tests`

- [ ] **Step 1: 修改 lib.rs 导入和注册**

```rust
// src-tauri/src/lib.rs
// 在现有 use commands::... 之后添加：
use commands::generate_persona::generate_persona;

// 在 tauri::generate_handler![...] 中添加 generate_persona,
```

- [ ] **Step 2: 编译验证**

Run: `cd src-tauri && cargo check`
Expected: PASS

Run: `cd src-tauri && cargo check --tests`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(lib): register generate_persona command in tauri handler"
```

---

### Task 7: 重构前端 PersonaGenerateModal（已有角色用）

**Files:**
- Modify: `src/lib/types.ts`
- Modify: `src/lib/components/PersonaGenerateModal.svelte`
- Test: `npx svelte-check --tsconfig ./tsconfig.json`

- [ ] **Step 1: 在 types.ts 中添加 GeneratePersonaResult**

```typescript
// src/lib/types.ts
// 在 RelationshipItem 之后添加：

export interface GeneratePersonaResult {
    personality: string | null;
    scenario: string | null;
    example_messages: string | null;
    creator_notes: string | null;
    detailed_persona: string;
    simplified_persona: string;
}
```

- [ ] **Step 2: 重构 PersonaGenerateModal.svelte**

```svelte
<!-- src/lib/components/PersonaGenerateModal.svelte -->
<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { X, Loader2, Sparkles } from 'lucide-svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { logger } from '$lib/logger';
    import type { GeneratePersonaResult } from '$lib/types';

    interface Props {
        open: boolean;
        agentId?: string;
        onClose: () => void;
        onGenerated: (result: GeneratePersonaResult) => void;
    }

    let { open, agentId, onClose, onGenerated }: Props = $props();

    let referenceCharacter = $state('');
    let supplement = $state('');
    let generating = $state(false);

    function handleClose() {
        if (generating) {
            if (!confirm('退出将会打断生成，确定要退出吗？')) {
                return;
            }
        }
        onClose();
    }

    async function handleGenerate() {
        const hasRef = referenceCharacter.trim().length > 0;
        const hasSupp = supplement.trim().length > 0;
        if (!hasRef && !hasSupp) {
            toastStore.show('参考角色和补充信息至少填写一项', 'error', 3000);
            return;
        }

        generating = true;
        try {
            const result = await invoke<GeneratePersonaResult>('generate_persona', {
                req: {
                    agent_id: agentId ?? null,
                    model_config: null,
                    reference_character: referenceCharacter.trim() || null,
                    supplement: supplement.trim() || null,
                },
            });
            logger.debug('[DEBUG PersonaGenerateModal] generated', { agentId });
            onGenerated(result);
            onClose();
        } catch (err: any) {
            logger.error('Failed to generate persona:', err);
            toastStore.show('生成失败: ' + String(err), 'error', 5000);
        } finally {
            generating = false;
        }
    }
</script>

{#if open}
    <div class="fixed inset-0 bg-black/50 z-50 flex items-center justify-center" onclick={handleClose} role="dialog" aria-modal="true">
        <div class="bg-surface rounded-xl p-6 w-[28rem] shadow-xl" onclick={(e) => e.stopPropagation()}>
            <div class="flex items-center justify-between mb-4">
                <div class="flex items-center gap-2">
                    <Sparkles size={18} class="text-primary" />
                    <h3 class="font-semibold">人设自生成</h3>
                </div>
                <button onclick={handleClose} class="p-1 hover:bg-bg rounded" aria-label="关闭">
                    <X size={18} />
                </button>
            </div>

            <div class="space-y-4">
                <div>
                    <label class="block text-sm font-medium mb-1">参考角色 <span class="text-text-secondary">（可选）</span></label>
                    <input
                        type="text"
                        bind:value={referenceCharacter}
                        disabled={generating}
                        placeholder="如：Fate/stay night 中的 Saber"
                        class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 bg-surface disabled:opacity-50"
                    />
                </div>

                <div>
                    <label class="block text-sm font-medium mb-1">补充信息 <span class="text-text-secondary">（可选）</span></label>
                    <textarea
                        bind:value={supplement}
                        disabled={generating}
                        rows={4}
                        placeholder="可填写任意相关内容：设定、要求、台词、聊天记录等..."
                        class="w-full px-3 py-2 border border-border rounded-lg focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none bg-surface disabled:opacity-50"
                    ></textarea>
                </div>

                <p class="text-xs text-text-secondary">
                    参考角色和补充信息至少填写一项
                </p>
            </div>

            <div class="flex justify-end gap-3 mt-6">
                <button
                    onclick={handleClose}
                    disabled={generating}
                    class="px-4 py-2 text-text-secondary hover:bg-gray-100 rounded-lg transition-colors disabled:opacity-50"
                >
                    取消
                </button>
                <button
                    onclick={handleGenerate}
                    disabled={generating || (!referenceCharacter.trim() && !supplement.trim())}
                    class="flex items-center gap-2 px-4 py-2 bg-primary text-white rounded-lg hover:bg-primary-dark transition-colors disabled:opacity-50"
                >
                    {#if generating}
                        <Loader2 size={16} class="animate-spin" />
                        <span>生成中...</span>
                    {:else}
                        <Sparkles size={16} />
                        <span>生成</span>
                    {/if}
                </button>
            </div>
        </div>
    </div>
{/if}
```

- [ ] **Step 3: Svelte 类型检查**

Run: `npx svelte-check --tsconfig ./tsconfig.json`
Expected: 无错误（允许 a11y 警告）

- [ ] **Step 4: Commit**

```bash
git add src/lib/types.ts src/lib/components/PersonaGenerateModal.svelte
git commit -m "feat(ui): implement PersonaGenerateModal with two-field form and generation state"
```

---

### Task 8: 修改 CreateAgentModal 启用自生成（新建角色用）

**Files:**
- Modify: `src/lib/components/CreateAgentModal.svelte`
- Test: `npx svelte-check --tsconfig ./tsconfig.json`

新建角色时，自生成按钮需要从 disabled 变为可用，点击后调用 `generate_persona` 并回填表单。

- [ ] **Step 1: 修改 CreateAgentModal.svelte 添加生成逻辑**

```svelte
<!-- src/lib/components/CreateAgentModal.svelte -->
<!-- 在 <script> 顶部添加导入和状态 -->
<script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { X, Bot, Sparkles, Loader2 } from 'lucide-svelte';
    import AvatarUploadModal from './AvatarUploadModal.svelte';
    import { toastStore } from '$lib/stores/toastStore.svelte';
    import { logger } from '$lib/logger';
    import type { GeneratePersonaResult } from '$lib/types';

    let { open = $bindable(false), onSuccess }: { open: boolean; onSuccess?: () => void } = $props();

    // ... 原有状态保持不变 ...
    let showGenerateFields = $state(false);
    let referenceCharacter = $state('');
    let additionalInfo = $state('');
    let generating = $state(false);
    // ...

    // 在 handleSubmit 之后添加生成函数
    async function handleGeneratePersona() {
        const hasRef = referenceCharacter.trim().length > 0;
        const hasSupp = additionalInfo.trim().length > 0;
        if (!hasRef && !hasSupp) {
            toastStore.show('参考角色和补充信息至少填写一项', 'error', 3000);
            return;
        }
        if (!form.model_name || !form.api_key) {
            toastStore.show('请先在下方填写模型名称和 API Key', 'error', 3000);
            return;
        }

        generating = true;
        try {
            const result = await invoke<GeneratePersonaResult>('generate_persona', {
                req: {
                    agent_id: null,
                    model_config: {
                        model_provider: form.model_provider,
                        model_name: form.model_name,
                        base_url: form.base_url || null,
                        api_key: form.api_key,
                        temperature: form.temperature,
                        max_tokens: form.max_tokens,
                        thinking_mode: form.thinking_mode,
                    },
                    reference_character: referenceCharacter.trim() || null,
                    supplement: additionalInfo.trim() || null,
                },
            });
            logger.debug('[DEBUG CreateAgentModal] persona generated');
            form.detailed_persona = result.detailed_persona;
            form.simplified_persona = result.simplified_persona;
            toastStore.show('人设生成完成', 'success', 2000);
        } catch (err: any) {
            logger.error('Failed to generate persona:', err);
            toastStore.show('生成失败: ' + String(err), 'error', 5000);
        } finally {
            generating = false;
        }
    }
</script>
```

- [ ] **Step 2: 修改模板中的生成按钮**

```svelte
<!-- 替换原有 disabled 的生成按钮 -->
<button
    type="button"
    onclick={handleGeneratePersona}
    disabled={generating}
    class="flex items-center gap-2 px-4 py-2 bg-primary text-white rounded-lg text-sm hover:bg-primary-dark transition-colors disabled:opacity-50"
>
    {#if generating}
        <Loader2 size={16} class="animate-spin" />
        <span>生成中...</span>
    {:else}
        <Sparkles size={16} />
        <span>生成</span>
    {/if}
</button>
```

- [ ] **Step 3: Svelte 类型检查**

Run: `npx svelte-check --tsconfig ./tsconfig.json`
Expected: 无错误

- [ ] **Step 4: Commit**

```bash
git add src/lib/components/CreateAgentModal.svelte
git commit -m "feat(ui): enable persona generation in CreateAgentModal for new agents"
```

---

### Task 9: 修改 AgentDetail 集成 PersonaGenerateModal

**Files:**
- Modify: `src/lib/components/AgentDetail.svelte`
- Test: `npx svelte-check --tsconfig ./tsconfig.json`

- [ ] **Step 1: 修改 AgentDetail.svelte 的导入和回调**

```svelte
<!-- src/lib/components/AgentDetail.svelte -->
<!-- 在 <script> 中修改 onGenerated 回调 -->

function handleGenerated(result: import('$lib/types').GeneratePersonaResult) {
    if (!agent) return;
    form.detailed_persona = result.detailed_persona;
    form.simplified_persona = result.simplified_persona;
    // 如果后端返回了 personality 等字段（理论上已有角色也会返回当前值）
    // 当前前端没有这些字段的输入框，所以只回填 visible 的 detailed/simplified
    showGenerateModal = false;
    toastStore.show('人设生成完成，请检查并保存', 'success', 3000);
}
```

- [ ] **Step 2: 修改 PersonaGenerateModal 的调用，传入 agentId**

```svelte
<!-- 修改模板中 PersonaGenerateModal 的使用 -->
<PersonaGenerateModal
    open={showGenerateModal}
    agentId={agent?.id}
    onClose={() => showGenerateModal = false}
    onGenerated={handleGenerated}
/>
```

- [ ] **Step 3: Svelte 类型检查**

Run: `npx svelte-check --tsconfig ./tsconfig.json`
Expected: 无错误

- [ ] **Step 4: Commit**

```bash
git add src/lib/components/AgentDetail.svelte
git commit -m "feat(ui): wire PersonaGenerateModal into AgentDetail with onGenerated callback"
```

---

### Task 10: 更新功能列表并做最终验证

**Files:**
- Modify: `docs/feature_list.md`

- [ ] **Step 1: 更新 feature_list.md**

将 AGT-13 行状态从 `⬜ 待实现` 改为 `✅ 已实现`。
在适当位置新增 websearch 相关的待实现项（如 CHAT-23 扩展或新增项）。

- [ ] **Step 2: 最终编译验证**

Run: `cd src-tauri && cargo check`
Expected: PASS

Run: `cd src-tauri && cargo check --tests`
Expected: PASS

Run: `npx svelte-check --tsconfig ./tsconfig.json`
Expected: 无错误

- [ ] **Step 3: Commit**

```bash
git add docs/feature_list.md
git commit -m "docs: mark AGT-13 as implemented in feature list"
```

---

## Self-Review Checklist

### 1. Spec Coverage

| 设计文档要求 | 对应 Task |
|-------------|----------|
| 两步多轮对话 | Task 4 |
| 第1步工具调用 fill_character_fields | Task 3 + Task 4 |
| 第2步 </> 标签输出 | Task 4 |
| 新建角色传 model_config | Task 8 |
| 已有角色传 agent_id | Task 7 + Task 9 |
| creator_notes 前置写入 | Task 4 |
| 第一步旧值参考 | Task 4 (build_step1_user_message) |
| 第一步结果写入 DB（已有角色） | Task 4 |
| 第二步结果不入库，返回前端 | Task 4 + Task 7 + Task 8 |
| 前端退出提示 | Task 7 |
| 新建角色模型配置校验 | Task 8 |
| 数据库修复 example_messages/creator_notes | Task 1 |

### 2. Placeholder Scan

- ✅ 无 TBD/TODO
- ✅ 所有 SQL 都是完整语句
- ✅ 所有函数都有完整实现代码
- ✅ 所有类型字段都已定义

### 3. Type Consistency

- ✅ `GeneratePersonaRequest` / `GeneratePersonaResponse` 在 models、commands、persona_generation 中一致
- ✅ `ModelConfig` 字段与前端表单字段一致
- ✅ `GeneratePersonaResult` 前端类型与后端 `GeneratePersonaResponse` 字段一致

---

## 执行选项

**Plan complete and saved to `docs/superpowers/plans/2026-05-17-persona-auto-generation.md`. Two execution options:**

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
