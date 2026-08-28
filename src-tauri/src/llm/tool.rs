use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::TimeZone;
use rand::Rng;
use crate::db::connection::DbState;
use crate::db::session as session_repo;
use crate::db::message as message_repo;
use crate::db::agent as agent_repo;
use crate::models::message::Message;
use crate::models::scheduled_task::CreateTimerRequest;

pub fn split_br_tags(content: &str) -> Vec<String> {
    content.split("<br/>")
        .flat_map(|s| s.split("</n>"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn send_message_tool_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "send_message",
            "description": "向指定会话发送一条消息。\n- 你可以在 content 中使用 <br/> 或 </n> 标签进行分割，被分割的消息将被显示为多条消息。\n- target_id 必须是系统提供的完整 session_id，绝对不能使用会话名称或其他 ID。\n- 只能回复 context_list 中列出的会话。\n- target_type 的取值为 \"private\"（私聊）或 \"group\"（群聊）。\n- 如果 target_id 无效，调用将会失败。",

            "parameters": {
                "type": "object",
                "properties": {
                    "target_type": {
                        "type": "string",
                        "enum": ["private", "group"],
                        "description": "目标会话类型"
                    },
                    "target_id": {
                        "type": "string",
                        "description": "目标会话的 session_id（必须使用系统提供的完整 ID）"
                    },
                    "content": {
                        "type": "string",
                        "description": "消息内容"
                    }
                },
                "required": ["target_type", "target_id", "content"]
            }
        }
    })
}

pub fn start_private_chat_tool_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "start_private_chat",
            "description": "当你需要和另一位角色进行一对一交流且不存在已有私聊会话时，向该角色发起私聊。\n- target_name 必须是对方的精确名称（exact match）。\n- 如果该角色不存在或名称不匹配，将返回错误。\n- 成功后你会获得一个新的私聊会话，之后可以通过 send_message 继续在该会话中发送消息。",
            "parameters": {
                "type": "object",
                "properties": {
                    "target_name": {
                        "type": "string",
                        "description": "目标角色的精确名称（exact match）"
                    },
                    "content": {
                        "type": "string",
                        "description": "第一条消息内容"
                    }
                },
                "required": ["target_name", "content"]
            }
        }
    })
}

pub fn update_relationship_tool_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "update_relationship",
            "description": "更新你对某个参与者的主观关系描述。这用于记录你对对方的整体定位（如朋友/同事/竞争对手）和基本态度（如喜欢/讨厌/尊敬），不是记忆具体事件。\n规则：\n1. 只更新整体关系定位，不要记录日常琐事（如\"他今天吃了汉堡\"）\n2. 描述控制在 200 字以内\n3. 必须提供 old_text（当前关系描述的完整内容），系统会精确匹配并替换\n4. 如果 old_text 不匹配（说明你记错了当前关系），系统会返回错误，请重新查询后再修改\n5. target_name 必须是参与者列表中的精确名称\n示例：\n- old_text: \"普通朋友，偶尔聊几句\" → new_text: \"值得信赖的朋友\"\n- old_text: \"\" → new_text: \"初次见面，看起来是个温和的人\"",

            "parameters": {
                "type": "object",
                "properties": {
                    "target_name": {
                        "type": "string",
                        "description": "目标参与者的精确名称（从【你认识的参与者】列表中获取）"
                    },
                    "old_text": {
                        "type": "string",
                        "description": "当前关系描述的完整文本（空字符串表示尚无描述）"
                    },
                    "new_text": {
                        "type": "string",
                        "description": "新的关系描述文本（200字以内）"
                    }
                },
                "required": ["target_name", "old_text", "new_text"]
            }
        }
    })
}

pub fn update_memory_tool_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "update_memory",
            "description": "记录动态信息：事实、事件、偏好、行为模式等。\nupdate_relationship 是静态关系定位；update_memory 是动态内容，不要混淆。\n规则：\n1. memory_type=\"self\"：更新你对自己的长期记忆（上限 3000 字），target_name 可留空\n2. memory_type=\"other\"：更新你对某位参与者的记忆（上限 500 字），target_name 必须填写该参与者的精确名称\n3. 必须提供 old_text（当前记忆的完整内容），系统会精确匹配并替换\n4. 如果 old_text 不匹配，系统会返回错误，请重新查询后再修改\n5. target_name 必须是参与者列表中的精确名称\n示例：\n- memory_type: \"self\", old_text: \"\", new_text: \"我是一个喜欢在深夜写代码的程序员，养了一只叫豆豆的猫...\"\n- memory_type: \"other\", target_name: \"Alice\", old_text: \"\", new_text: \"她不喜欢吃辣，对芒果过敏，喜欢听爵士乐。\"",

            "parameters": {
                "type": "object",
                "properties": {
                    "memory_type": {
                        "type": "string",
                        "enum": ["self", "other"],
                        "description": "记忆类型：self 表示更新关于自己的长期记忆，other 表示更新关于另一位参与者的记忆"
                    },
                    "target_name": {
                        "type": "string",
                        "description": "目标参与者的精确名称（memory_type=other 时必填；memory_type=self 时可为空字符串）"
                    },
                    "old_text": {
                        "type": "string",
                        "description": "当前记忆的完整文本（空字符串表示尚无记忆）"
                    },
                    "new_text": {
                        "type": "string",
                        "description": "新的记忆文本（self 上限 3000 字，other 上限 500 字）"
                    }
                },
                "required": ["memory_type", "target_name", "old_text", "new_text"]
            }
        }
    })
}

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

pub fn web_search_tool_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "web_search",
            "description": "搜索互联网获取实时信息。输入搜索关键词，返回相关网页的标题、链接和摘要。可多次调用以搜集不同方面的信息。",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "搜索关键词，应简洁明确，每次只搜索一个主题"
                    }
                },
                "required": ["query"]
            }
        }
    })
}

pub fn create_timer_tool_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "create_timer",
            "description": "创建一个定时任务。你可以设定一个未来事件或循环事件，到时间后你会收到一次特殊触发。\n支持 3 种模式：\n1. N 分钟后触发（单次，trigger_mode=\"after_minutes\"）\n2. 指定具体日期时间触发（单次，trigger_mode=\"datetime\"）\n3. 按固定间隔循环触发（task_type=\"recurring\"，interval_minutes）",
            "parameters": {
                "type": "object",
                "properties": {
                    "description": { "type": "string", "description": "事件描述，如'提醒起床'" },
                    "task_type": { "type": "string", "enum": ["single", "recurring"] },
                    "trigger_mode": { "type": "string", "enum": ["after_minutes", "datetime"] },
                    "after_minutes": { "type": "integer" },
                    "year": { "type": "integer" },
                    "month": { "type": "integer" },
                    "day": { "type": "integer" },
                    "hour": { "type": "integer" },
                    "minute": { "type": "integer" },
                    "interval_minutes": { "type": "integer", "description": "循环间隔分钟数" }
                },
                "required": ["description", "task_type"]
            }
        }
    })
}

