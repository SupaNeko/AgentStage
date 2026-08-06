use tauri::State;

use crate::db::agent_voice as voice_repo;
use crate::db::connection::{get_db, DbState};
use crate::db::usage as usage_repo;
use crate::llm::openai::OpenAiCompatibleProvider;
use crate::llm::translate;
use crate::models::agent_voice::*;
use crate::models::usage::LlmUsageRecord;
use crate::vits::protocol::VitsRequest;
use crate::vits::runtime::{runtime_exe_path, VitsState};

/// 从 config.json 的 speakers 字段解析说话人名称列表。
/// 兼容两种结构：数组 ["a","b"] 与 字典 {"a":0,"b":1}（字典按 id 升序取键）。
fn parse_speakers(json: &serde_json::Value) -> Vec<String> {
    match json.get("speakers") {
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|s| s.as_str())
            .map(|s| s.to_string())
            .collect(),
        Some(serde_json::Value::Object(map)) => {
            let mut pairs: Vec<(String, i64)> = map
                .iter()
                .filter_map(|(k, v)| v.as_i64().map(|id| (k.clone(), id)))
                .collect();
            pairs.sort_by_key(|(_, id)| *id);
            pairs.into_iter().map(|(name, _)| name).collect()
        }
        _ => vec![],
    }
}

/// 过滤消息中的表情标签（<sticker>包名_贴纸名</sticker>），返回剩余文本。
/// 表情不应进入翻译和语音合成。
fn strip_sticker_tags(text: &str) -> String {
    let re = regex::Regex::new(r"<sticker>[^<]+</sticker>").unwrap();
    let stripped = re.replace_all(text, " ");
    // 合并多余空白
    stripped.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[tauri::command]
pub async fn check_vits_runtime() -> Result<bool, String> {
    let data_dir = crate::get_data_dir().map_err(|e| e.to_string())?;
    Ok(runtime_exe_path(&data_dir).exists())
}

#[tauri::command]
pub async fn scan_vits_models() -> Result<Vec<VitsModelInfo>, String> {
    let data_dir = crate::get_data_dir().map_err(|e| e.to_string())?;
    let models_dir = data_dir.join("vits_models");
    if !models_dir.exists() {
        return Ok(vec![]);
    }

    let mut models = Vec::new();
    let entries = std::fs::read_dir(&models_dir).map_err(|e| e.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        // 只接受包含 .pth 权重文件的目录
        let has_pth = std::fs::read_dir(&path)
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .any(|e| {
                        e.path()
                            .extension()
                            .map(|ext| ext == "pth")
                            .unwrap_or(false)
                    })
            })
            .unwrap_or(false);
        if !has_pth {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        // Compatible with community config files not named config.json (e.g. paimon6k.json)
        let config_path = if path.join("config.json").exists() {
            path.join("config.json")
        } else {
            std::fs::read_dir(&path)
                .map(|rd| {
                    rd.filter_map(|e| e.ok())
                        .find(|e| {
                            e.path()
                                .extension()
                                .map(|ext| ext == "json")
                                .unwrap_or(false)
                        })
                        .map(|e| e.path())
                })
                .unwrap_or(None)
                .unwrap_or_else(|| path.join("config.json"))
        };
        let has_config = config_path.exists();
        let mut language = None;
        let mut speakers = vec![];
        if has_config {
            if let Ok(content) = std::fs::read_to_string(&config_path) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    language = json
                        .get("data")
                        .and_then(|d| d.get("language"))
                        .and_then(|l| l.as_str())
                        .map(|s| s.to_string())
                        .or_else(|| {
                            json.get("model")
                                .and_then(|m| m.get("language"))
                                .and_then(|l| l.as_str())
                                .map(|s| s.to_string())
                        });
                    speakers = parse_speakers(&json);
                }
            }
        }
        models.push(VitsModelInfo {
            name,
            path: path.to_string_lossy().to_string(),
            language,
            speakers,
            has_config,
        });
    }
    Ok(models)
}

