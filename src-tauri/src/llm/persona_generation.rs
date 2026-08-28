use crate::db::connection::DbState;
use crate::llm::openai::OpenAiCompatibleProvider;
use crate::llm::provider::LlmProvider;
use crate::llm::tool::{fill_character_fields_tool_schema, web_search_tool_schema};
use crate::models::generate_persona::{GeneratePersonaRequest, GeneratePersonaResponse, ModelConfig};
use crate::search::{SearchError, SearchProvider};
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

/// 搜索阶段（ReAct）工具调用轮数上限
const MAX_SEARCH_ROUNDS: usize = 20;

const SYSTEM_PROMPT_SEARCH: &str = r#"你是一个资料搜集助手。你的任务是通过 web_search 工具搜集角色设定资料。

【工作方式】
1. 分析用户给出的参考角色与补充信息，规划需要搜索的关键词（如角色性格、世界观、经典台词等）
2. 调用 web_search 工具进行搜索，每次只搜索一个主题；可根据已搜结果决定是否继续搜索其他角度
3. 当资料足够时，停止调用工具，直接输出一份中文资料汇总，作为最终回复

【输出要求】
- 最终回复是一份结构化的资料汇总（性格设定 / 世界观背景 / 经典台词 等分节），客观整理搜索到的信息，不要杜撰
- 如果搜索不到相关资料，如实说明"未搜索到相关资料"
- 不要在最终回复中提及工具调用过程"#;

/// 把搜索到的资料注入 Step1 用户消息
fn inject_search_material(msg: &str, search_context: Option<&str>) -> String {
    match search_context {
        Some(sc) if !sc.trim().is_empty() => {
            format!("【网络搜索资料】\n<search_material>\n{}\n</search_material>\n\n{}", sc.trim(), msg)
        }
        _ => msg.to_string(),
    }
}

/// 搜索阶段：ReAct 循环，允许多轮 web_search 工具调用，最终输出资料汇总。
/// 网络/Key/限流等致命错误会中断整个生成流程并显式返回给用户。
async fn run_search_phase(
    provider: &OpenAiCompatibleProvider,
    searcher: &dyn SearchProvider,
    reference: Option<&str>,
    supplement: Option<&str>,
    db_state: &DbState,
    usage_agent_id: &Option<String>,
    usage_model_config_id: &Option<String>,
) -> Result<String, String> {
    let mut user_msg = String::from("请搜集以下角色设定所需的资料：\n");
    if let Some(r) = reference {
        user_msg.push_str(&format!("【参考角色】{}\n", r));
    }
    if let Some(s) = supplement {
        user_msg.push_str(&format!("【补充信息】{}\n", s));
    }

    let mut messages = vec![serde_json::json!({ "role": "user", "content": user_msg })];
    let tools = vec![web_search_tool_schema()];
    let mut raw_results: Vec<String> = Vec::new();
    let mut final_summary = String::new();

    for round in 1..=MAX_SEARCH_ROUNDS {
        log_llm_call("Search", round, SYSTEM_PROMPT_SEARCH, &messages);
        let resp = provider
            .chat(SYSTEM_PROMPT_SEARCH, messages.clone(), tools.clone())
            .await
            .map_err(|e| format!("搜索阶段 AI 调用失败：{}", e))?;

        record_persona_usage(
            db_state,
            usage_agent_id,
            usage_model_config_id,
            &resp.usage,
            100 + round as i32, // 搜索阶段 call_round 从 101 起，与 Step1/2 区分
        )
        .await;

        log_llm_response("Search", round, resp.content.as_deref().unwrap_or(""), resp.tool_calls.len());

        if resp.tool_calls.is_empty() {
            final_summary = resp.content.unwrap_or_default().trim().to_string();
            break;
        }

        // 回传 assistant 消息（含 tool_calls）
        messages.push(serde_json::json!({
            "role": "assistant",
            "content": resp.content,
            "tool_calls": resp.tool_calls.iter().map(|tc| serde_json::json!({
                "id": tc.id,
                "type": "function",
                "function": { "name": tc.name, "arguments": tc.arguments },
            })).collect::<Vec<_>>(),
        }));

        for tc in &resp.tool_calls {
            let result_text = if tc.name != "web_search" {
                format!("错误：未知工具 {}，本阶段只允许调用 web_search", tc.name)
            } else {
                let query = serde_json::from_str::<serde_json::Value>(&tc.arguments)
                    .ok()
                    .and_then(|v| v["query"].as_str().map(|s| s.to_string()))
                    .unwrap_or_default();
                if query.trim().is_empty() {
                    "错误：web_search 缺少 query 参数".to_string()
                } else {
                    crate::logger::debug(&format!(
                        "[DEBUG persona_generation] Search round={} query={}", round, query
                    ));
                    match searcher.search(&query).await {
                        Ok(text) => {
                            raw_results.push(format!("【搜索：{}】\n{}", query, text));
                            text
                        }
                        Err(e) if e.is_fatal() => {
                            // 网络/Key/限流问题：中断生成，显式告知用户
                            return Err(e.user_message(searcher.display_name()));
                        }
                        Err(e) => {
                            // 非致命错误（如厂商临时错误）：作为工具结果返回，让 AI 换关键词重试
                            e.user_message(searcher.display_name())
                        }
                    }
                }
            };
            messages.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": tc.id,
                "content": result_text,
            }));
        }
    }

    if !final_summary.is_empty() {
        return Ok(final_summary);
    }
    if !raw_results.is_empty() {
        // 达到轮数上限但无汇总：拼接原始搜索结果（截断防超长）
        let joined = raw_results.join("\n\n");
        let truncated: String = joined.chars().take(6000).collect();
        return Ok(truncated);
    }
    Ok("（搜索未获得有效资料）".to_string())
}

