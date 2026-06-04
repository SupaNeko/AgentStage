use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::db::agent_bundle as agent_bundle_repo;
use crate::db::connection::{get_db, DbState};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewAgentBundleExportRequest {
    pub agent_ids: Vec<String>,
    pub user_persona_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportAgentBundleRequest {
    pub agent_ids: Vec<String>,
    pub user_persona_ids: Vec<String>,
    pub confirm_omissions: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewAgentBundleImportRequest {
    pub file_content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportAgentBundleRequest {
    pub file_content: String,
    pub agents: Vec<ImportAgentBundleAgentSelection>,
    pub user_personas: Vec<ImportAgentBundleUserPersonaSelection>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportAgentBundleAgentSelection {
    pub bundle_id: String,
    pub name: String,
    pub model_config_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportAgentBundleUserPersonaSelection {
    pub bundle_id: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentBundleExportPreviewResponse {
    pub agent_count: usize,
    pub user_persona_count: usize,
    pub omitted_relationship_count: usize,
    pub omitted_relationship_memory_count: usize,
    pub omitted_friendship_count: usize,
    pub warnings: Vec<String>,
    pub requires_confirmation: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentBundleExportResultResponse {
    pub preview: AgentBundleExportPreviewResponse,
    pub exported_path: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentBundleImportPreviewResponse {
    pub agent_count: usize,
    pub user_persona_count: usize,
    pub agents: Vec<AgentBundleImportPreviewAgentResponse>,
    pub user_personas: Vec<AgentBundleImportPreviewUserPersonaResponse>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentBundleImportPreviewAgentResponse {
    pub bundle_id: String,
    pub original_name: String,
    pub suggested_name: String,
    pub avatar_data_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentBundleImportPreviewUserPersonaResponse {
    pub bundle_id: String,
    pub original_name: String,
    pub suggested_name: String,
    pub avatar_data_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentBundleImportResultResponse {
    pub imported_agent_count: usize,
    pub imported_user_persona_count: usize,
    pub warnings: Vec<String>,
    pub renamed: bool,
}

#[tauri::command]
pub async fn preview_agent_bundle_export(
    state: State<'_, DbState>,
    req: PreviewAgentBundleExportRequest,
) -> Result<AgentBundleExportPreviewResponse, String> {
    let conn = get_db(&state).await?;
    let preview =
        agent_bundle_repo::preview_export_bundle(&conn, &req.agent_ids, &req.user_persona_ids)?;
    Ok(preview.into())
}

#[tauri::command]
pub async fn export_agent_bundle(
    state: State<'_, DbState>,
    req: ExportAgentBundleRequest,
) -> Result<AgentBundleExportResultResponse, String> {
    let export_root = get_export_root()?;
    let build = {
        let conn = get_db(&state).await?;
        agent_bundle_repo::build_export_bundle(
            &conn,
            &req.agent_ids,
            &req.user_persona_ids,
            req.confirm_omissions,
        )?
    };
    let result = agent_bundle_repo::write_export_bundle_to_file(&export_root, build)?;
    Ok(result.into())
}

#[tauri::command]
pub async fn preview_agent_bundle_import(
    state: State<'_, DbState>,
    req: PreviewAgentBundleImportRequest,
) -> Result<AgentBundleImportPreviewResponse, String> {
    let bundle = agent_bundle_repo::parse_agent_bundle_json(&req.file_content)?;
    let conn = get_db(&state).await?;
    let preview = agent_bundle_repo::preview_import_bundle_from_bundle(&conn, &bundle)?;
    Ok(preview.into())
}

#[tauri::command]
pub async fn import_agent_bundle(
    state: State<'_, DbState>,
    req: ImportAgentBundleRequest,
) -> Result<AgentBundleImportResultResponse, String> {
    let bundle = agent_bundle_repo::parse_agent_bundle_json(&req.file_content)?;
    let data_dir = crate::get_data_dir().map_err(|e| e.to_string())?;
    let agent_selections: Vec<agent_bundle_repo::ImportAgentSelection> = req
        .agents
        .into_iter()
        .map(|selection| agent_bundle_repo::ImportAgentSelection {
            bundle_id: selection.bundle_id,
            name: selection.name,
            model_config_id: selection.model_config_id,
        })
        .collect();
    let user_persona_selections: Vec<agent_bundle_repo::ImportUserPersonaSelection> = req
        .user_personas
        .into_iter()
        .map(|selection| agent_bundle_repo::ImportUserPersonaSelection {
            bundle_id: selection.bundle_id,
            name: selection.name,
        })
        .collect();

    let conn = get_db(&state).await?;
    let result = agent_bundle_repo::import_bundle_from_bundle(
        &conn,
        &data_dir,
        &bundle,
        &agent_selections,
        &user_persona_selections,
    )?;
    Ok(result.into())
}

fn get_export_root() -> Result<PathBuf, String> {
    let data_dir = crate::get_data_dir().map_err(|e| e.to_string())?;
    let app_root = data_dir
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.clone());
    Ok(app_root.join("exports"))
}

impl From<agent_bundle_repo::ExportBundlePreview> for AgentBundleExportPreviewResponse {
    fn from(preview: agent_bundle_repo::ExportBundlePreview) -> Self {
        Self {
            agent_count: preview.agent_count,
            user_persona_count: preview.user_persona_count,
            omitted_relationship_count: preview.omitted_relationship_count,
            omitted_relationship_memory_count: preview.omitted_relationship_memory_count,
            omitted_friendship_count: preview.omitted_friendship_count,
            warnings: preview.warnings,
            requires_confirmation: preview.requires_confirmation,
        }
    }
}

impl From<agent_bundle_repo::ExportBundleResult> for AgentBundleExportResultResponse {
    fn from(result: agent_bundle_repo::ExportBundleResult) -> Self {
        Self {
            preview: result.preview.into(),
            exported_path: result.exported_path,
            warnings: result.warnings,
        }
    }
}

impl From<agent_bundle_repo::ImportBundlePreview> for AgentBundleImportPreviewResponse {
    fn from(preview: agent_bundle_repo::ImportBundlePreview) -> Self {
        Self {
            agent_count: preview.agent_count,
            user_persona_count: preview.user_persona_count,
            agents: preview.agents.into_iter().map(Into::into).collect(),
            user_personas: preview.user_personas.into_iter().map(Into::into).collect(),
            warnings: preview.warnings,
        }
    }
}

impl From<agent_bundle_repo::ImportPreviewAgent> for AgentBundleImportPreviewAgentResponse {
    fn from(agent: agent_bundle_repo::ImportPreviewAgent) -> Self {
        Self {
            bundle_id: agent.bundle_id,
            original_name: agent.original_name,
            suggested_name: agent.suggested_name,
            avatar_data_url: agent.avatar_data_url,
        }
    }
}

impl From<agent_bundle_repo::ImportPreviewUserPersona>
    for AgentBundleImportPreviewUserPersonaResponse
{
    fn from(user_persona: agent_bundle_repo::ImportPreviewUserPersona) -> Self {
        Self {
            bundle_id: user_persona.bundle_id,
            original_name: user_persona.original_name,
            suggested_name: user_persona.suggested_name,
            avatar_data_url: user_persona.avatar_data_url,
        }
    }
}

impl From<agent_bundle_repo::ImportBundleResult> for AgentBundleImportResultResponse {
    fn from(result: agent_bundle_repo::ImportBundleResult) -> Self {
        Self {
            imported_agent_count: result.imported_agent_count,
            imported_user_persona_count: result.imported_user_persona_count,
            warnings: result.warnings,
            renamed: result.renamed,
        }
    }
}
