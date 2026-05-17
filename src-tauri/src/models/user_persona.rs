use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct UserPersona {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub avatar_path: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserPersonaRequest {
    pub name: String,
    pub description: Option<String>,
    pub avatar_path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserPersonaRequest {
    pub id: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub avatar_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CurrentUserPersonaResponse {
    pub id: Option<String>,
    pub name: String,
    pub description: String,
    pub avatar_path: Option<String>,
    pub is_custom: bool,
}