#[tauri::command]
pub async fn save_agent_voice(
    state: State<'_, DbState>,
    req: SaveAgentVoiceRequest,
) -> Result<AgentVoice, String> {
    let conn = get_db(&state).await?;
    voice_repo::save_agent_voice(&conn, &req).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_agent_voice(
    state: State<'_, DbState>,
    agent_id: String,
) -> Result<Option<AgentVoice>, String> {
    let conn = get_db(&state).await?;
    voice_repo::get_agent_voice_by_agent_id(&conn, &agent_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_agent_voice(
    state: State<'_, DbState>,
    agent_id: String,
) -> Result<(), String> {
    let conn = get_db(&state).await?;
    voice_repo::delete_agent_voice(&conn, &agent_id).map_err(|e| e.to_string())
}

/// 生成（或命中缓存直接返回）某条消息的语音文件路径
#[tauri::command]
pub async fn generate_voice(
    state: State<'_, DbState>,
    vits: State<'_, VitsState>,
    req: GenerateVoiceRequest,
) -> Result<String, String> {
    // 过滤表情标签：<sticker>...</sticker> 不参与翻译与合成
    let mut text = strip_sticker_tags(&req.text);
    if text.is_empty() {
        return Err("消息过滤表情后没有可合成的文本".into());
    }

    // Step 1: 读取语音配置
    let voice = {
        let conn = get_db(&state).await?;
        voice_repo::get_agent_voice_by_agent_id(&conn, &req.agent_id)
            .map_err(|e| e.to_string())?
            .ok_or("该角色未配置语音模型")?
    };

    // Step 2: 缓存命中且文件仍存在则直接返回
    {
        let conn = get_db(&state).await?;
        if let Some(cached) = voice_repo::get_vits_cache_by_message_id(&conn, &req.message_id)
            .map_err(|e| e.to_string())?
        {
            if std::path::Path::new(&cached.file_path).exists() {
                return Ok(cached.file_path);
            }
        }
    }

    // 生产级日志：记录每次语音生成的输入文本（单行化）
    crate::logger::info(&format!(
        "[VITS] generate start | agent={} session={} message={} text={}",
        req.agent_id,
        req.session_id,
        req.message_id,
        text.replace(['\n', '\r'], " ")
    ));

    // Step 3: 语言检测与翻译（独立 LLM 调用，不计入会话历史）
    if voice.translate_enabled {
        let (translate_model_id, agent, relationships) = {
            let conn = get_db(&state).await?;
            let agent = crate::db::agent::get_by_id(&conn, &req.agent_id)
                .map_err(|e| e.to_string())?
                .ok_or("Agent not found")?;
            let translate_model_id = voice
                .translate_model_config_id
                .clone()
                .or_else(|| agent.model_config_id.clone())
                .ok_or("该角色未配置可用于翻译的模型")?;
            let relationships =
                crate::db::agent_relationship::list_relationships_by_observer(&conn, &req.agent_id)
                    .map(|items| {
                        items
                            .iter()
                            .filter(|i| !i.relationship_text.trim().is_empty())
                            .map(|i| {
                                format!(
                                    "- 对 {}（{}）: {}",
                                    i.target_name, i.target_label, i.relationship_text
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default();
            (translate_model_id, agent, relationships)
        };

        let model_config = {
            let conn = get_db(&state).await?;
            crate::db::model_config::get_by_id(&conn, &translate_model_id)
                .map_err(|e| e.to_string())?
                .ok_or("翻译用的模型配置不存在")?
        };

        let provider = OpenAiCompatibleProvider::new(
            model_config
                .api_key_encrypted
                .as_ref()
                .and_then(|enc| crate::crypto::decrypt(enc).ok())
                .unwrap_or_default(),
            model_config.base_url.clone(),
            model_config.model_name.clone(),
            model_config.temperature,
            model_config.max_tokens,
        );

        let translate_req = TranslateForTtsRequest {
            text: text.clone(),
            target_language: voice.target_language.clone(),
            agent_persona: agent.detailed_persona.clone(),
            agent_relationships: relationships,
            memories: agent.long_term_memory.clone().unwrap_or_default(),
            model_config_id: translate_model_id.clone(),
        };

        let result = translate::translate_for_tts(&provider, &translate_req).await?;

        let (prompt_tokens, completion_tokens, total_tokens) = result
            .usage
            .as_ref()
            .and_then(|u| u.as_object())
            .map(|obj| {
                let prompt = obj.get("prompt_tokens").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let completion = obj
                    .get("completion_tokens")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0) as i32;
                let total = obj.get("total_tokens").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                (prompt, completion, total)
            })
            .unwrap_or((0, 0, 0));

        // 单独归类为 tts_translate，支持按角色/模型/会话维度统计
        let usage = LlmUsageRecord {
            id: uuid::Uuid::new_v4().to_string(),
            agent_id: req.agent_id.clone(),
            model_config_id: translate_model_id,
            session_id: Some(req.session_id.clone()),
            trigger_type: "tts_translate".to_string(),
            call_round: 1,
            prompt_tokens,
            completion_tokens,
            total_tokens,
            message_id: Some(req.message_id.clone()),
            created_at: chrono::Utc::now().timestamp_millis(),
        };
        usage_repo::insert_usage_record(&state, &usage).await?;

        if result.response.need_translate {
            crate::logger::info(&format!(
                "[VITS] translated | message={} from={} to={}",
                req.message_id,
                text.replace(['\n', '\r'], " "),
                result.response.translated_text.replace(['\n', '\r'], " ")
            ));
            text = result.response.translated_text;
        }
    }

    // Step 4: 调用 VITS 运行时生成语音
    let data_dir = crate::get_data_dir().map_err(|e| e.to_string())?;
    let cache_dir = data_dir.join("vits_cache").join(&req.session_id);
    std::fs::create_dir_all(&cache_dir).map_err(|e| e.to_string())?;
    let output_path = cache_dir.join(format!("{}.wav", req.message_id));

    let vits_req = VitsRequest {
        action: "generate".into(),
        text: Some(text),
        model_path: Some(voice.model_path.clone()),
        speaker_id: voice.speaker_id.clone(),
        emotion_params: voice.emotion_params.clone(),
        speed: Some(voice.speed),
        target_language: Some(voice.target_language.clone()),
        output_path: Some(output_path.to_string_lossy().to_string()),
    };

    // 着重标记：最终交给 VITS 合成的文本（翻译后/未翻译的原文）
    crate::logger::info(&format!(
        "[VITS] >>> TTS INPUT >>> message={} model={} speaker={:?} speed={} text={}",
        req.message_id,
        voice.model_name,
        voice.speaker_id,
        voice.speed,
        vits_req.text.as_deref().unwrap_or("").replace(['\n', '\r'], " ")
    ));

    let resp = {
        let mut runtime = vits.lock().await;
        runtime.generate(&vits_req).await?
    };
    if !resp.success {
        crate::logger::error(&format!(
            "[VITS] generate failed | message={} error={}",
            req.message_id,
            resp.message.clone().unwrap_or_default()
        ));
        return Err(resp.message.unwrap_or_else(|| "VITS generation failed".into()));
    }

    crate::logger::info(&format!(
        "[VITS] generate done | message={} output={} duration_ms={:?}",
        req.message_id,
        output_path.to_string_lossy(),
        resp.duration_ms
    ));

    // Step 5: 记录缓存
    let file_size = std::fs::metadata(&output_path)
        .map(|m| m.len() as i64)
        .unwrap_or(0);
    {
        let conn = get_db(&state).await?;
        voice_repo::insert_vits_cache(
            &conn,
            &req.message_id,
            &req.session_id,
            &req.agent_id,
            &output_path.to_string_lossy(),
            file_size,
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(output_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn list_voice_cache(
    state: State<'_, DbState>,
    agent_id: Option<String>,
) -> Result<Vec<VoiceCacheItem>, String> {
    let conn = get_db(&state).await?;
    voice_repo::list_vits_cache(&conn, agent_id.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_voice_cache(state: State<'_, DbState>, id: String) -> Result<(), String> {
    let conn = get_db(&state).await?;
    let item = voice_repo::get_vits_cache_by_id(&conn, &id).map_err(|e| e.to_string())?;
    if let Some(item) = item {
        let _ = std::fs::remove_file(&item.file_path);
    }
    voice_repo::delete_vits_cache(&conn, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_voice_cache(
    state: State<'_, DbState>,
    session_id: Option<String>,
) -> Result<(), String> {
    let conn = get_db(&state).await?;
    let items = voice_repo::list_vits_cache(&conn, None).map_err(|e| e.to_string())?;
    for item in &items {
        if session_id.as_ref().map_or(true, |sid| item.session_id == *sid) {
            let _ = std::fs::remove_file(&item.file_path);
        }
    }
    voice_repo::clear_vits_cache(&conn, session_id.as_deref()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_speakers_array() {
        let json = serde_json::json!({"speakers": ["芳乃", "茉子"]});
        assert_eq!(parse_speakers(&json), vec!["芳乃", "茉子"]);
    }

    #[test]
    fn test_parse_speakers_dict_sorted_by_id() {
        // SenrenBanka 模型的实际结构：{"芳乃":0,"茉子":1,"丛雨":2,...}
        let json = serde_json::json!({"speakers": {"丛雨": 2, "芳乃": 0, "茉子": 1}});
        assert_eq!(parse_speakers(&json), vec!["芳乃", "茉子", "丛雨"]);
    }

    #[test]
    fn test_parse_speakers_missing_or_invalid() {
        assert!(parse_speakers(&serde_json::json!({})).is_empty());
        assert!(parse_speakers(&serde_json::json!({"speakers": 123})).is_empty());
        // 数组中的非字符串项被跳过
        assert_eq!(
            parse_speakers(&serde_json::json!({"speakers": ["a", 1, "b"]})),
            vec!["a", "b"]
        );
    }

    #[test]
    fn test_strip_sticker_tags() {
        assert_eq!(
            strip_sticker_tags("你好 <sticker>测试包_开心</sticker> 今天怎么样"),
            "你好 今天怎么样"
        );
        // 多个表情
        assert_eq!(
            strip_sticker_tags("<sticker>包_哭</sticker>不要这样<sticker>包_生气</sticker>"),
            "不要这样"
        );
        // 纯表情消息过滤后为空
        assert_eq!(strip_sticker_tags("<sticker>包_笑</sticker>"), "");
        // 无表情时不变
        assert_eq!(strip_sticker_tags("普通消息"), "普通消息");
        // 不完整标签保留原文（与前端 parseStickerContent 行为一致）
        assert_eq!(strip_sticker_tags("你好<sticker>没闭合"), "你好<sticker>没闭合");
    }
}
