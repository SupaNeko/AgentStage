use async_trait::async_trait;

/// 搜索错误分类。每种错误都有面向用户的明确中文提示。
#[derive(Debug)]
pub enum SearchError {
    /// 网络问题：连接失败、超时
    Network(String),
    /// API Key 无效 / 无权限（401/403）
    Auth(String),
    /// 触发限流（429）
    RateLimited,
    /// 厂商返回的其他错误
    Provider(String),
    /// 未配置厂商或 Key
    NotConfigured,
}

impl SearchError {
    pub fn user_message(&self, provider_name: &str) -> String {
        match self {
            SearchError::Network(detail) => format!(
                "无法连接到{}搜索服务（网络问题或请求超时），请检查网络后重试。详情：{}",
                provider_name, detail
            ),
            SearchError::Auth(_) => format!(
                "{}搜索 API Key 无效或已过期，请到 设置 → 通用 → 搜索 API 检查 Key",
                provider_name
            ),
            SearchError::RateLimited => format!(
                "{}搜索 API 触发限流，请稍后重试",
                provider_name
            ),
            SearchError::Provider(detail) => format!(
                "{}搜索服务返回错误：{}",
                provider_name, detail
            ),
            SearchError::NotConfigured => {
                "未配置搜索 API，请到 设置 → 通用 → 搜索 API 选择厂商并填写 Key".to_string()
            }
        }
    }

    /// 是否应中断整个人设生成流程（网络/Key/限流问题重试无意义，需显式告知用户）
    pub fn is_fatal(&self) -> bool {
        matches!(
            self,
            SearchError::Network(_)
                | SearchError::Auth(_)
                | SearchError::RateLimited
                | SearchError::NotConfigured
        )
    }
}

impl std::fmt::Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.user_message("搜索"))
    }
}

#[derive(Debug, Clone)]
pub struct SearchHit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[async_trait]
pub trait SearchProvider: Send + Sync {
    fn display_name(&self) -> &'static str;
    /// 执行搜索，返回格式化后的结果文本（供 LLM 阅读）
    async fn search(&self, query: &str) -> Result<String, SearchError>;
}

pub fn create_provider(provider: &str, api_key: &str) -> Result<Box<dyn SearchProvider>, SearchError> {
    match provider {
        "bocha" => Ok(Box::new(BochaProvider::new(api_key))),
        "zhipu" => Ok(Box::new(ZhipuProvider::new(api_key))),
        "kimi" => Ok(Box::new(KimiProvider::new(api_key))),
        _ => Err(SearchError::NotConfigured),
    }
}

/// 单次搜索请求超时
const SEARCH_TIMEOUT_SECS: u64 = 15;

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(SEARCH_TIMEOUT_SECS))
        .build()
        .expect("Failed to build reqwest client")
}

fn map_reqwest_err(e: reqwest::Error) -> SearchError {
    if e.is_timeout() {
        SearchError::Network(format!("请求超时（{}秒）", SEARCH_TIMEOUT_SECS))
    } else if e.is_connect() {
        SearchError::Network("连接失败".to_string())
    } else {
        SearchError::Provider(e.to_string())
    }
}

/// 根据 HTTP 状态码分类错误
fn map_http_status(status: u16, body: &str) -> SearchError {
    let truncated: String = body.chars().take(200).collect();
    match status {
        401 | 403 => SearchError::Auth(truncated),
        429 => SearchError::RateLimited,
        _ => SearchError::Provider(format!("HTTP {}: {}", status, truncated)),
    }
}

/// 把搜索结果格式化为 LLM 可读的文本
fn format_hits(hits: &[SearchHit]) -> String {
    if hits.is_empty() {
        return "（未找到相关结果）".to_string();
    }
    let mut out = String::new();
    for (i, h) in hits.iter().enumerate() {
        out.push_str(&format!("[{}] {}\n链接: {}\n摘要: {}\n\n", i + 1, h.title, h.url, h.snippet));
    }
    out.trim_end().to_string()
}

// ============================== 博查 ==============================

pub struct BochaProvider {
    client: reqwest::Client,
    api_key: String,
}

impl BochaProvider {
    pub fn new(api_key: &str) -> Self {
        Self {
            client: http_client(),
            api_key: api_key.to_string(),
        }
    }
}