/// 记录一次人设生成相关的 LLM 调用用量
async fn record_persona_usage(
    db_state: &DbState,
    usage_agent_id: &Option<String>,
    usage_model_config_id: &Option<String>,
    usage_json: &Option<serde_json::Value>,
    call_round: i32,
) {
    if let (Some(agent_id), Some(model_config_id), Some(usage_json)) =
        (usage_agent_id, usage_model_config_id, usage_json)
    {
        let prompt = usage_json["prompt_tokens"].as_i64().unwrap_or(0) as i32;
        let completion = usage_json["completion_tokens"].as_i64().unwrap_or(0) as i32;
        let total = usage_json["total_tokens"].as_i64().unwrap_or(0) as i32;
        let now = chrono::Utc::now().timestamp_millis();
        let record = crate::models::usage::LlmUsageRecord {
            id: format!("usage_{}_{}", agent_id, uuid::Uuid::new_v4()),
            agent_id: agent_id.clone(),
            model_config_id: model_config_id.clone(),
            session_id: None,
            trigger_type: "persona_generation".to_string(),
            call_round,
            prompt_tokens: prompt,
            completion_tokens: completion,
            total_tokens: total,
            message_id: None,
            created_at: now,
        };
        let _ = crate::db::usage::insert_usage_record(db_state, &record).await;
    }
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
    let has_model_id = req.model_config_id.is_some();
    if has_agent == has_model_id {
        return Err("必须且只能传 agent_id 或 model_config_id 中的一个".to_string());
    }
    let has_reference = req.reference_character.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
    let has_supplement = req.supplement.as_ref().map(|s| !s.is_empty()).unwrap_or(false);
    if !has_reference && !has_supplement {
        return Err("参考角色和补充信息至少填写一项".to_string());
    }

    // 2. Get model config and save IDs for usage tracking
    let usage_agent_id = req.agent_id.clone();
    let usage_model_config_id: Option<String>;
    let model_config = if let Some(ref id) = req.agent_id {
        let conn = db_state.0.lock().await;
        let agent = crate::db::agent::get_by_id(&conn, id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "角色不存在".to_string())?;
        let llm_config = crate::db::agent::resolve_llm_config(&conn, &agent)
            .map_err(|e| format!("该角色未配置模型信息: {}", e))?;
        usage_model_config_id = agent.model_config_id.clone();
        ModelConfig {
            model_provider: "openai".to_string(),
            model_name: llm_config.model_name,
            base_url: llm_config.base_url,
            api_key: llm_config.api_key,
            temperature: llm_config.temperature,
            max_tokens: llm_config.max_tokens,
        }
    } else if let Some(ref mc_id) = req.model_config_id {
        let conn = db_state.0.lock().await;
        let mc = crate::db::model_config::get_by_id(&conn, mc_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "模型配置不存在".to_string())?;
        usage_model_config_id = Some(mc_id.clone());
        let api_key = mc.api_key_encrypted
            .as_ref()
            .and_then(|enc| crate::crypto::decrypt(enc).ok())
            .ok_or("解密 API Key 失败")?;
        ModelConfig {
            model_provider: mc.provider,
            model_name: mc.model_name,
            base_url: mc.base_url,
            api_key,
            temperature: mc.temperature,
            max_tokens: mc.max_tokens,
        }
    } else {
        return Err("必须提供 agent_id 或 model_config_id".to_string());
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

    // 4.5 Search phase (optional, ReAct multi-round web_search)
    let search_context: Option<String> = if req.enable_search.unwrap_or(false) {
        let (provider_name, api_key) = {
            let conn = db_state.0.lock().await;
            let settings = crate::db::settings::get_or_create_settings(&conn)
                .map_err(|e| e.to_string())?;
            let key = settings
                .search_api_key_encrypted
                .as_ref()
                .and_then(|enc| crate::crypto::decrypt(enc).ok());
            (settings.search_provider.clone(), key)
        };
        let provider_name = provider_name
            .filter(|p| !p.is_empty())
            .ok_or_else(|| SearchError::NotConfigured.user_message(""))?;
        let api_key = api_key
            .filter(|k| !k.is_empty())
            .ok_or_else(|| SearchError::NotConfigured.user_message(""))?;
        let searcher = crate::search::create_provider(&provider_name, &api_key)
            .map_err(|e| e.user_message(&provider_name))?;
        Some(
            run_search_phase(
                &provider,
                searcher.as_ref(),
                req.reference_character.as_deref(),
                req.supplement.as_deref(),
                db_state,
                &usage_agent_id,
                &usage_model_config_id,
            )
            .await?,
        )
    } else {
        None
    };

    // 5. Step 1: Information extraction
    let step1_user_msg = inject_search_material(
        &build_step1_user_message(
            req.reference_character.as_deref(),
            req.supplement.as_deref(),
            &existing,
        ),
        search_context.as_deref(),
    );

    let step1_messages = vec![serde_json::json!({
        "role": "user",
        "content": step1_user_msg,
    })];

    let tools = vec![fill_character_fields_tool_schema()];

    log_llm_call("Step1", 1, SYSTEM_PROMPT_STEP1, &step1_messages);

    let response1 = provider.chat(SYSTEM_PROMPT_STEP1, step1_messages, tools).await?;

    // Record usage for step 1
    record_persona_usage(db_state, &usage_agent_id, &usage_model_config_id, &response1.usage, 1).await;

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

        // Record usage for step 2
        record_persona_usage(db_state, &usage_agent_id, &usage_model_config_id, &response2.usage, 2).await;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inject_search_material_prepends() {
        let msg = "【参考角色】\nSaber\n\n请分析以上信息。";
        let out = inject_search_material(msg, Some("搜索到的资料内容"));
        assert!(out.starts_with("【网络搜索资料】\n<search_material>\n搜索到的资料内容\n</search_material>"));
        assert!(out.contains(msg));
        assert!(out.find("search_material").unwrap() < out.find("【参考角色】").unwrap());
    }

    #[test]
    fn test_inject_search_material_none_or_empty_is_noop() {
        let msg = "原始消息";
        assert_eq!(inject_search_material(msg, None), msg);
        assert_eq!(inject_search_material(msg, Some("")), msg);
        assert_eq!(inject_search_material(msg, Some("   ")), msg);
    }
}
