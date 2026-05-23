use crate::db::connection::DbState;
use crate::llm::openai::OpenAiCompatibleProvider;
use crate::llm::provider::LlmProvider;
use crate::llm::tool::fill_character_fields_tool_schema;
use crate::models::generate_persona::{GeneratePersonaRequest, GeneratePersonaResponse, ModelConfig};
use once_cell::sync::Lazy;

static DETAILED_PERSONA_RE: Lazy<regex::Regex> = Lazy::new(|| {
    regex::Regex::new(r"(?s)<detailed_persona>\s*(.*?)\s*</detailed_persona>").unwrap()
});

static SIMPLIFIED_PERSONA_RE: Lazy<regex::Regex> = Lazy::new(|| {
    regex::Regex::new(r"(?s)<simplified_persona>\s*(.*?)\s*</simplified_persona>").unwrap()
});

const SYSTEM_PROMPT_STEP2: &str = r#"你是一个专业的角色设定创作师。你的任务是根据已提取的角色信息，生成高质量的"详细人设"和"简易人设"。

【绝对严格的输出格式要求】
你必须使用以下两个 XML 标签包裹对应内容，标签名必须完全匹配，不能有任何拼写错误：
输出模板：
```
<detailed_persona>详细人设内容</detailed_persona>
<simplified_persona>简易人设内容</simplified_persona>
```

【字数限制（硬性要求）】
- <detailed_persona> 内的内容必须控制在 2000 个汉字以内
- <simplified_persona> 内的内容必须控制在 50 个汉字以内，用一两句话客观描述角色身份，仅说明是谁即可，**禁止在这里描述性格、品行等内容**。
simplified_persona示例1：Fate系列中的Saber，英灵，职介是剑阶。
simplified_persona示例1：xxx公司的员工abc，男。

如果你之前的输出格式有误或字数超限，我会指出错误，请你修正后重新输出。修正时只需要输出修正后的完整标签内容，不要道歉或解释。"#;

const SYSTEM_PROMPT_STEP1: &str = r#"你是一个专业的角色设定分析师。你的任务是根据用户提供的参考角色信息和补充内容，提取并结构化角色的核心设定信息。

如果你具备网络搜索能力，建议先搜索该参考角色的详细信息（尤其是性格设定、世界观背景和经典台词），以提高分析准确性。

你需要调用 fill_character_fields 工具，将分析结果填入以下字段：
- personality: 性格特征描述
- scenario: 所处世界观/场景
- example_messages: 经典台词或代表性对话

【优先级规则】
如果"参考角色"和"补充信息"中的描述存在冲突，**必须优先采用"补充信息"中的内容**。补充信息是用户明确给出的设定要求，高于模型自身的知识库和搜索结果。

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

【优先级规则 — 非常重要】
如果已提取的信息之间存在冲突，或者与参考角色的已知设定不一致，**必须优先采用"补充说明"中的内容**。补充说明是用户明确给出的设定要求，具有最高优先级，高于模型自身的知识库和参考角色的原始设定。

【输出格式要求 — 必须严格遵守】
<detailed_persona>
（详细人设内容，直接注入 System Prompt 的完整设定，控制在2000个汉字以内）
</detailed_persona>

<simplified_persona>
（简易人设内容，给其他角色看的简介，必须控制在50个汉字以内，以一两句话客观角度简单描述该角色的身份信息）
</simplified_persona>

【硬性规则】
1. 必须包含 <detailed_persona>...</detailed_persona> 和 <simplified_persona>...</simplified_persona> 标签对
2. 标签之间不要添加其他说明文字
3. <simplified_persona> 的内容绝对不可超过50个汉字
4. 如果参考角色信息不足，可基于补充信息和你的知识合理发挥"#,
        personality, scenario, example_messages, creator_notes
    )
}

fn log_llm_call(step: &str, attempt: usize, system: &str, messages: &[serde_json::Value]) {
    let messages_json = serde_json::to_string(messages).unwrap_or_else(|_| "[序列化失败]".to_string());
    crate::logger::debug(&format!(
        "[DEBUG persona_generation] {} attempt={} SYSTEM_PROMPT={} MESSAGES={}",
        step, attempt, system, messages_json
    ));
}

fn log_llm_response(step: &str, attempt: usize, content: &str, tool_calls_count: usize) {
    crate::logger::debug(&format!(
        "[DEBUG persona_generation] {} attempt={} content_len={} tool_calls={} CONTENT={}",
        step, attempt, content.len(), tool_calls_count, content
    ));
}