pub fn delete_timer_tool_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "delete_timer",
            "description": "删除一个你创建的定时任务。你可以在\"等待中的定时任务\"列表中查看任务ID。",
            "parameters": {
                "type": "object",
                "properties": {
                    "task_id": { "type": "string" }
                },
                "required": ["task_id"]
            }
        }
    })
}

pub fn get_all_tool_schemas() -> Vec<serde_json::Value> {
    vec![
        send_message_tool_schema(),
        start_private_chat_tool_schema(),
        update_relationship_tool_schema(),
        update_memory_tool_schema(),
        create_timer_tool_schema(),
        delete_timer_tool_schema(),
    ]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmCallUsage {
    pub call_round: i32,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}

#[derive(Debug)]
pub enum ToolError {
    InvalidArguments(String),
    EmptyContent,
    TargetNotFound(String),
    DatabaseError(String),
    SessionMuted(String),
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolError::InvalidArguments(s) => write!(f, "工具参数格式错误: {}", s),
            ToolError::EmptyContent => write!(f, "工具调用内容为空"),
            ToolError::TargetNotFound(s) => write!(f, "找不到目标会话: {}", s),
            ToolError::DatabaseError(s) => write!(f, "保存消息失败: {}", s),
            ToolError::SessionMuted(s) => write!(f, "会话已禁言: {}", s),
        }
    }
}

#[derive(Clone)]
pub struct ToolExecutor {
    db_state: DbState,
    scheduler: crate::scheduler::Scheduler,
}

impl ToolExecutor {
    pub fn new(db_state: DbState, scheduler: crate::scheduler::Scheduler) -> Self {
        Self { db_state, scheduler }
    }

    pub async fn execute_single(
        &self,
        agent_id: &str,
        tool_call: &ToolCall,
        session_pages: &HashMap<String, i32>,
    ) -> Result<Vec<Message>, ToolError> {
        match tool_call.name.as_str() {
            "send_message" => self.execute_send_message(agent_id, &tool_call.arguments, session_pages).await,
            "start_private_chat" => self.execute_start_private_chat(agent_id, &tool_call.arguments, session_pages).await,
            "update_relationship" => { self.execute_update_relationship(agent_id, &tool_call.arguments, session_pages).await?; Ok(vec![]) }
            "update_memory" => { self.execute_update_memory(agent_id, &tool_call.arguments, session_pages).await?; Ok(vec![]) }
            "create_timer" => { self.execute_create_timer(agent_id, &tool_call.arguments).await?; Ok(vec![]) }
            "delete_timer" => { self.execute_delete_timer(agent_id, &tool_call.arguments).await?; Ok(vec![]) }
            _ => Err(ToolError::InvalidArguments(format!("未知工具: {}", tool_call.name))),
        }
    }

    pub async fn execute(
        &self,
        agent_id: &str,
        tool_calls: Vec<ToolCall>,
        session_pages: &HashMap<String, i32>,
    ) -> Result<Vec<Message>, ToolError> {
        let mut results = Vec::new();
        for tc in tool_calls {
            let msgs = self.execute_single(agent_id, &tc, session_pages).await?;
            results.extend(msgs);
        }
        Ok(results)
    }

    /// 根据 session_pages 中的快照解析当时使用的 user_persona_id。
    /// 如果 session_pages 包含有效会话，查询 chat_page_participants 快照获取 user 类型的 participant_id。
    /// 否则 fallback 到当前 active_persona_id。
    fn resolve_user_persona_id(
        &self,
        conn: &rusqlite::Connection,
        session_pages: &HashMap<String, i32>,
    ) -> Result<Option<String>, rusqlite::Error> {
        if let Some((session_id, page_index)) = session_pages.iter().next() {
            if let Ok(Some(chat_page_id)) = crate::db::chat_page_participant::get_chat_page_id(conn, session_id, *page_index) {
                let mut stmt = conn.prepare(
                    "SELECT participant_id FROM chat_page_participants WHERE chat_page_id = ?1 AND participant_type = 'user'"
                )?;
                let rows = stmt.query_map([&chat_page_id], |row| {
                    row.get::<_, String>(0)
                })?;
                for row in rows {
                    if let Ok(pid) = row {
                        crate::logger::debug(&format!(
                            "[DEBUG ToolExecutor::resolve_user_persona_id] resolved from snapshot: session={}, page={}, persona_id={}",
                            session_id, page_index, pid
                        ));
                        return Ok(Some(pid));
                    }
                }
            }
        }
        // Fallback: current active persona
        let active_id: Option<String> = conn.query_row(
            "SELECT active_persona_id FROM app_settings WHERE id = 1", [], |row| row.get(0),
        ).ok().flatten();
        crate::logger::debug(&format!(
            "[DEBUG ToolExecutor::resolve_user_persona_id] fallback to active_persona_id={:?}", active_id
        ));
        Ok(active_id)
    }

    async fn execute_send_message(
        &self,
        agent_id: &str,
        arguments: &str,
        session_pages: &HashMap<String, i32>,
    ) -> Result<Vec<Message>, ToolError> {
        crate::logger::debug(&format!(
            "[DEBUG ToolExecutor::execute_send_message] START agent_id={}, args_raw={}",
            agent_id, arguments
        ));

        let args: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| ToolError::InvalidArguments(e.to_string()))?;

        let raw_target_id = args["target_id"].as_str().unwrap_or("");
        let content = args["content"].as_str().unwrap_or("");

        crate::logger::debug(&format!(
            "[DEBUG ToolExecutor::execute_send_message] parsed raw_target_id={}, content_len={}",
            raw_target_id, content.len()
        ));

        if content.is_empty() {
            crate::logger::warn(&format!(
                "[DEBUG ToolExecutor::execute_send_message] Empty content, aborting"
            ));
            return Err(ToolError::EmptyContent);
        }

        // 自动映射 target_id
        let target_id = self.resolve_target_id(agent_id, raw_target_id).await?;
        crate::logger::debug(&format!(
            "[DEBUG ToolExecutor::execute_send_message] resolved target_id={}", target_id
        ));