#[async_trait]
impl SearchProvider for BochaProvider {
    fn display_name(&self) -> &'static str {
        "博查"
    }

    async fn search(&self, query: &str) -> Result<String, SearchError> {
        let resp = self
            .client
            .post("https://api.bochaai.com/v1/web-search")
            .bearer_auth(&self.api_key)
            .json(&serde_json::json!({
                "query": query,
                "freshness": "noLimit",
                "summary": true,
                "count": 10,
            }))
            .send()
            .await
            .map_err(map_reqwest_err)?;

        let status = resp.status().as_u16();
        let body = resp.text().await.map_err(map_reqwest_err)?;
        if status != 200 {
            return Err(map_http_status(status, &body));
        }
        let json: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| SearchError::Provider(format!("响应解析失败: {}", e)))?;

        // 博查部分错误以 200 + body 内 code 返回
        if let Some(code) = json.get("code").and_then(|c| c.as_i64()) {
            if code == 401 || code == 403 {
                return Err(SearchError::Auth(
                    json.get("msg").and_then(|m| m.as_str()).unwrap_or("").to_string(),
                ));
            }
            if code != 200 {
                let msg = json.get("msg").and_then(|m| m.as_str()).unwrap_or("未知错误");
                return Err(SearchError::Provider(format!("code={}: {}", code, msg)));
            }
        }

        let hits: Vec<SearchHit> = json
            .pointer("/data/webPages/value")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|item| SearchHit {
                        title: item["name"].as_str().unwrap_or("").to_string(),
                        url: item["url"].as_str().unwrap_or("").to_string(),
                        snippet: item["summary"]
                            .as_str()
                            .or_else(|| item["snippet"].as_str())
                            .unwrap_or("")
                            .to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(format_hits(&hits))
    }
}

// ============================== 智谱 ==============================

pub struct ZhipuProvider {
    client: reqwest::Client,
    api_key: String,
}

impl ZhipuProvider {
    pub fn new(api_key: &str) -> Self {
        Self {
            client: http_client(),
            api_key: api_key.to_string(),
        }
    }
}

#[async_trait]
impl SearchProvider for ZhipuProvider {
    fn display_name(&self) -> &'static str {
        "智谱"
    }

    async fn search(&self, query: &str) -> Result<String, SearchError> {
        let resp = self
            .client
            .post("https://open.bigmodel.cn/api/paas/v4/web_search")
            .bearer_auth(&self.api_key)
            .json(&serde_json::json!({
                "search_query": query,
                "search_engine": "search_std",
                "search_intent": false,
                "count": 10,
                "search_recency_filter": "noLimit",
            }))
            .send()
            .await
            .map_err(map_reqwest_err)?;

        let status = resp.status().as_u16();
        let body = resp.text().await.map_err(map_reqwest_err)?;
        if status != 200 {
            return Err(map_http_status(status, &body));
        }
        let json: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| SearchError::Provider(format!("响应解析失败: {}", e)))?;

        let hits: Vec<SearchHit> = json
            .get("search_result")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|item| SearchHit {
                        title: item["title"].as_str().unwrap_or("").to_string(),
                        url: item["link"].as_str().unwrap_or("").to_string(),
                        snippet: item["content"].as_str().unwrap_or("").to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(format_hits(&hits))
    }
}

// ============================== Kimi（Moonshot $web_search 内置工具） ==============================

pub struct KimiProvider {
    client: reqwest::Client,
    api_key: String,
}

impl KimiProvider {
    pub fn new(api_key: &str) -> Self {
        Self {
            client: http_client(),
            api_key: api_key.to_string(),
        }
    }
}

