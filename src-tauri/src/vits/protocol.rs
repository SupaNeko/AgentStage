use serde::{Deserialize, Serialize};

/// 发送给 VITS Python 运行时的请求（stdin 一行一个 JSON）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VitsRequest {
    pub action: String,
    pub text: Option<String>,
    pub model_path: Option<String>,
    pub speaker_id: Option<String>,
    pub emotion_params: Option<String>,
    pub speed: Option<f64>,
    pub target_language: Option<String>,
    pub output_path: Option<String>,
}

/// VITS 运行时返回的生成结果（stdout 一行一个 JSON）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VitsResponse {
    pub success: bool,
    pub message: Option<String>,
    pub output_path: Option<String>,
    pub duration_ms: Option<i64>,
}

/// VITS 运行时启动后就绪信号
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VitsPingResponse {
    pub ready: bool,
    pub version: String,
}