        // 检查目标会话是否禁言
        {
            let conn = self.db_state.0.lock().await;
            let muted: bool = conn.query_row(
                "SELECT COALESCE(mute_enabled, 0) FROM session_settings WHERE session_id = ?1",
                [&target_id],
                |row| Ok(row.get::<_, i32>(0)? != 0),
            ).unwrap_or(false);
            if muted {
                crate::logger::warn(&format!(
                    "[execute_send_message] target session={} is muted, blocking message from agent={}",
                    target_id, agent_id
                ));
                return Err(ToolError::SessionMuted(target_id.clone()));
            }
        }

        // 使用触发时绑定的 page_index，避免 reset 后的页面漂移
        let bound_page = session_pages.get(&target_id).copied();
        crate::logger::debug(&format!(
            "[DEBUG ToolExecutor::execute_send_message] bound_page={:?} for target_id={}",
            bound_page, target_id
        ));

        // 按 <br/> 或 </n> 拆分内容，每条拆分段作为独立消息插入
        let contents = split_br_tags(content);
        let conn = self.db_state.0.lock().await;
        let mut messages = Vec::new();
        for c in &contents {
            let msg = message_repo::insert_message(
                &conn, &target_id, "agent", agent_id, c, "text", bound_page,
            ).map_err(|e| ToolError::DatabaseError(e.to_string()))?;
            messages.push(msg);
        }
        drop(conn);

        // Trigger overflow summary check
        self.scheduler.spawn_overflow_summary(target_id.to_string());

        // Reset proactive timer after sending a message
        {
            let conn = self.db_state.0.lock().await;
            if let Ok(Some(agent)) = agent_repo::get_by_id(&conn, agent_id) {
                drop(conn);
                if agent.proactive_enabled {
                    let min_ms = agent.proactive_min_minutes as i64 * 60 * 1000;
                    let max_ms = agent.proactive_max_minutes as i64 * 60 * 1000;
                    let random_ms = rand::thread_rng().gen_range(min_ms..=max_ms);
                    let next_at = chrono::Utc::now().timestamp_millis() + random_ms;
                    self.scheduler.set_proactive_timer(agent_id, next_at).await;
                }
            }
        }

        crate::logger::debug(&format!(
            "[DEBUG ToolExecutor::execute_send_message] wrote {} messages target_id={}, page_index={:?}",
            messages.len(), target_id, bound_page
        ));

