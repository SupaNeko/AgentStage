pub const SYSTEM_PROMPT: &str = "你是一个正在参与即时通讯聊天的 AI 角色。请根据上下文自然地回应。\n你可以同时参与多个私聊和群聊，在回复时请根据上下文判断应该回复哪个会话。\n如果需要回复多个会话，可以多次调用 send_message 工具。\n请注意：你每次被调用时，都会看到自上次回复以来积累的所有新消息。请综合考虑这些消息后再决定如何回应。";

pub const LAYER_PERSONA_TITLE: &str = "【你的角色设定】";

pub const LAYER_PARTICIPANTS_TITLE: &str = "【你认识的参与者】";
pub const LAYER_PARTICIPANTS_FORMAT: &str = "- {}（{}）：{}\n";

pub const LAYER_HISTORY_TITLE: &str = "【历史聊天记录】";
pub const LAYER_HISTORY_SESSION_SEPARATOR: &str = "\n--- {} ---\n";
pub const LAYER_HISTORY_MESSAGE_FORMAT: &str = "[{}] {}: {}\n";
pub const LAYER_FOOTER_NOTE: &str = "以上是最新的聊天记录，请根据上下文决定是否需要回复。";

pub const LAYER_INSTRUCTION_TITLE: &str = "【工具使用说明】";
pub const TOOL_INSTRUCTION_TEMPLATE: &str = r#"你可以使用 send_message 工具向指定会话发送消息。
当前你正在以下会话中聊天：
{context_list}
请根据上下文决定是否需要回复，以及回复哪个会话。
如果需要回复，请调用 send_message 工具，参数如下：
- target_type: "private" 或 "group"
- target_id: 目标会话的 session_id（必须是上面列出的 ID 之一）
- content: 你要发送的消息内容

注意：你只能向上面列出的会话发送消息。target_id 必须是完整的 session_id，不能使用名称或其他 ID。"#;

pub const INSTRUCTION_CONTEXT_LIST_FORMAT: &str = "- session_id: {}, 名称: {}, 类型: {}\n";

pub const UNKNOWN_SESSION: &str = "未知会话";
pub const USER_NAME_DEFAULT: &str = "用户";
pub const USER_PERSONA_DEFAULT: &str = "正在与你聊天的真实用户";
pub const USER_NAME: &str = "用户";
pub const SYSTEM_NAME: &str = "系统";
pub const UNKNOWN_AGENT_PREFIX: &str = "未知角色(";
pub const UNKNOWN_TYPE_PREFIX: &str = "未知(";
