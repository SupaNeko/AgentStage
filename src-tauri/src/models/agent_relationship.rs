use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRelationship {
    pub observer_id: String,
    pub target_id: String,
    pub target_type: String,
    pub relationship_text: String,
    pub updated_at: i64,
}

/// 前端"关系设定"标签页展示用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelationshipItem {
    pub target_id: String,
    pub target_type: String,
    pub target_name: String,
    pub target_avatar: Option<String>,
    pub target_label: String,
    pub relationship_text: String,
    pub updated_at: i64,
}
