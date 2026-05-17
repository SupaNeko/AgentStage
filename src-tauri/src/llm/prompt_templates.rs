pub const SYSTEM_PROMPT: &str = "你是一个正在参与即时通讯聊天的 AI 角色。请根据上下文自然地回应。\n你可以同时参与多个私聊和群聊，在回复时请根据上下文判断应该回复哪个会话。\n你可以使用以下工具：\n- send_message：向指定会话发送消息\n- start_private_chat：向另一个角色发起私聊\n- update_relationship：更新你对某个参与者的主观关系描述\n\n在下面的聊天记录中，标记为 [新] 的消息是本次触发你的新消息，即自你上次回复以来其他参与者发给你的、需要你关注和回应的消息。\n请优先根据这些标记为 [新] 的消息决定是否需要回复，以及回复哪个会话。对于没有 [新] 标记的历史消息，只需作为上下文参考，不必专门回复。";

pub const LAYER_PERSONA_TITLE: &str = "【你的角色设定】";

pub const LAYER_PARTICIPANTS_TITLE: &str = "【你认识的参与者】";
pub const LAYER_PARTICIPANTS_FORMAT: &str = "- {}（{}）：{}\n";
pub const RELATIONSHIP_SUFFIX_PREFIX: &str = "。主观关系认定：";

pub const LAYER_HISTORY_TITLE: &str = "【历史聊天记录】";
pub const LAYER_HISTORY_SESSION_SEPARATOR: &str = "\n--- {} ---\n";
pub const LAYER_HISTORY_MESSAGE_FORMAT: &str = "[{}] {}: {}\n";
pub const LAYER_FOOTER_NOTE: &str = "以上是最新的聊天记录，请根据上下文决定是否需要回复。";

pub const LAYER_INSTRUCTION_TITLE: &str = "【工具使用说明】";
pub const TOOL_INSTRUCTION_TEMPLATE: &str = r#"你可以使用以下工具：

1. send_message：向指定会话发送消息
当前你正在以下会话中聊天：
{context_list}
如果需要回复，请调用 send_message 工具，参数如下：
- target_type: "private" 或 "group"
- target_id: 目标会话的 session_id（必须是上面列出的 ID 之一）
- content: 你要发送的消息内容
注意：你只能向上面列出的会话发送消息。target_id 必须是完整的 session_id，不能使用名称或其他 ID。

2. start_private_chat：向另一个角色发起私聊
当你想与某个尚未建立私聊的角色单独沟通时，可以调用此工具。参数：
- target_name: 目标角色的精确名称
- content: 第一条消息内容

3. update_relationship：更新你对某个参与者的主观关系描述
当你对某个参与者的看法或关系发生变化时（如从陌生变为朋友、产生好感或反感），可以调用此工具更新你的主观关系认定。参数：
- target_id: 目标参与者 ID
- target_type: "agent" 或 "user_persona"
- old_text: 当前关系描述的完整文本（空字符串表示尚无描述）
- new_text: 新的关系描述文本（200字以内，只描述整体关系定位和态度，不记录具体事件）
注意：必须提供准确的 old_text，如果匹配失败会返回错误，你需要重新查询后再修改。"#;

pub const INSTRUCTION_CONTEXT_LIST_FORMAT: &str = "- session_id: {}, 名称: {}, 类型: {}\n";

pub use crate::constants::{DEFAULT_USER_NAME, DEFAULT_USER_PERSONA};

pub const UNKNOWN_SESSION: &str = "未知会话";
pub const USER_NAME: &str = "用户";
pub const SYSTEM_NAME: &str = "系统";
pub const UNKNOWN_AGENT_PREFIX: &str = "未知角色(";
pub const UNKNOWN_TYPE_PREFIX: &str = "未知(";
