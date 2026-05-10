use serde::{Deserialize, Serialize};

pub fn send_message_tool_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "send_message",
            "description": "Send a message to a target user or group",
            "parameters": {
                "type": "object",
                "properties": {
                    "target_type": {
                        "type": "string",
                        "enum": ["private", "group"],
                        "description": "Whether the target is a private user or a group"
                    },
                    "target_id": {
                        "type": "string",
                        "description": "The ID of the target user or group"
                    },
                    "content": {
                        "type": "string",
                        "description": "The message content to send"
                    }
                },
                "required": ["target_type", "target_id", "content"]
            }
        }
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<serde_json::Value>,
}
