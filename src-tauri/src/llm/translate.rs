use crate::llm::provider::LlmProvider;
use crate::models::agent_voice::{TranslateForTtsRequest, TranslateForTtsResponse, TranslateForTtsResult};

const TRANSLATE_PROMPT: &str = r#"You are a translation assistant for a roleplay character.

Character persona:
{persona}

Character relationships:
{relationships}

Relevant memories:
{memories}

Task:
1. Detect whether the following text is already written in the target language "{target_language}".
2. If yes, return need_translate=false and the original text unchanged.
3. If no, translate the text into "{target_language}". The translation must match the character's tone and personality described in the persona above — the same sentence can have very different translations depending on who is speaking, so choose the expression that fits this character.

Text:
{text}

Return JSON only, no explanation, no markdown fences:
{"need_translate": true, "translated_text": "..."}
"#;

/// TTS 前的语言检测与翻译。
/// 这是一次完全独立的 LLM 调用：不进入会话历史，但携带人设/关系/记忆以保证译文符合角色口吻。
pub async fn translate_for_tts(
    provider: &dyn LlmProvider,
    req: &TranslateForTtsRequest,
) -> Result<TranslateForTtsResult, String> {
    let prompt = TRANSLATE_PROMPT
        .replace("{persona}", &req.agent_persona)
        .replace("{relationships}", &req.agent_relationships)
        .replace("{memories}", &req.memories)
        .replace("{target_language}", &req.target_language)
        .replace("{text}", &req.text);

    let messages = vec![serde_json::json!({
        "role": "user",
        "content": prompt,
    })];

    let response = provider
        .chat("You are a helpful translation assistant.", messages, vec![])
        .await?;

    let content = response.content.clone().ok_or("Empty LLM response")?;

    // 容错：模型可能在 JSON 外包裹解释文字或 markdown 代码块，截取首尾花括号之间的内容
    let json_start = content.find('{').ok_or("No JSON in translate response")?;
    let json_end = content.rfind('}').ok_or("No JSON in translate response")?;
    if json_end < json_start {
        return Err("Malformed JSON in translate response".into());
    }
    let json_str = &content[json_start..=json_end];
    let parsed: TranslateForTtsResponse = serde_json::from_str(json_str)
        .map_err(|e| format!("Parse translate response failed: {}", e))?;

    Ok(TranslateForTtsResult {
        response: parsed,
        usage: response.usage,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use crate::llm::tool::LlmResponse;

    struct MockProvider {
        reply: String,
    }

    #[async_trait]
    impl LlmProvider for MockProvider {
        async fn chat(
            &self,
            _system_prompt: &str,
            _messages: Vec<serde_json::Value>,
            _tools: Vec<serde_json::Value>,
        ) -> Result<LlmResponse, String> {
            Ok(LlmResponse {
                content: Some(self.reply.clone()),
                tool_calls: vec![],
                usage: Some(serde_json::json!({
                    "prompt_tokens": 10,
                    "completion_tokens": 5,
                    "total_tokens": 15,
                })),
            })
        }

        async fn chat_raw(
            &self,
            _messages: Vec<serde_json::Value>,
            _tools: Vec<serde_json::Value>,
        ) -> Result<LlmResponse, String> {
            unimplemented!()
        }
    }

    fn sample_request() -> TranslateForTtsRequest {
        TranslateForTtsRequest {
            text: "你好，今天过得怎么样？".to_string(),
            target_language: "ja".to_string(),
            agent_persona: "一个活泼的少女".to_string(),
            agent_relationships: "对用户: 青梅竹马".to_string(),
            memories: "用户喜欢猫".to_string(),
            model_config_id: "cfg1".to_string(),
        }
    }

    #[tokio::test]
    async fn test_translate_needed() {
        let provider = MockProvider {
            reply: r#"{"need_translate": true, "translated_text": "こんにちは、今日はどう？"}"#.to_string(),
        };
        let result = translate_for_tts(&provider, &sample_request()).await.unwrap();
        assert!(result.response.need_translate);
        assert_eq!(result.response.translated_text, "こんにちは、今日はどう？");
        assert!(result.usage.is_some());
    }

    #[tokio::test]
    async fn test_translate_not_needed() {
        let provider = MockProvider {
            reply: r#"{"need_translate": false, "translated_text": "こんにちは"}"#.to_string(),
        };
        let result = translate_for_tts(&provider, &sample_request()).await.unwrap();
        assert!(!result.response.need_translate);
    }

    #[tokio::test]
    async fn test_translate_response_with_markdown_fence() {
        let provider = MockProvider {
            reply: "```json\n{\"need_translate\": true, \"translated_text\": \"テスト\"}\n```".to_string(),
        };
        let result = translate_for_tts(&provider, &sample_request()).await.unwrap();
        assert!(result.response.need_translate);
        assert_eq!(result.response.translated_text, "テスト");
    }

    #[tokio::test]
    async fn test_translate_invalid_response_errors() {
        let provider = MockProvider {
            reply: "这不是 JSON".to_string(),
        };
        let result = translate_for_tts(&provider, &sample_request()).await;
        assert!(result.is_err());
    }
}