#[async_trait]
impl SearchProvider for KimiProvider {
    fn display_name(&self) -> &'static str {
        "Kimi"
    }

    async fn search(&self, query: &str) -> Result<String, SearchError> {
        // Kimi 没有独立的搜索 HTTP 接口，通过 chat/completions + 内置 $web_search 工具实现：
        // 模型生成搜索参数并自行执行搜索，客户端只需把 tool_call 的 arguments 原样回传。
        let mut messages = vec![
            serde_json::json!({
                "role": "system",
                "content": "你是搜索助手。使用 $web_search 工具搜索用户的问题，然后用中文简明扼要地汇总搜索结果（列出关键事实与来源）。",
            }),
            serde_json::json!({ "role": "user", "content": query }),
        ];
        let tools = serde_json::json!([{
            "type": "builtin_function",
            "function": { "name": "$web_search" }
        }]);

        // 内置工具循环上限，防止异常死循环
        for _ in 0..4 {
            let resp = self
                .client
                .post("https://api.moonshot.cn/v1/chat/completions")
                .bearer_auth(&self.api_key)
                .json(&serde_json::json!({
                    "model": "moonshot-v1-8k",
                    "messages": messages,
                    "tools": tools,
                    "temperature": 0.3,
                }))
                .send()
                .await
                .map_err(map_reqwest_err)?;

            let status = resp.status().as_u16();
            let body = resp.text().await.map_err(map_reqwest_err)?;
            if status != 200 {
                return Err(map_http_status(status, &body));
            }
            let json: serde_json::Value = serde_json::from_str(&body)
                .map_err(|e| SearchError::Provider(format!("响应解析失败: {}", e)))?;

            let message = &json["choices"][0]["message"];
            let tool_calls = message["tool_calls"].as_array();

            match tool_calls {
                Some(calls) if !calls.is_empty() => {
                    // 回传 assistant 消息 + 每个 tool_call 的 arguments 原样作为 tool 结果
                    messages.push(message.clone());
                    for call in calls {
                        let args = call["function"]["arguments"].as_str().unwrap_or("{}");
                        messages.push(serde_json::json!({
                            "role": "tool",
                            "tool_call_id": call["id"].as_str().unwrap_or(""),
                            "name": call["function"]["name"].as_str().unwrap_or(""),
                            "content": args,
                        }));
                    }
                }
                _ => {
                    let content = message["content"].as_str().unwrap_or("").trim();
                    if content.is_empty() {
                        return Ok("（未找到相关结果）".to_string());
                    }
                    return Ok(content.to_string());
                }
            }
        }
        Err(SearchError::Provider("Kimi 搜索循环超过上限".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_http_status() {
        assert!(matches!(map_http_status(401, ""), SearchError::Auth(_)));
        assert!(matches!(map_http_status(403, ""), SearchError::Auth(_)));
        assert!(matches!(map_http_status(429, ""), SearchError::RateLimited));
        assert!(matches!(map_http_status(500, "oops"), SearchError::Provider(_)));
    }

    #[test]
    fn test_error_user_messages_are_explicit() {
        let cases = vec![
            SearchError::Network("连接失败".into()),
            SearchError::Auth("".into()),
            SearchError::RateLimited,
            SearchError::Provider("HTTP 500".into()),
            SearchError::NotConfigured,
        ];
        for c in cases {
            let msg = c.user_message("博查");
            assert!(!msg.is_empty());
            assert!(msg.contains("博查") || msg.contains("搜索"));
        }
    }

    #[test]
    fn test_is_fatal_classification() {
        assert!(SearchError::Network("x".into()).is_fatal());
        assert!(SearchError::Auth("".into()).is_fatal());
        assert!(SearchError::RateLimited.is_fatal());
        assert!(!SearchError::Provider("HTTP 500".into()).is_fatal());
    }

    #[test]
    fn test_format_hits_empty() {
        assert_eq!(format_hits(&[]), "（未找到相关结果）");
    }

    #[test]
    fn test_format_hits() {
        let hits = vec![
            SearchHit { title: "标题1".into(), url: "https://a.com".into(), snippet: "摘要1".into() },
            SearchHit { title: "标题2".into(), url: "https://b.com".into(), snippet: "摘要2".into() },
        ];
        let out = format_hits(&hits);
        assert!(out.contains("[1] 标题1"));
        assert!(out.contains("链接: https://a.com"));
        assert!(out.contains("摘要: 摘要2"));
    }

    #[test]
    fn test_create_provider_unknown() {
        assert!(matches!(create_provider("unknown", "k"), Err(SearchError::NotConfigured)));
        assert!(create_provider("bocha", "k").is_ok());
        assert!(create_provider("zhipu", "k").is_ok());
        assert!(create_provider("kimi", "k").is_ok());
    }
}