fn parse_persona_tags(content: &str) -> Result<(String, String), String> {
    // 1. 先移除 <think>...</think> 思考标签内的所有内容
    static THINK_RE: Lazy<regex::Regex> = Lazy::new(|| {
        regex::Regex::new(r"(?s)<think>.*?</think>").unwrap()
    });
    let cleaned = THINK_RE.replace_all(content, "");

    // 2. 提取最后一个 <detailed_persona> 标签对（正式输出通常在最后）
    let detailed = DETAILED_PERSONA_RE
        .captures_iter(&cleaned)
        .filter_map(|c| c.get(1))
        .map(|m| m.as_str().trim())
        .last()
        .map(|s| s.to_string())
        .ok_or_else(|| "未找到 <detailed_persona> 标签".to_string())?;

    // 3. 提取最后一个 <simplified_persona> 标签对
    let simplified = SIMPLIFIED_PERSONA_RE
        .captures_iter(&cleaned)
        .filter_map(|c| c.get(1))
        .map(|m| m.as_str().trim())
        .last()
        .map(|s| s.to_string())
        .ok_or_else(|| "未找到 <simplified_persona> 标签".to_string())?;

    if detailed.is_empty() {
        return Err("<detailed_persona> 内容为空".to_string());
    }
    if simplified.is_empty() {
        return Err("<simplified_persona> 内容为空".to_string());
    }
    if detailed.chars().count() > 2000 {
        return Err("<detailed_persona> 内容超过 2000 字限制".to_string());
    }
    if simplified.chars().count() > 50 {
        return Err("<simplified_persona> 内容超过 50 字限制".to_string());
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

pub async fn generate(
    db_state: &DbState,
    req: &GeneratePersonaRequest,
) -> Result<GeneratePersonaResponse, String> {
    // 1. Parameter validation
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

    // 2. Get model config
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

    // 3. Read existing values (for existing agents)
    let existing: (Option<String>, Option<String>, Option<String>) = if let Some(ref id) = req.agent_id {
        let conn = db_state.0.lock().await;
        let agent = crate::db::agent::get_by_id(&conn, id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "角色不存在".to_string())?;
        (agent.personality, agent.scenario, agent.example_messages)
    } else {
        (None, None, None)
    };

    // 4. Pre-write creator_notes
    let creator_notes = req.supplement.clone().unwrap_or_default();
    if let Some(ref id) = req.agent_id {
        if !creator_notes.is_empty() {
            let conn = db_state.0.lock().await;
            let update_req = crate::models::agent::UpdateAgentRequest {
                id: id.clone(),
                creator_notes: Some(creator_notes.clone()),
                ..Default::default()
            };
            crate::db::agent::update(&conn, &update_req).map_err(|e| e.to_string())?;
        }
    }

    // 5. Step 1: Information extraction
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

    log_llm_call("Step1", 1, SYSTEM_PROMPT_STEP1, &step1_messages);

    let response1 = provider.chat(SYSTEM_PROMPT_STEP1, step1_messages, tools).await?;

    let content1 = response1.content.as_deref().unwrap_or("");
    log_llm_response("Step1", 1, content1, response1.tool_calls.len());
    for (i, tc) in response1.tool_calls.iter().enumerate() {
        crate::logger::debug(&format!(
            "[DEBUG persona_generation] Step1 tool_call[{}]: name={} args={}",
            i, tc.name, tc.arguments
        ));
    }

    // 6. Parse tool call
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

    // 7. Write to DB (existing agent only)
    if let Some(ref id) = req.agent_id {
        let conn = db_state.0.lock().await;
        let update_req = crate::models::agent::UpdateAgentRequest {
            id: id.clone(),
            personality: Some(personality.clone()),
            scenario: Some(scenario.clone()),
            example_messages: Some(example_messages.clone()),
            ..Default::default()
        };
        crate::db::agent::update(&conn, &update_req).map_err(|e| e.to_string())?;
    }

    // 8. Build step 2 message history
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

    // 9. Step 2: Persona generation with retry loop
    const MAX_STEP2_RETRIES: usize = 2;
    let mut step2_attempt = 0;
    let (detailed_persona, simplified_persona) = loop {
        log_llm_call("Step2", step2_attempt + 1, SYSTEM_PROMPT_STEP2, &step2_messages);

        let response2 = provider.chat(SYSTEM_PROMPT_STEP2, step2_messages.clone(), vec![]).await?;

        let content2 = response2.content.ok_or_else(|| {
            log_llm_response("Step2", step2_attempt + 1, "[无content]", response2.tool_calls.len());
            "第2步 AI 未返回内容".to_string()
        })?;

        log_llm_response("Step2", step2_attempt + 1, &content2, response2.tool_calls.len());

        match parse_persona_tags(&content2) {
            Ok(result) => break result,
            Err(e) => {
                step2_attempt += 1;
                if step2_attempt > MAX_STEP2_RETRIES {
                    return Err(format!(
                        "人设生成失败（第2步已重试{}次）: {}",
                        step2_attempt, e
                    ));
                }
                crate::logger::warn(&format!(
                    "[DEBUG persona_generation] Step2 attempt={} failed: {}, will retry",
                    step2_attempt, e
                ));
                // 将 AI 的错误输出和修正要求加入对话历史
                step2_messages.push(serde_json::json!({
                    "role": "assistant",
                    "content": content2,
                }));
                step2_messages.push(serde_json::json!({
                    "role": "user",
                    "content": format!(
                        "你的输出存在问题，请修正后重新输出完整内容。错误：{}。\n\n注意：\n1. 必须使用 <detailed_persona>...</detailed_persona> 和 <simplified_persona>...</simplified_persona> 标签对\n2. <simplified_persona> 的内容绝对不可超过50个汉字\n3. 直接输出修正后的完整内容，不要道歉或解释",
                        e
                    ),
                }));
            }
        }
    };

    Ok(GeneratePersonaResponse {
        personality: Some(personality),
        scenario: Some(scenario),
        example_messages: Some(example_messages),
        creator_notes: Some(creator_notes),
        detailed_persona,
        simplified_persona,
    })
}
