use crate::db::connection::DbState;
use crate::llm::persona_generation;
use crate::models::generate_persona::{GeneratePersonaRequest, GeneratePersonaResponse};

#[tauri::command]
pub async fn generate_persona(
    db_state: tauri::State<'_, DbState>,
    req: GeneratePersonaRequest,
) -> Result<GeneratePersonaResponse, String> {
    crate::logger::debug(&format!(
        "[DEBUG generate_persona] agent_id={:?}, has_ref={}, has_supp={}",
        req.agent_id,
        req.reference_character.as_ref().map(|s| !s.is_empty()).unwrap_or(false),
        req.supplement.as_ref().map(|s| !s.is_empty()).unwrap_or(false),
    ));

    let result = persona_generation::generate(&db_state, &req).await;

    match &result {
        Ok(r) => crate::logger::debug(&format!(
            "[DEBUG generate_persona] success detailed_len={} simplified_len={}",
            r.detailed_persona.len(),
            r.simplified_persona.len(),
        )),
        Err(e) => crate::logger::error(&format!("[DEBUG generate_persona] failed: {}", e)),
    }

    result
}