        Ok(messages)
    }

    async fn execute_start_private_chat(
        &self,
        agent_id: &str,
        arguments: &str,
        session_pages: &HashMap<String, i32>,
    ) -> Result<Vec<Message>, ToolError> {
        crate::logger::debug(&format!(
            "[DEBUG ToolExecutor::execute_start_private_chat] START agent_id={}, args_raw={}",
            agent_id, arguments
        ));

        let args: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| ToolError::InvalidArguments(e.to_string()))?;

        let target_name = args["target_name"].as_str().unwrap_or("");
        let content = args["content"].as_str().unwrap_or("");

        crate::logger::debug(&format!(
            "[DEBUG ToolExecutor::execute_start_private_chat] parsed target_name={}, content_len={}",
            target_name, content.len()
        ));

        if content.is_empty() {
            crate::logger::warn(&format!(
                "[DEBUG ToolExecutor::execute_start_private_chat] Empty content, aborting"
            ));
            return Err(ToolError::EmptyContent);
        }

        let conn = self.db_state.0.lock().await;

        let target_agent = agent_repo::get_agent_by_name(&conn, target_name)
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?
            .ok_or_else(|| ToolError::TargetNotFound(target_name.to_string()))?;

        if target_agent.id == agent_id {
            return Err(ToolError::InvalidArguments("无法与自己发起私聊".to_string()));
        }

        let target_id = target_agent.id;

        let session = session_repo::get_private_session_between_agents(&conn, agent_id, &target_id)
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        let session_id = if let Some(session) = session {
            session.id
        } else {
            let new_session = session_repo::create_agent_agent_session(&conn, agent_id, &target_id)
                .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
            new_session.id
        };

        // 插入双向 friendships
        let now = chrono::Utc::now().timestamp_millis();
        let _ = conn.execute(
            "INSERT OR IGNORE INTO friendships (id, agent_id_1, agent_id_2, participant_type_2, created_at, source_session_id) VALUES (?1, ?2, ?3, 'agent', ?4, ?5)",
            rusqlite::params![uuid::Uuid::new_v4().to_string(), agent_id, &target_id, now, &session_id],
        );
        let _ = conn.execute(
            "INSERT OR IGNORE INTO friendships (id, agent_id_1, agent_id_2, participant_type_2, created_at, source_session_id) VALUES (?1, ?2, ?3, 'agent', ?4, ?5)",
            rusqlite::params![uuid::Uuid::new_v4().to_string(), &target_id, agent_id, now, &session_id],
        );

        let bound_page = session_pages.get(&session_id).copied();
        let contents = split_br_tags(content);
        let mut messages = Vec::new();
        for c in &contents {
            let msg = message_repo::insert_message(
                &conn, &session_id, "agent", agent_id, c, "text", bound_page,
            ).map_err(|e| ToolError::DatabaseError(e.to_string()))?;
            messages.push(msg);
        }

        crate::logger::debug(&format!(
            "[DEBUG ToolExecutor::execute_start_private_chat] wrote {} messages session_id={}, page_index={:?}",
            messages.len(), session_id, bound_page
        ));

        Ok(messages)
    }

    async fn execute_update_relationship(
        &self,
        agent_id: &str,
        arguments: &str,
        session_pages: &HashMap<String, i32>,
    ) -> Result<Vec<Message>, ToolError> {
        crate::logger::debug(&format!(
            "[DEBUG ToolExecutor::execute_update_relationship] START agent_id={}, args_raw={}",
            agent_id, arguments
        ));

        let args: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| ToolError::InvalidArguments(e.to_string()))?;

        let target_name = args["target_name"].as_str().unwrap_or("");
        let old_text = args["old_text"].as_str().unwrap_or("");
        let new_text = args["new_text"].as_str().unwrap_or("");

        crate::logger::debug(&format!(
            "[DEBUG ToolExecutor::execute_update_relationship] parsed target_name='{}', old_text_len={}, new_text_len={}",
            target_name, old_text.len(), new_text.len()
        ));

        if target_name.is_empty() {
            return Err(ToolError::InvalidArguments("target_name 不能为空".to_string()));
        }

        // 校验长度
        if new_text.chars().count() > 200 {
            crate::logger::warn(&format!(
                "[DEBUG ToolExecutor::execute_update_relationship] Text too long: {} chars", new_text.chars().count()
            ));
            return Err(ToolError::InvalidArguments(format!(
                "关系描述超过 200 字限制（当前 {} 字）", new_text.chars().count()
            )));
        }

        let conn = self.db_state.0.lock().await;

        // 根据名称查找目标
        let (target_id, target_type) = if let Ok(Some(agent)) = agent_repo::get_agent_by_name(&conn, target_name) {
            crate::logger::debug(&format!(
                "[DEBUG ToolExecutor::execute_update_relationship] resolved to agent id={}", agent.id
            ));
            (agent.id, "agent".to_string())
        } else {
            // 尝试从快照或当前激活人设解析用户人设
            let persona_id = self.resolve_user_persona_id(&conn, session_pages)
                .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
            if let Some(pid) = persona_id {
                if let Ok(persona) = crate::db::user_persona::get_user_persona_by_id(&conn, &pid) {
                    if persona.name == target_name {
                        crate::logger::debug(&format!(
                            "[DEBUG ToolExecutor::execute_update_relationship] resolved to user_persona id={}", pid
                        ));
                        (pid, "user_persona".to_string())
                    } else {
                        crate::logger::warn(&format!(
                            "[DEBUG ToolExecutor::execute_update_relationship] persona name '{}' does not match target_name '{}'", persona.name, target_name
                        ));
                        return Err(ToolError::InvalidArguments(format!(
                            "找不到目标参与者 '{}'", target_name
                        )));
                    }
                } else {
                    return Err(ToolError::InvalidArguments(format!(
                        "找不到目标参与者 '{}'", target_name
                    )));
                }
            } else {
                return Err(ToolError::InvalidArguments(format!(
                    "找不到目标参与者 '{}'", target_name
                )));
            }
        };

        // old_text 匹配校验
        let current = crate::db::agent_relationship::get_relationship(&conn, agent_id, &target_id, &target_type)
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        crate::logger::debug(&format!(
            "[DEBUG ToolExecutor::execute_update_relationship] compare current='{}' (len={}) vs old_text='{}' (len={}) equal={}",
            current, current.len(), old_text, old_text.len(), current == old_text
        ));

        if current != old_text {
            crate::logger::warn(&format!(
                "[DEBUG ToolExecutor::execute_update_relationship] old_text mismatch"
            ));
            return Err(ToolError::InvalidArguments(format!(
                "old_text 不匹配。当前关系描述为：\"{}\"（长度{}），你提交的是：\"{}\"（长度{}）。请基于当前内容重新提交修改。",
                current, current.len(), old_text, old_text.len()
            )));
        }

        crate::db::agent_relationship::upsert_relationship(&conn, agent_id, &target_id, &target_type, new_text)
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        crate::logger::debug(&format!(
            "[DEBUG ToolExecutor::execute_update_relationship] END updated agent_id={} -> target_id={}",
            agent_id, target_id
        ));

        Ok(Vec::new())
    }

    async fn execute_update_memory(
        &self,
        agent_id: &str,
        arguments: &str,
        session_pages: &HashMap<String, i32>,
    ) -> Result<Vec<Message>, ToolError> {
        crate::logger::debug(&format!(
            "[DEBUG ToolExecutor::execute_update_memory] START agent_id={}, args_raw={}",
            agent_id, arguments
        ));

        let args: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| ToolError::InvalidArguments(e.to_string()))?;

        let memory_type = args["memory_type"].as_str().unwrap_or("");
        let target_name = args["target_name"].as_str().unwrap_or("");
        let old_text = args["old_text"].as_str().unwrap_or("");
        let new_text = args["new_text"].as_str().unwrap_or("");

        crate::logger::debug(&format!(
            "[DEBUG ToolExecutor::execute_update_memory] parsed memory_type='{}', target_name='{}', old_text_len={}, new_text_len={}",
            memory_type, target_name, old_text.len(), new_text.len()
        ));

        match memory_type {
            "self" => {
                if new_text.chars().count() > 3000 {
                    crate::logger::warn(&format!(
                        "[DEBUG ToolExecutor::execute_update_memory] Self memory too long: {} chars", new_text.chars().count()
                    ));
                    return Err(ToolError::InvalidArguments(format!(
                        "自我记忆超过 3000 字限制（当前 {} 字）", new_text.chars().count()
                    )));
                }

                let conn = self.db_state.0.lock().await;

                let current: String = conn.query_row(
                    "SELECT COALESCE(long_term_memory, '') FROM agents WHERE id = ?1",
                    [agent_id],
                    |row| row.get(0),
                ).map_err(|e| ToolError::DatabaseError(e.to_string()))?;

                crate::logger::debug(&format!(
                    "[DEBUG ToolExecutor::execute_update_memory] self compare current='{}' (len={}) vs old_text='{}' (len={}) equal={}",
                    current, current.len(), old_text, old_text.len(), current == old_text
                ));

                if current != old_text {
                    crate::logger::warn(&format!(
                        "[DEBUG ToolExecutor::execute_update_memory] self old_text mismatch"
                    ));
                    return Err(ToolError::InvalidArguments(format!(
                        "old_text 不匹配。当前自我记忆为：\"{}\"（长度{}），你提交的是：\"{}\"（长度{}）。请基于当前内容重新提交修改。",
                        current, current.len(), old_text, old_text.len()
                    )));
                }

                conn.execute(
                    "UPDATE agents SET long_term_memory = ?1 WHERE id = ?2",
                    rusqlite::params![new_text, agent_id],
                ).map_err(|e| ToolError::DatabaseError(e.to_string()))?;

                crate::logger::debug(&format!(
                    "[DEBUG ToolExecutor::execute_update_memory] END updated self memory for agent_id={}",
                    agent_id
                ));

                Ok(Vec::new())
            }
            "other" => {
                if new_text.chars().count() > 500 {
                    crate::logger::warn(&format!(
                        "[DEBUG ToolExecutor::execute_update_memory] Other memory too long: {} chars", new_text.chars().count()
                    ));
                    return Err(ToolError::InvalidArguments(format!(
                        "他人记忆超过 500 字限制（当前 {} 字）", new_text.chars().count()
                    )));
                }

                if target_name.is_empty() {
                    return Err(ToolError::InvalidArguments("memory_type=other 时 target_name 不能为空".to_string()));
                }

                let conn = self.db_state.0.lock().await;

                // 根据名称查找目标
                let (target_id, target_type) = if let Ok(Some(agent)) = agent_repo::get_agent_by_name(&conn, target_name) {
                    crate::logger::debug(&format!(
                        "[DEBUG ToolExecutor::execute_update_memory] resolved to agent id={}", agent.id
                    ));
                    (agent.id, "agent".to_string())
                } else {
                    // 尝试从快照或当前激活人设解析用户人设
                    let persona_id = self.resolve_user_persona_id(&conn, session_pages)
                        .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
                    if let Some(pid) = persona_id {
                        if let Ok(persona) = crate::db::user_persona::get_user_persona_by_id(&conn, &pid) {
                            if persona.name == target_name {
                                crate::logger::debug(&format!(
                                    "[DEBUG ToolExecutor::execute_update_memory] resolved to user_persona id={}", pid
                                ));
                                (pid, "user_persona".to_string())
                            } else {
                                crate::logger::warn(&format!(
                                    "[DEBUG ToolExecutor::execute_update_memory] persona name '{}' does not match target_name '{}'", persona.name, target_name
                                ));
                                return Err(ToolError::InvalidArguments(format!(
                                    "找不到目标参与者 '{}'", target_name
                                )));
                            }
                        } else {
                            return Err(ToolError::InvalidArguments(format!(
                                "找不到目标参与者 '{}'", target_name
                            )));
                        }
                    } else {
                        return Err(ToolError::InvalidArguments(format!(
                            "找不到目标参与者 '{}'", target_name
                        )));
                    }
                };

                // old_text 匹配校验
                let current: String = conn.query_row(
                    "SELECT COALESCE(memory_text, '') FROM agent_relationships WHERE observer_id = ?1 AND target_id = ?2 AND target_type = ?3",
                    rusqlite::params![agent_id, &target_id, &target_type],
                    |row| row.get(0),
                ).unwrap_or_default();

                crate::logger::debug(&format!(
                    "[DEBUG ToolExecutor::execute_update_memory] other compare current='{}' (len={}) vs old_text='{}' (len={}) equal={}",
                    current, current.len(), old_text, old_text.len(), current == old_text
                ));

                if current != old_text {
                    crate::logger::warn(&format!(
                        "[DEBUG ToolExecutor::execute_update_memory] other old_text mismatch"
                    ));
                    return Err(ToolError::InvalidArguments(format!(
                        "old_text 不匹配。当前对 '{}' 的记忆为：\"{}\"（长度{}），你提交的是：\"{}\"（长度{}）。请基于当前内容重新提交修改。",
                        target_name, current, current.len(), old_text, old_text.len()
                    )));
                }

                crate::db::agent_relationship::upsert_memory(&conn, agent_id, &target_id, &target_type, new_text)
                    .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

                crate::logger::debug(&format!(
                    "[DEBUG ToolExecutor::execute_update_memory] END updated memory agent_id={} -> target_id={}",
                    agent_id, target_id
                ));

                Ok(Vec::new())
            }
            _ => {
                Err(ToolError::InvalidArguments(format!(
                    "不支持的 memory_type: '{}'，必须是 'self' 或 'other'", memory_type
                )))
            }
        }
    }

    async fn execute_create_timer(
        &self,
        agent_id: &str,
        arguments: &str,
    ) -> Result<Vec<Message>, ToolError> {
        let args: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| ToolError::InvalidArguments(e.to_string()))?;
        let description = args["description"].as_str().ok_or(ToolError::InvalidArguments("missing description".to_string()))?;
        let task_type = args["task_type"].as_str().ok_or(ToolError::InvalidArguments("missing task_type".to_string()))?;

        let now = chrono::Utc::now().timestamp_millis();
        let next_trigger_at = if task_type == "single" {
            let trigger_mode = args["trigger_mode"].as_str().ok_or(ToolError::InvalidArguments("missing trigger_mode".to_string()))?;
            if trigger_mode == "after_minutes" {
                let minutes = args["after_minutes"].as_i64().ok_or(ToolError::InvalidArguments("missing after_minutes".to_string()))?;
                if minutes <= 0 { return Err(ToolError::InvalidArguments("after_minutes must be > 0".to_string())); }
                now + minutes * 60 * 1000
            } else if trigger_mode == "datetime" {
                let year = args["year"].as_i64().ok_or(ToolError::InvalidArguments("missing year".to_string()))? as i32;
                let month = args["month"].as_i64().ok_or(ToolError::InvalidArguments("missing month".to_string()))? as u32;
                let day = args["day"].as_i64().ok_or(ToolError::InvalidArguments("missing day".to_string()))? as u32;
                let hour = args["hour"].as_i64().ok_or(ToolError::InvalidArguments("missing hour".to_string()))? as u32;
                let minute = args["minute"].as_i64().ok_or(ToolError::InvalidArguments("missing minute".to_string()))? as u32;
                let dt = chrono::Local.with_ymd_and_hms(year, month, day, hour, minute, 0)
                    .single().ok_or(ToolError::InvalidArguments("invalid datetime".to_string()))?;
                let ts = dt.timestamp_millis();
                if ts <= now { return Err(ToolError::InvalidArguments("datetime must be in the future".to_string())); }
                ts
            } else {
                return Err(ToolError::InvalidArguments("invalid trigger_mode".to_string()));
            }
        } else if task_type == "recurring" {
            let interval = args["interval_minutes"].as_i64().ok_or(ToolError::InvalidArguments("missing interval_minutes".to_string()))?;
            if interval <= 0 { return Err(ToolError::InvalidArguments("interval_minutes must be > 0".to_string())); }
            now + interval * 60 * 1000
        } else {
            return Err(ToolError::InvalidArguments("invalid task_type".to_string()));
        };

        let req = CreateTimerRequest {
            description: description.to_string(),
            task_type: task_type.to_string(),
            trigger_mode: args["trigger_mode"].as_str().map(|s| s.to_string()),
            after_minutes: args["after_minutes"].as_i64().map(|v| v as i32),
            year: args["year"].as_i64().map(|v| v as i32),
            month: args["month"].as_i64().map(|v| v as i32),
            day: args["day"].as_i64().map(|v| v as i32),
            hour: args["hour"].as_i64().map(|v| v as i32),
            minute: args["minute"].as_i64().map(|v| v as i32),
            interval_minutes: args["interval_minutes"].as_i64().map(|v| v as i32),
            next_trigger_at: Some(next_trigger_at),
            target_session_id: None,
        };

        let conn = self.db_state.0.lock().await;
        let task_id = crate::db::scheduled_task::insert_task(&conn, &req, agent_id)
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        crate::logger::info(&format!("[create_timer] agent_id={} created task_id={}", agent_id, task_id));
        Ok(Vec::new())
    }

    async fn execute_delete_timer(
        &self,
        agent_id: &str,
        arguments: &str,
    ) -> Result<Vec<Message>, ToolError> {
        let args: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| ToolError::InvalidArguments(e.to_string()))?;
        let task_id = args["task_id"].as_str().ok_or(ToolError::InvalidArguments("missing task_id".to_string()))?;

        let conn = self.db_state.0.lock().await;
        let tasks = crate::db::scheduled_task::list_by_agent(&conn, agent_id)
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        if !tasks.iter().any(|t| t.id == task_id) {
            return Err(ToolError::InvalidArguments("任务不存在或不属于你".to_string()));
        }

        crate::db::scheduled_task::delete_task(&conn, task_id)
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        crate::logger::info(&format!("[delete_timer] agent_id={} deleted task_id={}", agent_id, task_id));
        Ok(Vec::new())
    }

    async fn resolve_target_id(
        &self,
        agent_id: &str,
        raw: &str,
    ) -> Result<String, ToolError> {
        let conn = self.db_state.0.lock().await;

        // 1. 如果 raw 本身就是合法的 session_id，直接返回
        if let Ok(Some(_)) = session_repo::get_session_by_id(&conn, raw) {
            crate::logger::debug(&format!(
                "[DEBUG resolve_target_id] raw='{}' is valid session_id", raw
            ));
            return Ok(raw.to_string());
        }

        // 2. 如果 raw 是 agent_id，优先查找与 sender 的 agent-agent 私聊 session
        if let Ok(Some(session)) = session_repo::get_private_session_between_agents(&conn, agent_id, raw) {
            crate::logger::debug(&format!(
                "[DEBUG resolve_target_id] raw='{}' resolved to agent-agent session_id={}", raw, session.id
            ));
            return Ok(session.id);
        }

        // 3. 查找 raw 对应的 user-agent 私聊 session
        if let Ok(Some(session)) = session_repo::get_private_session_by_agent_id(&conn, raw) {
            crate::logger::debug(&format!(
                "[DEBUG resolve_target_id] raw='{}' resolved to user-agent session_id={}", raw, session.id
            ));
            return Ok(session.id);
        }

        // 4. 默认：使用该 agent 自己的 user-agent 私聊 session
        if let Ok(Some(session)) = session_repo::get_private_session_by_agent_id(&conn, agent_id) {
            crate::logger::warn(&format!(
                "[DEBUG resolve_target_id] raw='{}' not found, fallback to agent's default session {}",
                raw, session.id
            ));
            return Ok(session.id);
        }

        crate::logger::error(&format!(
            "[DEBUG resolve_target_id] raw='{}' not found for agent_id={}", raw, agent_id
        ));
        Err(ToolError::TargetNotFound(raw.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use rusqlite::Connection;
    use crate::db::connection::DbState;
    fn init_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("PRAGMA foreign_keys = OFF;", []).unwrap();
        conn.execute_batch(crate::db::schema::BASE_SCHEMA).unwrap();
        conn
    }

    fn make_db_state(conn: Connection) -> DbState {
        DbState(Arc::new(Mutex::new(conn)))
    }

    fn create_test_agent(conn: &Connection, agent_id: &str, name: &str) {
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, created_at, updated_at) VALUES (?1, ?2, '', '', ?3, ?3)",
            (agent_id, name, 0i64),
        ).unwrap();
    }

    #[tokio::test]
    async fn test_start_private_chat_creates_session_and_message() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1", "Alice");
        create_test_agent(&conn, "agent-2", "Bob");
        let db_state = make_db_state(conn);

        let scheduler = crate::scheduler::Scheduler::new(db_state.clone());
        let executor = ToolExecutor::new(db_state, scheduler);
        let session_pages = HashMap::new();
        let tool_call = ToolCall {
            id: "tc-1".to_string(),
            name: "start_private_chat".to_string(),
            arguments: r#"{"target_name":"Bob","content":"Hello Bob"}"#.to_string(),
        };

        let messages = executor.execute("agent-1", vec![tool_call], &session_pages).await.unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].content, "Hello Bob");
        assert_eq!(messages[0].sender_id, "agent-1");

        let conn = executor.db_state.0.lock().await;
        let session = crate::db::session::get_private_session_between_agents(&conn, "agent-1", "agent-2").unwrap();
        assert!(session.is_some());
        let session_id = session.unwrap().id;
        assert_eq!(messages[0].session_id, session_id);

        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM friendships WHERE source_session_id = ?1",
            [&session_id],
            |row| row.get(0),
        ).unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_start_private_chat_reuses_existing_session() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1", "Alice");
        create_test_agent(&conn, "agent-2", "Bob");
        let db_state = make_db_state(conn);

        let scheduler = crate::scheduler::Scheduler::new(db_state.clone());
        let executor = ToolExecutor::new(db_state, scheduler);
        let session_pages = HashMap::new();

        let tc1 = ToolCall {
            id: "tc-1".to_string(),
            name: "start_private_chat".to_string(),
            arguments: r#"{"target_name":"Bob","content":"First"}"#.to_string(),
        };
        let msgs1 = executor.execute("agent-1", vec![tc1], &session_pages).await.unwrap();
        let session_id_1 = msgs1[0].session_id.clone();

        let tc2 = ToolCall {
            id: "tc-2".to_string(),
            name: "start_private_chat".to_string(),
            arguments: r#"{"target_name":"Bob","content":"Second"}"#.to_string(),
        };
        let msgs2 = executor.execute("agent-1", vec![tc2], &session_pages).await.unwrap();
        assert_eq!(msgs2[0].session_id, session_id_1);
    }

    #[tokio::test]
    async fn test_start_private_chat_target_not_found() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1", "Alice");
        let db_state = make_db_state(conn);

        let scheduler = crate::scheduler::Scheduler::new(db_state.clone());
        let executor = ToolExecutor::new(db_state, scheduler);
        let tc = ToolCall {
            id: "tc-1".to_string(),
            name: "start_private_chat".to_string(),
            arguments: r#"{"target_name":"Unknown","content":"Hi"}"#.to_string(),
        };
        let result = executor.execute("agent-1", vec![tc], &HashMap::new()).await;
        assert!(matches!(result, Err(ToolError::TargetNotFound(_))));
    }

    #[tokio::test]
    async fn test_start_private_chat_self_chat_fails() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1", "Alice");
        let db_state = make_db_state(conn);

        let scheduler = crate::scheduler::Scheduler::new(db_state.clone());
        let executor = ToolExecutor::new(db_state, scheduler);
        let tc = ToolCall {
            id: "tc-1".to_string(),
            name: "start_private_chat".to_string(),
            arguments: r#"{"target_name":"Alice","content":"Hi me"}"#.to_string(),
        };
        let result = executor.execute("agent-1", vec![tc], &HashMap::new()).await;
        assert!(matches!(result, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn test_resolve_target_id_prefers_agent_agent_session() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1", "Alice");
        create_test_agent(&conn, "agent-2", "Bob");
        let _ua_session = crate::db::session::create_private_session(&conn, "agent-2").unwrap();
        let aa_session = crate::db::session::create_agent_agent_session(&conn, "agent-1", "agent-2").unwrap();
        let _ = crate::db::session::create_private_session(&conn, "agent-1").unwrap();
        let db_state = make_db_state(conn);

        let scheduler = crate::scheduler::Scheduler::new(db_state.clone());
        let executor = ToolExecutor::new(db_state, scheduler);

        let resolved = executor.resolve_target_id("agent-1", "agent-2").await.unwrap();
        assert_eq!(resolved, aa_session.id);

        let resolved2 = executor.resolve_target_id("agent-1", "nonexistent").await.unwrap();
        let conn_guard = executor.db_state.0.lock().await;
        let fallback = crate::db::session::get_private_session_by_agent_id(&*conn_guard, "agent-1").unwrap().unwrap();
        assert_eq!(resolved2, fallback.id);
    }

    #[tokio::test]
    async fn test_update_memory_self_updates_long_term_memory() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1", "Alice");
        let db_state = make_db_state(conn);

        let scheduler = crate::scheduler::Scheduler::new(db_state.clone());
        let executor = ToolExecutor::new(db_state, scheduler);
        let tc = ToolCall {
            id: "tc-1".to_string(),
            name: "update_memory".to_string(),
            arguments: r#"{"memory_type":"self","target_name":"","old_text":"","new_text":"我喜欢在雨天读书"}"#.to_string(),
        };
        let result = executor.execute("agent-1", vec![tc], &HashMap::new()).await;
        assert!(result.is_ok());

        let conn_guard = executor.db_state.0.lock().await;
        let memory: String = conn_guard.query_row(
            "SELECT long_term_memory FROM agents WHERE id = 'agent-1'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(memory, "我喜欢在雨天读书");
    }

    #[tokio::test]
    async fn test_update_memory_other_updates_agent_memory() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1", "Alice");
        create_test_agent(&conn, "agent-2", "Bob");
        let db_state = make_db_state(conn);

        let scheduler = crate::scheduler::Scheduler::new(db_state.clone());
        let executor = ToolExecutor::new(db_state, scheduler);
        let tc = ToolCall {
            id: "tc-1".to_string(),
            name: "update_memory".to_string(),
            arguments: r#"{"memory_type":"other","target_name":"Bob","old_text":"","new_text":"他讨厌吃香菜"}"#.to_string(),
        };
        let result = executor.execute("agent-1", vec![tc], &HashMap::new()).await;
        assert!(result.is_ok());

        let conn_guard = executor.db_state.0.lock().await;
        let memory: String = conn_guard.query_row(
            "SELECT memory_text FROM agent_relationships WHERE observer_id = 'agent-1' AND target_id = 'agent-2'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(memory, "他讨厌吃香菜");
    }

    #[tokio::test]
    async fn test_update_memory_enforces_char_limits() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1", "Alice");
        create_test_agent(&conn, "agent-2", "Bob");
        let db_state = make_db_state(conn);

        let scheduler = crate::scheduler::Scheduler::new(db_state.clone());
        let executor = ToolExecutor::new(db_state, scheduler);

        // self memory > 3000 chars
        let long_self = "a".repeat(3001);
        let tc_self = ToolCall {
            id: "tc-self".to_string(),
            name: "update_memory".to_string(),
            arguments: format!(r#"{{"memory_type":"self","target_name":"","old_text":"","new_text":"{}"}}"#, long_self),
        };
        let result_self = executor.execute("agent-1", vec![tc_self], &HashMap::new()).await;
        assert!(matches!(result_self, Err(ToolError::InvalidArguments(_))));

        // other memory > 500 chars
        let long_other = "b".repeat(501);
        let tc_other = ToolCall {
            id: "tc-other".to_string(),
            name: "update_memory".to_string(),
            arguments: format!(r#"{{"memory_type":"other","target_name":"Bob","old_text":"","new_text":"{}"}}"#, long_other),
        };
        let result_other = executor.execute("agent-1", vec![tc_other], &HashMap::new()).await;
        assert!(matches!(result_other, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn test_execute_create_timer_after_minutes() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1", "Alice");
        let db_state = make_db_state(conn);

        let scheduler = crate::scheduler::Scheduler::new(db_state.clone());
        let executor = ToolExecutor::new(db_state, scheduler);
        let tc = ToolCall {
            id: "tc-1".to_string(),
            name: "create_timer".to_string(),
            arguments: r#"{"description":"reminder","task_type":"single","trigger_mode":"after_minutes","after_minutes":5}"#.to_string(),
        };
        let result = executor.execute("agent-1", vec![tc], &HashMap::new()).await;
        assert!(result.is_ok());

        let conn_guard = executor.db_state.0.lock().await;
        let tasks = crate::db::scheduled_task::list_by_agent(&*conn_guard, "agent-1").unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].description, "reminder");
        assert_eq!(tasks[0].task_type, "single");
        assert_eq!(tasks[0].after_minutes, Some(5));
        assert!(tasks[0].next_trigger_at > chrono::Utc::now().timestamp_millis());
    }

    #[tokio::test]
    async fn test_execute_create_timer_datetime() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1", "Alice");
        let db_state = make_db_state(conn);

        let scheduler = crate::scheduler::Scheduler::new(db_state.clone());
        let executor = ToolExecutor::new(db_state, scheduler);
        let tc = ToolCall {
            id: "tc-1".to_string(),
            name: "create_timer".to_string(),
            arguments: r#"{"description":"future event","task_type":"single","trigger_mode":"datetime","year":2099,"month":1,"day":1,"hour":12,"minute":0}"#.to_string(),
        };
        let result = executor.execute("agent-1", vec![tc], &HashMap::new()).await;
        assert!(result.is_ok());

        let conn_guard = executor.db_state.0.lock().await;
        let tasks = crate::db::scheduled_task::list_by_agent(&*conn_guard, "agent-1").unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].description, "future event");
        assert_eq!(tasks[0].year, Some(2099));
        assert_eq!(tasks[0].month, Some(1));
        assert_eq!(tasks[0].day, Some(1));
        assert_eq!(tasks[0].hour, Some(12));
        assert_eq!(tasks[0].minute, Some(0));
    }

    #[tokio::test]
    async fn test_execute_create_timer_recurring() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1", "Alice");
        let db_state = make_db_state(conn);

        let scheduler = crate::scheduler::Scheduler::new(db_state.clone());
        let executor = ToolExecutor::new(db_state, scheduler);
        let tc = ToolCall {
            id: "tc-1".to_string(),
            name: "create_timer".to_string(),
            arguments: r#"{"description":"recurring","task_type":"recurring","interval_minutes":10}"#.to_string(),
        };
        let result = executor.execute("agent-1", vec![tc], &HashMap::new()).await;
        assert!(result.is_ok());

        let conn_guard = executor.db_state.0.lock().await;
        let tasks = crate::db::scheduled_task::list_by_agent(&*conn_guard, "agent-1").unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].task_type, "recurring");
        assert_eq!(tasks[0].interval_minutes, Some(10));
    }

    #[tokio::test]
    async fn test_execute_delete_timer() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1", "Alice");
        let db_state = make_db_state(conn);

        let scheduler = crate::scheduler::Scheduler::new(db_state.clone());
        let executor = ToolExecutor::new(db_state, scheduler);
        let tc_create = ToolCall {
            id: "tc-1".to_string(),
            name: "create_timer".to_string(),
            arguments: r#"{"description":"to delete","task_type":"single","trigger_mode":"after_minutes","after_minutes":5}"#.to_string(),
        };
        let _ = executor.execute("agent-1", vec![tc_create], &HashMap::new()).await.unwrap();

        let conn_guard = executor.db_state.0.lock().await;
        let tasks = crate::db::scheduled_task::list_by_agent(&*conn_guard, "agent-1").unwrap();
        let task_id = tasks[0].id.clone();
        drop(conn_guard);

        let tc_delete = ToolCall {
            id: "tc-2".to_string(),
            name: "delete_timer".to_string(),
            arguments: format!(r#"{{"task_id":"{}"}}"#, task_id),
        };
        let result = executor.execute("agent-1", vec![tc_delete], &HashMap::new()).await;
        assert!(result.is_ok());

        let conn_guard = executor.db_state.0.lock().await;
        let tasks = crate::db::scheduled_task::list_by_agent(&*conn_guard, "agent-1").unwrap();
        assert_eq!(tasks.len(), 0);
    }

    #[tokio::test]
    async fn test_execute_delete_timer_not_owned_fails() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1", "Alice");
        create_test_agent(&conn, "agent-2", "Bob");
        let db_state = make_db_state(conn);

        let scheduler = crate::scheduler::Scheduler::new(db_state.clone());
        let executor = ToolExecutor::new(db_state, scheduler);
        let tc_create = ToolCall {
            id: "tc-1".to_string(),
            name: "create_timer".to_string(),
            arguments: r#"{"description":"secret","task_type":"single","trigger_mode":"after_minutes","after_minutes":5}"#.to_string(),
        };
        let _ = executor.execute("agent-1", vec![tc_create], &HashMap::new()).await.unwrap();

        let conn_guard = executor.db_state.0.lock().await;
        let tasks = crate::db::scheduled_task::list_by_agent(&*conn_guard, "agent-1").unwrap();
        let task_id = tasks[0].id.clone();
        drop(conn_guard);

        let tc_delete = ToolCall {
            id: "tc-2".to_string(),
            name: "delete_timer".to_string(),
            arguments: format!(r#"{{"task_id":"{}"}}"#, task_id),
        };
        let result = executor.execute("agent-2", vec![tc_delete], &HashMap::new()).await;
        assert!(matches!(result, Err(ToolError::InvalidArguments(_))));
    }

    #[tokio::test]
    async fn test_update_memory_other_uses_snapshot_user_persona() {
        let conn = init_test_db();
        create_test_agent(&conn, "agent-1", "Alice");
        create_test_agent(&conn, "agent-2", "Bob");
        // Insert two user personas
        conn.execute(
            "INSERT INTO user_personas (id, name, description, avatar_path, created_at, updated_at) VALUES ('up-old', 'Old User', '', NULL, 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO user_personas (id, name, description, avatar_path, created_at, updated_at) VALUES ('up-new', 'New User', '', NULL, 0, 0)",
            [],
        ).unwrap();
        // Set active persona to "up-new"
        conn.execute(
            "INSERT INTO app_settings (id, active_persona_id, updated_at) VALUES (1, 'up-new', 0)",
            [],
        ).unwrap();
        // Insert session and chat_page
        conn.execute(
            "INSERT INTO sessions (id, session_type, created_at, updated_at) VALUES ('sess-1', 'private', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO private_sessions (session_id, participant_1_type, participant_1_id, participant_2_type, participant_2_id, created_at, current_chat_page) VALUES ('sess-1', 'user', 'user', 'agent', 'agent-2', 0, 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO chat_pages (id, session_id, page_index, name, is_active, message_count, created_at, updated_at) VALUES ('cp-0', 'sess-1', 0, 'Page 0', 1, 0, 0, 0)",
            [],
        ).unwrap();
        // Insert snapshot with user persona = up-old
        conn.execute(
            "INSERT INTO chat_page_participants (chat_page_id, participant_id, participant_type, participant_name, participant_avatar, participant_simplified_persona) VALUES ('cp-0', 'up-old', 'user', 'Old User', NULL, NULL)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO chat_page_participants (chat_page_id, participant_id, participant_type, participant_name, participant_avatar, participant_simplified_persona) VALUES ('cp-0', 'agent-2', 'agent', 'Bob', NULL, NULL)",
            [],
        ).unwrap();

        let db_state = make_db_state(conn);
        let scheduler = crate::scheduler::Scheduler::new(db_state.clone());
        let executor = ToolExecutor::new(db_state, scheduler);

        let mut session_pages = HashMap::new();
        session_pages.insert("sess-1".to_string(), 0);

        let tc = ToolCall {
            id: "tc-1".to_string(),
            name: "update_memory".to_string(),
            arguments: r#"{"memory_type":"other","target_name":"Old User","old_text":"","new_text":"他喜欢喝茶"}"#.to_string(),
        };
        let result = executor.execute("agent-1", vec![tc], &session_pages).await;
        assert!(result.is_ok(), "Expected success but got: {:?}", result);

        let conn_guard = executor.db_state.0.lock().await;
        let memory: String = conn_guard.query_row(
            "SELECT memory_text FROM agent_relationships WHERE observer_id = 'agent-1' AND target_id = 'up-old' AND target_type = 'user_persona'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(memory, "他喜欢喝茶");

        // Ensure memory was NOT attached to up-new (current active)
        let count_new: i32 = conn_guard.query_row(
            "SELECT COUNT(*) FROM agent_relationships WHERE observer_id = 'agent-1' AND target_id = 'up-new'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(count_new, 0, "Memory should not be attached to current active persona up-new");
    }
}
