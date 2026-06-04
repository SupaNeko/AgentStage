use base64::{engine::general_purpose, Engine as _};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const BUNDLE_FORMAT: &str = "agentstage.bundle";
pub const BUNDLE_VERSION: i32 = 1;

const TARGET_AGENT: &str = "agent";
const TARGET_USER_PERSONA: &str = "user_persona";
const PREVIEW_AVATAR_MAX_DECODED_BYTES: usize = 512 * 1024;
const PREVIEW_AVATAR_MAX_DATA_URL_BYTES: usize = 700 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStageBundle {
    pub format: String,
    pub version: i32,
    pub exported_at: i64,
    pub agents: Vec<BundleAgent>,
    pub user_personas: Vec<BundleUserPersona>,
    pub relationships: Vec<BundleRelationship>,
    pub friendships: Vec<BundleFriendship>,
    pub assets: Vec<BundleAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleAgent {
    pub id: String,
    pub name: String,
    pub avatar_asset_id: Option<String>,
    pub detailed_persona: String,
    pub simplified_persona: String,
    pub personality: Option<String>,
    pub scenario: Option<String>,
    pub example_messages: Option<String>,
    pub first_message: Option<String>,
    pub creator_notes: Option<String>,
    pub tags: Option<String>,
    pub long_term_memory: Option<String>,
    pub memory_enabled: bool,
    pub proactive_enabled: bool,
    pub proactive_min_minutes: i32,
    pub proactive_max_minutes: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleUserPersona {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub avatar_asset_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleRelationship {
    pub observer_id: String,
    pub target_id: String,
    pub target_type: String,
    pub relationship_text: String,
    pub memory_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleFriendship {
    pub agent_1_id: String,
    pub agent_2_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleAsset {
    pub id: String,
    pub original_path: Option<String>,
    pub mime_type: String,
    pub base64_content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExportBundlePreview {
    pub agent_count: usize,
    pub user_persona_count: usize,
    pub omitted_relationship_count: usize,
    pub omitted_relationship_memory_count: usize,
    pub omitted_friendship_count: usize,
    pub warnings: Vec<String>,
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportBundleResult {
    pub preview: ExportBundlePreview,
    pub exported_path: Option<String>,
    pub bundle_json: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ExportBundleBuild {
    pub preview: ExportBundlePreview,
    pub bundle: Option<AgentStageBundle>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportBundlePreview {
    pub agent_count: usize,
    pub user_persona_count: usize,
    pub agents: Vec<ImportPreviewAgent>,
    pub user_personas: Vec<ImportPreviewUserPersona>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreviewAgent {
    pub bundle_id: String,
    pub original_name: String,
    pub suggested_name: String,
    pub avatar_data_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreviewUserPersona {
    pub bundle_id: String,
    pub original_name: String,
    pub suggested_name: String,
    pub avatar_data_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportAgentSelection {
    pub bundle_id: String,
    pub name: String,
    pub model_config_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportUserPersonaSelection {
    pub bundle_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportBundleRequest {
    pub json: String,
    pub agents: Vec<ImportAgentSelection>,
    pub user_personas: Vec<ImportUserPersonaSelection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportBundleResult {
    pub imported_agent_count: usize,
    pub imported_user_persona_count: usize,
    pub warnings: Vec<String>,
    pub renamed: bool,
    pub agent_id_map: HashMap<String, String>,
    pub user_persona_id_map: HashMap<String, String>,
}

#[derive(Debug)]
struct WrittenAvatar {
    relative_path: String,
    absolute_path: PathBuf,
}

struct FinalImportPlan {
    agent_names: HashMap<String, String>,
    user_persona_names: HashMap<String, String>,
    agent_model_config_ids: HashMap<String, Option<String>>,
    renamed: bool,
}

#[derive(Debug)]
struct DbAgentForBundle {
    id: String,
    name: String,
    avatar_path: Option<String>,
    detailed_persona: String,
    simplified_persona: String,
    personality: Option<String>,
    scenario: Option<String>,
    example_messages: Option<String>,
    first_message: Option<String>,
    creator_notes: Option<String>,
    tags: Option<String>,
    long_term_memory: Option<String>,
    memory_enabled: bool,
    proactive_enabled: bool,
    proactive_min_minutes: i32,
    proactive_max_minutes: i32,
}

#[derive(Debug)]
struct DbUserPersonaForBundle {
    id: String,
    name: String,
    description: Option<String>,
    avatar_path: Option<String>,
}

pub fn preview_export_bundle(
    conn: &Connection,
    agent_ids: &[String],
    user_persona_ids: &[String],
) -> Result<ExportBundlePreview, String> {
    if agent_ids.is_empty() && user_persona_ids.is_empty() {
        return Err("璇烽€夋嫨鑷冲皯涓€涓鑹叉垨鐢ㄦ埛浜鸿".to_string());
    }

    let selected_agents = selected_existing_agents(conn, agent_ids)?;
    let selected_personas = selected_existing_personas(conn, user_persona_ids)?;
    if selected_agents.is_empty() && selected_personas.is_empty() {
        return Err("娌℃湁鍙鍑虹殑閰嶇疆".to_string());
    }

    let selected_agent_ids: HashSet<String> =
        selected_agents.iter().map(|a| a.id.clone()).collect();
    let selected_user_persona_ids: HashSet<String> =
        selected_personas.iter().map(|p| p.id.clone()).collect();
    let (omitted_relationship_count, omitted_relationship_memory_count) =
        count_omitted_relationships(conn, &selected_agent_ids, &selected_user_persona_ids)?;
    let omitted_friendship_count = count_omitted_friendships(conn, &selected_agent_ids)?;

    let mut warnings = Vec::new();
    if omitted_relationship_count > 0
        || omitted_relationship_memory_count > 0
        || omitted_friendship_count > 0
    {
        warnings.push(format!(
            "{} relationship descriptions, {} relationship memories, and {} friendships point to unselected objects and will be omitted.",
            omitted_relationship_count, omitted_relationship_memory_count, omitted_friendship_count
        ));
    }

    Ok(ExportBundlePreview {
        agent_count: selected_agents.len(),
        user_persona_count: selected_personas.len(),
        omitted_relationship_count,
        omitted_relationship_memory_count,
        omitted_friendship_count,
        requires_confirmation: !warnings.is_empty(),
        warnings,
    })
}

pub fn export_bundle_to_file(
    conn: &Connection,
    export_root: &Path,
    agent_ids: &[String],
    user_persona_ids: &[String],
    confirm_omissions: bool,
) -> Result<ExportBundleResult, String> {
    let build = build_export_bundle(conn, agent_ids, user_persona_ids, confirm_omissions)?;
    write_export_bundle_to_file(export_root, build)
}

pub fn build_export_bundle(
    conn: &Connection,
    agent_ids: &[String],
    user_persona_ids: &[String],
    confirm_omissions: bool,
) -> Result<ExportBundleBuild, String> {
    let preview = preview_export_bundle(conn, agent_ids, user_persona_ids)?;
    if preview.requires_confirmation && !confirm_omissions {
        return Ok(ExportBundleBuild {
            warnings: preview.warnings.clone(),
            preview,
            bundle: None,
        });
    }

    let (bundle, warnings) = build_bundle(conn, agent_ids, user_persona_ids)?;
    Ok(ExportBundleBuild {
        preview,
        bundle: Some(bundle),
        warnings,
    })
}

pub fn write_export_bundle_to_file(
    export_root: &Path,
    build: ExportBundleBuild,
) -> Result<ExportBundleResult, String> {
    let Some(bundle) = build.bundle else {
        return Ok(ExportBundleResult {
            preview: build.preview,
            exported_path: None,
            bundle_json: None,
            warnings: build.warnings,
        });
    };

    let json = serde_json::to_string_pretty(&bundle).map_err(|e| e.to_string())?;
    let bundle_dir = export_root.join("bundles");
    fs::create_dir_all(&bundle_dir).map_err(|e| e.to_string())?;
    let path = bundle_dir.join(format!(
        "agentstage-bundle-{}.agentstage",
        chrono::Utc::now().timestamp_millis()
    ));
    fs::write(&path, &json).map_err(|e| e.to_string())?;

    Ok(ExportBundleResult {
        preview: build.preview,
        exported_path: Some(path.to_string_lossy().to_string()),
        bundle_json: Some(json),
        warnings: build.warnings,
    })
}

pub fn preview_import_bundle(conn: &Connection, json: &str) -> Result<ImportBundlePreview, String> {
    let bundle = parse_agent_bundle_json(json)?;
    preview_import_bundle_from_bundle(conn, &bundle)
}

pub fn parse_agent_bundle_json(json: &str) -> Result<AgentStageBundle, String> {
    parse_and_validate_bundle(json)
}

pub fn preview_import_bundle_from_bundle(
    conn: &Connection,
    bundle: &AgentStageBundle,
) -> Result<ImportBundlePreview, String> {
    let asset_map: HashMap<&str, &BundleAsset> =
        bundle.assets.iter().map(|a| (a.id.as_str(), a)).collect();
    let mut used_names = existing_names(conn)?;

    let mut warnings = Vec::new();
    let mut agents = Vec::new();
    for agent in &bundle.agents {
        let suggested_name = suggest_unique_import_name(&agent.name, &mut used_names);
        agents.push(ImportPreviewAgent {
            bundle_id: agent.id.clone(),
            original_name: agent.name.clone(),
            suggested_name,
            avatar_data_url: avatar_data_url(
                agent.avatar_asset_id.as_deref(),
                &asset_map,
                &mut warnings,
            ),
        });
    }

    let mut user_personas = Vec::new();
    for persona in &bundle.user_personas {
        let suggested_name = suggest_unique_import_name(&persona.name, &mut used_names);
        user_personas.push(ImportPreviewUserPersona {
            bundle_id: persona.id.clone(),
            original_name: persona.name.clone(),
            suggested_name,
            avatar_data_url: avatar_data_url(
                persona.avatar_asset_id.as_deref(),
                &asset_map,
                &mut warnings,
            ),
        });
    }

    Ok(ImportBundlePreview {
        agent_count: agents.len(),
        user_persona_count: user_personas.len(),
        agents,
        user_personas,
        warnings,
    })
}

pub fn import_bundle(
    conn: &Connection,
    data_dir: &Path,
    json: &str,
    agent_selections: &[ImportAgentSelection],
    user_persona_selections: &[ImportUserPersonaSelection],
) -> Result<ImportBundleResult, String> {
    let bundle = parse_agent_bundle_json(json)?;
    import_bundle_from_bundle(
        conn,
        data_dir,
        &bundle,
        agent_selections,
        user_persona_selections,
    )
}

pub fn import_bundle_from_bundle(
    conn: &Connection,
    data_dir: &Path,
    bundle: &AgentStageBundle,
    agent_selections: &[ImportAgentSelection],
    user_persona_selections: &[ImportUserPersonaSelection],
) -> Result<ImportBundleResult, String> {
    let preview = preview_import_bundle_from_bundle(conn, bundle)?;
    let import_plan = validate_final_import_names(
        conn,
        bundle,
        &preview,
        agent_selections,
        user_persona_selections,
    )?;
    conn.execute_batch("BEGIN IMMEDIATE")
        .map_err(|e| e.to_string())?;

    let result = import_bundle_in_transaction(
        conn,
        data_dir,
        bundle,
        &preview,
        agent_selections,
        user_persona_selections,
        &import_plan,
    );

    match result {
        Ok((result, written_avatar_paths)) => {
            if let Err(e) = conn.execute_batch("COMMIT") {
                let _ = conn.execute_batch("ROLLBACK");
                cleanup_written_avatars(&written_avatar_paths);
                return Err(e.to_string());
            }
            Ok(result)
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            cleanup_written_avatars(e.written_avatar_paths.as_slice());
            Err(e.message)
        }
    }
}

struct ImportFailure {
    message: String,
    written_avatar_paths: Vec<PathBuf>,
}

fn validate_final_import_names(
    conn: &Connection,
    bundle: &AgentStageBundle,
    preview: &ImportBundlePreview,
    agent_selections: &[ImportAgentSelection],
    user_persona_selections: &[ImportUserPersonaSelection],
) -> Result<FinalImportPlan, String> {
    let agent_selection_map: HashMap<&str, &ImportAgentSelection> = agent_selections
        .iter()
        .map(|s| (s.bundle_id.as_str(), s))
        .collect();
    let persona_selection_map: HashMap<&str, &ImportUserPersonaSelection> = user_persona_selections
        .iter()
        .map(|s| (s.bundle_id.as_str(), s))
        .collect();
    let mut used_names = existing_names(conn)?;
    let mut agent_names = HashMap::new();
    let mut user_persona_names = HashMap::new();
    let mut agent_model_config_ids = HashMap::new();
    let mut renamed = false;

    for agent in &bundle.agents {
        let preview_agent = preview
            .agents
            .iter()
            .find(|a| a.bundle_id == agent.id)
            .ok_or_else(|| "Import bundle is missing final agent name.".to_string())?;
        let selection = agent_selection_map.get(agent.id.as_str());
        let final_name = selection
            .map(|s| s.name.as_str())
            .unwrap_or(preview_agent.suggested_name.as_str())
            .trim()
            .to_string();
        validate_available_import_name(&final_name, &mut used_names)?;
        if final_name != agent.name {
            renamed = true;
        }
        agent_names.insert(agent.id.clone(), final_name);
        agent_model_config_ids.insert(
            agent.id.clone(),
            selection.and_then(|s| s.model_config_id.clone()),
        );
    }

    for persona in &bundle.user_personas {
        let preview_persona = preview
            .user_personas
            .iter()
            .find(|p| p.bundle_id == persona.id)
            .ok_or_else(|| "Import bundle is missing final user persona name.".to_string())?;
        let selection = persona_selection_map.get(persona.id.as_str());
        let final_name = selection
            .map(|s| s.name.as_str())
            .unwrap_or(preview_persona.suggested_name.as_str())
            .trim()
            .to_string();
        validate_available_import_name(&final_name, &mut used_names)?;
        if final_name != persona.name {
            renamed = true;
        }
        user_persona_names.insert(persona.id.clone(), final_name);
    }

    Ok(FinalImportPlan {
        agent_names,
        user_persona_names,
        agent_model_config_ids,
        renamed,
    })
}

fn validate_available_import_name(
    name: &str,
    used_names: &mut HashSet<String>,
) -> Result<(), String> {
    if name.is_empty() {
        return Err("Import name cannot be empty.".to_string());
    }
    if !used_names.insert(name.to_string()) {
        return Err(format!(
            "Import name conflicts with an existing or selected name: {}",
            name
        ));
    }
    Ok(())
}

fn import_bundle_in_transaction(
    conn: &Connection,
    data_dir: &Path,
    bundle: &AgentStageBundle,
    preview: &ImportBundlePreview,
    agent_selections: &[ImportAgentSelection],
    user_persona_selections: &[ImportUserPersonaSelection],
    import_plan: &FinalImportPlan,
) -> Result<(ImportBundleResult, Vec<PathBuf>), ImportFailure> {
    let asset_map: HashMap<String, BundleAsset> = bundle
        .assets
        .iter()
        .map(|a| (a.id.clone(), a.clone()))
        .collect();
    let agent_selection_map: HashMap<&str, &ImportAgentSelection> = agent_selections
        .iter()
        .map(|s| (s.bundle_id.as_str(), s))
        .collect();
    let persona_selection_map: HashMap<&str, &ImportUserPersonaSelection> = user_persona_selections
        .iter()
        .map(|s| (s.bundle_id.as_str(), s))
        .collect();

    let mut warnings = Vec::new();
    let mut renamed = false;
    let mut agent_id_map = HashMap::new();
    let mut user_persona_id_map = HashMap::new();
    let mut written_avatar_paths = Vec::new();
    let now = chrono::Utc::now().timestamp_millis();

    for agent in &bundle.agents {
        let preview_agent = preview
            .agents
            .iter()
            .find(|a| a.bundle_id == agent.id)
            .ok_or_else(|| import_failure("瀵煎叆鏂囦欢缂哄皯蹇呰瀛楁", &written_avatar_paths))?;
        let selection = agent_selection_map.get(agent.id.as_str());
        let final_name = selection
            .map(|s| s.name.clone())
            .unwrap_or_else(|| preview_agent.suggested_name.clone());
        if final_name != agent.name {
            renamed = true;
        }
        let final_name = import_plan.agent_names.get(&agent.id).ok_or_else(|| {
            import_failure(
                "Import bundle is missing final agent name.",
                &written_avatar_paths,
            )
        })?;
        let model_config_id = import_plan
            .agent_model_config_ids
            .get(&agent.id)
            .cloned()
            .flatten();
        let new_id = Uuid::new_v4().to_string();
        let avatar = write_imported_avatar(
            data_dir,
            TARGET_AGENT,
            agent.avatar_asset_id.as_deref(),
            &asset_map,
            &mut warnings,
        );
        let avatar_path = avatar.as_ref().map(|a| a.relative_path.clone());
        if let Some(avatar) = avatar {
            written_avatar_paths.push(avatar.absolute_path);
        }

        conn.execute(
            r#"INSERT INTO agents (
                id, name, avatar_path, detailed_persona, simplified_persona,
                personality, scenario, example_messages, first_message, creator_notes, tags,
                model_config_id, agent_temperature, long_term_memory, memory_enabled,
                proactive_enabled, proactive_min_minutes, proactive_max_minutes, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, NULL, ?13, ?14, ?15, ?16, ?17, ?18, ?18)"#,
            params![
                &new_id,
                &final_name,
                &avatar_path,
                &agent.detailed_persona,
                &agent.simplified_persona,
                &agent.personality,
                &agent.scenario,
                &agent.example_messages,
                &agent.first_message,
                &agent.creator_notes,
                &agent.tags,
                &model_config_id,
                &agent.long_term_memory,
                agent.memory_enabled as i32,
                agent.proactive_enabled as i32,
                agent.proactive_min_minutes,
                agent.proactive_max_minutes,
                now,
            ],
        )
        .map_err(|e| import_failure(e.to_string(), &written_avatar_paths))?;
        agent_id_map.insert(agent.id.clone(), new_id);
    }

    for persona in &bundle.user_personas {
        let preview_persona = preview
            .user_personas
            .iter()
            .find(|p| p.bundle_id == persona.id)
            .ok_or_else(|| import_failure("瀵煎叆鏂囦欢缂哄皯蹇呰瀛楁", &written_avatar_paths))?;
        let selection = persona_selection_map.get(persona.id.as_str());
        let final_name = selection
            .map(|s| s.name.clone())
            .unwrap_or_else(|| preview_persona.suggested_name.clone());
        if final_name != persona.name {
            renamed = true;
        }
        let final_name = import_plan.user_persona_names.get(&persona.id).ok_or_else(|| {
            import_failure(
                "Import bundle is missing final user persona name.",
                &written_avatar_paths,
            )
        })?;
        let new_id = Uuid::new_v4().to_string();
        let avatar = write_imported_avatar(
            data_dir,
            TARGET_USER_PERSONA,
            persona.avatar_asset_id.as_deref(),
            &asset_map,
            &mut warnings,
        );
        let avatar_path = avatar.as_ref().map(|a| a.relative_path.clone());
        if let Some(avatar) = avatar {
            written_avatar_paths.push(avatar.absolute_path);
        }
        conn.execute(
            "INSERT INTO user_personas (id, name, description, avatar_path, is_default, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5)",
            params![&new_id, &final_name, &persona.description, &avatar_path, now],
        )
        .map_err(|e| import_failure(e.to_string(), &written_avatar_paths))?;
        user_persona_id_map.insert(persona.id.clone(), new_id);
    }

    for relationship in &bundle.relationships {
        let observer_id = agent_id_map
            .get(&relationship.observer_id)
            .ok_or_else(|| import_failure("瀵煎叆鏂囦欢缂哄皯蹇呰瀛楁", &written_avatar_paths))?;
        let target_id = if relationship.target_type == TARGET_AGENT {
            agent_id_map.get(&relationship.target_id)
        } else {
            user_persona_id_map.get(&relationship.target_id)
        }
        .ok_or_else(|| import_failure("瀵煎叆鏂囦欢缂哄皯蹇呰瀛楁", &written_avatar_paths))?;

        conn.execute(
            "INSERT INTO agent_relationships (observer_id, target_id, target_type, relationship_text, memory_text, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                observer_id,
                target_id,
                &relationship.target_type,
                &relationship.relationship_text,
                &relationship.memory_text,
                now,
            ],
        )
        .map_err(|e| import_failure(e.to_string(), &written_avatar_paths))?;
    }

    let mut imported_friend_pairs = HashSet::new();
    for friendship in &bundle.friendships {
        let agent_1_id = agent_id_map
            .get(&friendship.agent_1_id)
            .ok_or_else(|| import_failure("瀵煎叆鏂囦欢缂哄皯蹇呰瀛楁", &written_avatar_paths))?;
        let agent_2_id = agent_id_map
            .get(&friendship.agent_2_id)
            .ok_or_else(|| import_failure("瀵煎叆鏂囦欢缂哄皯蹇呰瀛楁", &written_avatar_paths))?;
        let pair = unordered_pair(agent_1_id, agent_2_id);
        if imported_friend_pairs.insert(pair) {
            add_imported_friendship(conn, agent_1_id, agent_2_id, now)
                .map_err(|e| import_failure(e.to_string(), &written_avatar_paths))?;
        }
    }

    Ok((
        ImportBundleResult {
            imported_agent_count: agent_id_map.len(),
            imported_user_persona_count: user_persona_id_map.len(),
            warnings,
            renamed: renamed || import_plan.renamed,
            agent_id_map,
            user_persona_id_map,
        },
        written_avatar_paths,
    ))
}

fn import_failure(message: impl Into<String>, written_avatar_paths: &[PathBuf]) -> ImportFailure {
    ImportFailure {
        message: message.into(),
        written_avatar_paths: written_avatar_paths.to_vec(),
    }
}

fn cleanup_written_avatars(paths: &[PathBuf]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

fn add_imported_friendship(
    conn: &Connection,
    agent_id_1: &str,
    agent_id_2: &str,
    now: i64,
) -> Result<(), rusqlite::Error> {
    insert_friendship_direction_if_missing(conn, agent_id_1, agent_id_2, now)?;
    insert_friendship_direction_if_missing(conn, agent_id_2, agent_id_1, now)?;
    Ok(())
}

fn insert_friendship_direction_if_missing(
    conn: &Connection,
    agent_id_1: &str,
    agent_id_2: &str,
    now: i64,
) -> Result<(), rusqlite::Error> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM friendships WHERE agent_id_1 = ?1 AND agent_id_2 = ?2 AND participant_type_2 = 'agent')",
        params![agent_id_1, agent_id_2],
        |row| row.get(0),
    )?;
    if !exists {
        conn.execute(
            "INSERT INTO friendships (id, agent_id_1, agent_id_2, participant_type_2, created_at, source_session_id) VALUES (?1, ?2, ?3, 'agent', ?4, NULL)",
            params![Uuid::new_v4().to_string(), agent_id_1, agent_id_2, now],
        )?;
    }
    Ok(())
}

fn build_bundle(
    conn: &Connection,
    agent_ids: &[String],
    user_persona_ids: &[String],
) -> Result<(AgentStageBundle, Vec<String>), String> {
    let agents = selected_existing_agents(conn, agent_ids)?;
    let personas = selected_existing_personas(conn, user_persona_ids)?;
    let selected_agent_ids: HashSet<String> = agents.iter().map(|a| a.id.clone()).collect();
    let selected_persona_ids: HashSet<String> = personas.iter().map(|p| p.id.clone()).collect();
    let mut assets = Vec::new();
    let mut warnings = Vec::new();

    let bundle_agents = agents
        .into_iter()
        .map(|agent| {
            let avatar_asset_id =
                embed_avatar_asset(agent.avatar_path.as_deref(), &mut assets, &mut warnings);
            BundleAgent {
                id: bundle_agent_id(&agent.id),
                name: agent.name,
                avatar_asset_id,
                detailed_persona: agent.detailed_persona,
                simplified_persona: agent.simplified_persona,
                personality: agent.personality,
                scenario: agent.scenario,
                example_messages: agent.example_messages,
                first_message: agent.first_message,
                creator_notes: agent.creator_notes,
                tags: agent.tags,
                long_term_memory: agent.long_term_memory,
                memory_enabled: agent.memory_enabled,
                proactive_enabled: agent.proactive_enabled,
                proactive_min_minutes: agent.proactive_min_minutes,
                proactive_max_minutes: agent.proactive_max_minutes,
            }
        })
        .collect();

    let bundle_user_personas = personas
        .into_iter()
        .map(|persona| {
            let avatar_asset_id =
                embed_avatar_asset(persona.avatar_path.as_deref(), &mut assets, &mut warnings);
            BundleUserPersona {
                id: bundle_user_persona_id(&persona.id),
                name: persona.name,
                description: persona.description,
                avatar_asset_id,
            }
        })
        .collect();

    let relationships = export_relationships(conn, &selected_agent_ids, &selected_persona_ids)?;
    let friendships = export_friendships(conn, &selected_agent_ids)?;

    Ok((
        AgentStageBundle {
            format: BUNDLE_FORMAT.to_string(),
            version: BUNDLE_VERSION,
            exported_at: chrono::Utc::now().timestamp_millis(),
            agents: bundle_agents,
            user_personas: bundle_user_personas,
            relationships,
            friendships,
            assets,
        },
        warnings,
    ))
}

fn selected_existing_agents(
    conn: &Connection,
    agent_ids: &[String],
) -> Result<Vec<DbAgentForBundle>, String> {
    let mut agents = Vec::new();
    for id in ordered_unique(agent_ids) {
        let agent = conn
            .query_row(
                r#"SELECT id, name, avatar_path, detailed_persona, simplified_persona,
                   personality, scenario, example_messages, first_message, creator_notes, tags,
                   long_term_memory, memory_enabled, proactive_enabled, proactive_min_minutes, proactive_max_minutes
                   FROM agents WHERE id = ?1 AND is_deleted = 0"#,
                [id.as_str()],
                |row| {
                    Ok(DbAgentForBundle {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        avatar_path: row.get(2)?,
                        detailed_persona: row.get(3)?,
                        simplified_persona: row.get(4)?,
                        personality: row.get(5)?,
                        scenario: row.get(6)?,
                        example_messages: row.get(7)?,
                        first_message: row.get(8)?,
                        creator_notes: row.get(9)?,
                        tags: row.get(10)?,
                        long_term_memory: row.get(11)?,
                        memory_enabled: row.get::<_, i32>(12)? != 0,
                        proactive_enabled: row.get::<_, i32>(13)? != 0,
                        proactive_min_minutes: row.get(14)?,
                        proactive_max_minutes: row.get(15)?,
                    })
                },
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(agent) = agent {
            agents.push(agent);
        }
    }
    Ok(agents)
}

fn selected_existing_personas(
    conn: &Connection,
    user_persona_ids: &[String],
) -> Result<Vec<DbUserPersonaForBundle>, String> {
    let mut personas = Vec::new();
    for id in ordered_unique(user_persona_ids) {
        let persona = conn
            .query_row(
                "SELECT id, name, description, avatar_path FROM user_personas WHERE id = ?1",
                [id.as_str()],
                |row| {
                    Ok(DbUserPersonaForBundle {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        description: row.get(2)?,
                        avatar_path: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if let Some(persona) = persona {
            personas.push(persona);
        }
    }
    Ok(personas)
}

fn count_omitted_relationships(
    conn: &Connection,
    selected_agent_ids: &HashSet<String>,
    selected_user_persona_ids: &HashSet<String>,
) -> Result<(usize, usize), String> {
    let mut stmt = conn
        .prepare(
            "SELECT observer_id, target_id, target_type, relationship_text, memory_text FROM agent_relationships",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .map_err(|e| e.to_string())?;

    let mut relationship_count = 0;
    let mut memory_count = 0;
    for row in rows {
        let (observer_id, target_id, target_type, relationship_text, memory_text) =
            row.map_err(|e| e.to_string())?;
        if !selected_agent_ids.contains(&observer_id) {
            continue;
        }
        let target_included = match target_type.as_str() {
            TARGET_AGENT => selected_agent_ids.contains(&target_id),
            TARGET_USER_PERSONA => selected_user_persona_ids.contains(&target_id),
            _ => false,
        };
        if target_included {
            continue;
        }
        if !relationship_text.is_empty() {
            relationship_count += 1;
        }
        if !memory_text.is_empty() {
            memory_count += 1;
        }
    }
    Ok((relationship_count, memory_count))
}

fn count_omitted_friendships(
    conn: &Connection,
    selected_agent_ids: &HashSet<String>,
) -> Result<usize, String> {
    let mut stmt = conn
        .prepare(
            "SELECT agent_id_1, agent_id_2 FROM friendships WHERE participant_type_2 = 'agent'",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;
    let mut pairs = HashSet::new();
    for row in rows {
        let (a, b) = row.map_err(|e| e.to_string())?;
        let selected_a = selected_agent_ids.contains(&a);
        let selected_b = selected_agent_ids.contains(&b);
        if selected_a ^ selected_b {
            pairs.insert(unordered_pair(&a, &b));
        }
    }
    Ok(pairs.len())
}

fn export_relationships(
    conn: &Connection,
    selected_agent_ids: &HashSet<String>,
    selected_user_persona_ids: &HashSet<String>,
) -> Result<Vec<BundleRelationship>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT observer_id, target_id, target_type, relationship_text, memory_text FROM agent_relationships ORDER BY observer_id, target_type, target_id",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(BundleRelationship {
                observer_id: bundle_agent_id(&row.get::<_, String>(0)?),
                target_id: row.get::<_, String>(1)?,
                target_type: row.get(2)?,
                relationship_text: row.get(3)?,
                memory_text: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut relationships = Vec::new();
    for row in rows {
        let mut relationship = row.map_err(|e| e.to_string())?;
        let observer_raw = relationship
            .observer_id
            .strip_prefix("agent:")
            .ok_or_else(|| "瀵煎叆鏂囦欢缂哄皯蹇呰瀛楁".to_string())?;
        if !selected_agent_ids.contains(observer_raw) {
            continue;
        }
        let target_included = match relationship.target_type.as_str() {
            TARGET_AGENT => {
                let included = selected_agent_ids.contains(&relationship.target_id);
                relationship.target_id = bundle_agent_id(&relationship.target_id);
                included
            }
            TARGET_USER_PERSONA => {
                let included = selected_user_persona_ids.contains(&relationship.target_id);
                relationship.target_id = bundle_user_persona_id(&relationship.target_id);
                included
            }
            _ => false,
        };
        if target_included {
            relationships.push(relationship);
        }
    }
    Ok(relationships)
}

fn export_friendships(
    conn: &Connection,
    selected_agent_ids: &HashSet<String>,
) -> Result<Vec<BundleFriendship>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT agent_id_1, agent_id_2 FROM friendships WHERE participant_type_2 = 'agent'",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;
    let mut pairs = HashSet::new();
    for row in rows {
        let (a, b) = row.map_err(|e| e.to_string())?;
        if selected_agent_ids.contains(&a) && selected_agent_ids.contains(&b) {
            pairs.insert(unordered_pair(&a, &b));
        }
    }
    let mut friendships: Vec<_> = pairs
        .into_iter()
        .map(|(a, b)| BundleFriendship {
            agent_1_id: bundle_agent_id(&a),
            agent_2_id: bundle_agent_id(&b),
        })
        .collect();
    friendships.sort_by(|a, b| {
        a.agent_1_id
            .cmp(&b.agent_1_id)
            .then(a.agent_2_id.cmp(&b.agent_2_id))
    });
    Ok(friendships)
}

fn parse_and_validate_bundle(json: &str) -> Result<AgentStageBundle, String> {
    let bundle: AgentStageBundle =
        serde_json::from_str(json).map_err(|_| "瀵煎叆鏂囦欢鏍煎紡閿欒".to_string())?;
    if bundle.format != BUNDLE_FORMAT || bundle.version != BUNDLE_VERSION {
        return Err("涓嶆敮鎸佺殑瀵煎叆鏂囦欢鐗堟湰".to_string());
    }

    let mut ids = HashSet::new();
    let mut agent_ids = HashSet::new();
    let mut user_persona_ids = HashSet::new();
    let mut asset_ids = HashSet::new();
    for agent in &bundle.agents {
        validate_prefixed_id(&agent.id, "agent:")?;
        if !ids.insert(agent.id.clone()) {
            return Err("瀵煎叆鏂囦欢鍖呭惈閲嶅 ID".to_string());
        }
        agent_ids.insert(agent.id.clone());
    }
    for persona in &bundle.user_personas {
        validate_prefixed_id(&persona.id, "user_persona:")?;
        if !ids.insert(persona.id.clone()) {
            return Err("瀵煎叆鏂囦欢鍖呭惈閲嶅 ID".to_string());
        }
        user_persona_ids.insert(persona.id.clone());
    }
    for asset in &bundle.assets {
        validate_prefixed_id(&asset.id, "asset:")?;
        if !ids.insert(asset.id.clone()) {
            return Err("瀵煎叆鏂囦欢鍖呭惈閲嶅 ID".to_string());
        }
        asset_ids.insert(asset.id.clone());
    }

    for agent in &bundle.agents {
        validate_optional_asset_ref(agent.avatar_asset_id.as_deref(), &asset_ids)?;
    }
    for persona in &bundle.user_personas {
        validate_optional_asset_ref(persona.avatar_asset_id.as_deref(), &asset_ids)?;
    }
    for relationship in &bundle.relationships {
        if !agent_ids.contains(&relationship.observer_id) {
            return Err("瀵煎叆鏂囦欢缂哄皯蹇呰瀛楁".to_string());
        }
        match relationship.target_type.as_str() {
            TARGET_AGENT => {
                validate_prefixed_id(&relationship.target_id, "agent:")?;
                if !agent_ids.contains(&relationship.target_id) {
                    return Err("瀵煎叆鏂囦欢缂哄皯蹇呰瀛楁".to_string());
                }
            }
            TARGET_USER_PERSONA => {
                validate_prefixed_id(&relationship.target_id, "user_persona:")?;
                if !user_persona_ids.contains(&relationship.target_id) {
                    return Err("瀵煎叆鏂囦欢缂哄皯蹇呰瀛楁".to_string());
                }
            }
            _ => return Err("瀵煎叆鏂囦欢缂哄皯蹇呰瀛楁".to_string()),
        }
    }
    for friendship in &bundle.friendships {
        if !agent_ids.contains(&friendship.agent_1_id)
            || !agent_ids.contains(&friendship.agent_2_id)
        {
            return Err("瀵煎叆鏂囦欢缂哄皯蹇呰瀛楁".to_string());
        }
    }

    Ok(bundle)
}

fn validate_prefixed_id(id: &str, prefix: &str) -> Result<(), String> {
    if id.starts_with(prefix) && id.len() > prefix.len() {
        Ok(())
    } else {
        Err("瀵煎叆鏂囦欢缂哄皯蹇呰瀛楁".to_string())
    }
}

fn validate_optional_asset_ref(
    asset_id: Option<&str>,
    asset_ids: &HashSet<String>,
) -> Result<(), String> {
    if let Some(asset_id) = asset_id {
        validate_prefixed_id(asset_id, "asset:")?;
        if !asset_ids.contains(asset_id) {
            return Err("瀵煎叆鏂囦欢缂哄皯蹇呰瀛楁".to_string());
        }
    }
    Ok(())
}

fn existing_names(conn: &Connection) -> Result<HashSet<String>, String> {
    let mut names = HashSet::new();
    let mut agent_stmt = conn
        .prepare("SELECT name FROM agents WHERE is_deleted = 0")
        .map_err(|e| e.to_string())?;
    let agent_rows = agent_stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    for row in agent_rows {
        names.insert(row.map_err(|e| e.to_string())?.trim().to_string());
    }

    let mut persona_stmt = conn
        .prepare("SELECT name FROM user_personas")
        .map_err(|e| e.to_string())?;
    let persona_rows = persona_stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    for row in persona_rows {
        names.insert(row.map_err(|e| e.to_string())?.trim().to_string());
    }
    Ok(names)
}

fn suggest_unique_import_name(original: &str, used_names: &mut HashSet<String>) -> String {
    if !used_names.contains(original) {
        used_names.insert(original.to_string());
        return original.to_string();
    }

    let mut index = 1;
    loop {
        let candidate = format!("{}_{}", original, index);
        if !used_names.contains(&candidate) {
            used_names.insert(candidate.clone());
            return candidate;
        }
        index += 1;
    }
}

fn avatar_data_url(
    asset_id: Option<&str>,
    asset_map: &HashMap<&str, &BundleAsset>,
    warnings: &mut Vec<String>,
) -> Option<String> {
    let asset_id = asset_id?;
    let asset = match asset_map.get(asset_id) {
        Some(asset) => *asset,
        None => {
            warnings.push(format!(
                "Avatar asset {} does not exist; skipped import preview.",
                asset_id
            ));
            return None;
        }
    };
    let decoded_size = estimated_base64_decoded_size(&asset.base64_content);
    let data_url_size = asset.mime_type.len() + "data:;base64,".len() + asset.base64_content.len();
    if decoded_size > PREVIEW_AVATAR_MAX_DECODED_BYTES
        || data_url_size > PREVIEW_AVATAR_MAX_DATA_URL_BYTES
    {
        warnings.push(format!(
            "Avatar asset {} is too large for import preview; skipped avatarDataUrl.",
            asset_id
        ));
        return None;
    }
    Some(format!(
        "data:{};base64,{}",
        asset.mime_type, asset.base64_content
    ))
}

fn estimated_base64_decoded_size(base64_content: &str) -> usize {
    let trimmed = base64_content.trim_end_matches('=');
    (trimmed.len() * 3) / 4
}

fn write_imported_avatar(
    data_dir: &Path,
    target_type: &str,
    asset_id: Option<&str>,
    asset_map: &HashMap<String, BundleAsset>,
    warnings: &mut Vec<String>,
) -> Option<WrittenAvatar> {
    let asset_id = asset_id?;
    let asset = match asset_map.get(asset_id) {
        Some(asset) => asset,
        None => {
            warnings.push(format!(
                "Avatar asset {} does not exist; skipped avatar write.",
                asset_id
            ));
            return None;
        }
    };
    let bytes = match general_purpose::STANDARD.decode(&asset.base64_content) {
        Ok(bytes) => bytes,
        Err(e) => {
            warnings.push(format!("Avatar asset {} decode failed: {}", asset_id, e));
            return None;
        }
    };
    let ext = extension_for_asset(asset, &bytes);
    let avatar_dir = data_dir.join("avatars").join(target_type);
    if let Err(e) = fs::create_dir_all(&avatar_dir) {
        warnings.push(format!("Avatar directory creation failed: {}", e));
        return None;
    }

    for _ in 0..10 {
        let filename = format!("{}.{}", Uuid::new_v4(), ext);
        let absolute_path = avatar_dir.join(&filename);
        if absolute_path.exists() {
            continue;
        }
        match fs::write(&absolute_path, &bytes) {
            Ok(_) => {
                return Some(WrittenAvatar {
                    relative_path: format!("avatars/{}/{}", target_type, filename),
                    absolute_path,
                })
            }
            Err(e) => {
                warnings.push(format!("Avatar asset {} write failed: {}", asset_id, e));
                return None;
            }
        }
    }
    warnings.push(format!(
        "Avatar asset {} write failed: filename collision.",
        asset_id
    ));
    None
}

fn embed_avatar_asset(
    avatar_path: Option<&str>,
    assets: &mut Vec<BundleAsset>,
    warnings: &mut Vec<String>,
) -> Option<String> {
    let avatar_path = avatar_path?;
    let resolved = resolve_existing_avatar_path(avatar_path);
    let bytes = match fs::read(&resolved) {
        Ok(bytes) => bytes,
        Err(e) => {
            warnings.push(format!(
                "Avatar file {} read failed and was skipped: {}",
                avatar_path, e
            ));
            return None;
        }
    };
    let asset_id = format!("asset:{}", Uuid::new_v4());
    let mime_type = mime_type_for_path(&resolved, &bytes);
    assets.push(BundleAsset {
        id: asset_id.clone(),
        original_path: Some(avatar_path.to_string()),
        mime_type,
        base64_content: general_purpose::STANDARD.encode(bytes),
    });
    Some(asset_id)
}

fn resolve_existing_avatar_path(avatar_path: &str) -> PathBuf {
    let path = PathBuf::from(avatar_path);
    if path.is_absolute() {
        return path;
    }
    if let Ok(data_dir) = crate::get_data_dir() {
        return data_dir.join(path);
    }
    path
}

fn mime_type_for_path(path: &Path, bytes: &[u8]) -> String {
    if bytes.starts_with(b"\x89PNG") {
        "image/png".to_string()
    } else if bytes.starts_with(b"\xff\xd8") {
        "image/jpeg".to_string()
    } else if path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("webp"))
        .unwrap_or(false)
    {
        "image/webp".to_string()
    } else {
        "image/png".to_string()
    }
}

fn extension_for_asset(asset: &BundleAsset, bytes: &[u8]) -> &'static str {
    if asset.mime_type == "image/jpeg" || bytes.starts_with(b"\xff\xd8") {
        "jpg"
    } else if asset.mime_type == "image/webp" {
        "webp"
    } else {
        "png"
    }
}

fn ordered_unique(ids: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    ids.iter()
        .filter(|id| seen.insert((*id).clone()))
        .cloned()
        .collect()
}

fn bundle_agent_id(id: &str) -> String {
    format!("agent:{}", id)
}

fn bundle_user_persona_id(id: &str) -> String {
    format!("user_persona:{}", id)
}

fn unordered_pair(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::BASE_SCHEMA).unwrap();
        conn.execute(
            "INSERT INTO app_settings (id, updated_at) VALUES (1, 0)",
            [],
        )
        .unwrap();
        conn
    }

    fn insert_agent(conn: &Connection, id: &str, name: &str) {
        conn.execute(
            "INSERT INTO agents (id, name, detailed_persona, simplified_persona, long_term_memory, created_at, updated_at) VALUES (?1, ?2, 'detailed', 'simple', 'agent memory', 0, 0)",
            params![id, name],
        )
        .unwrap();
    }

    fn insert_persona(conn: &Connection, id: &str, name: &str) {
        conn.execute(
            "INSERT INTO user_personas (id, name, description, created_at, updated_at) VALUES (?1, ?2, 'persona desc', 0, 0)",
            params![id, name],
        )
        .unwrap();
    }

    fn bundle_json(bundle: &AgentStageBundle) -> String {
        serde_json::to_string(bundle).unwrap()
    }

    fn minimal_bundle() -> AgentStageBundle {
        AgentStageBundle {
            format: BUNDLE_FORMAT.to_string(),
            version: BUNDLE_VERSION,
            exported_at: 0,
            agents: vec![],
            user_personas: vec![],
            relationships: vec![],
            friendships: vec![],
            assets: vec![],
        }
    }

    fn test_bundle_agent(id: &str, name: &str) -> BundleAgent {
        BundleAgent {
            id: id.to_string(),
            name: name.to_string(),
            avatar_asset_id: None,
            detailed_persona: "detailed".to_string(),
            simplified_persona: "simple".to_string(),
            personality: None,
            scenario: None,
            example_messages: None,
            first_message: None,
            creator_notes: None,
            tags: None,
            long_term_memory: None,
            memory_enabled: true,
            proactive_enabled: false,
            proactive_min_minutes: 90,
            proactive_max_minutes: 180,
        }
    }

    #[test]
    fn export_ab_from_abc_omits_c_relationships_and_warns() {
        let conn = init_test_db();
        insert_agent(&conn, "a", "A");
        insert_agent(&conn, "b", "B");
        insert_agent(&conn, "c", "C");
        conn.execute(
            "INSERT INTO agent_relationships (observer_id, target_id, target_type, relationship_text, memory_text, updated_at) VALUES ('a', 'b', 'agent', 'A likes B', 'B memory', 0)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO agent_relationships (observer_id, target_id, target_type, relationship_text, memory_text, updated_at) VALUES ('a', 'c', 'agent', 'A knows C', 'C memory', 0)",
            [],
        ).unwrap();
        crate::db::agent_relationship::add_friendship(&conn, "a", "c").unwrap();

        let agent_ids = vec!["a".to_string(), "b".to_string()];
        let preview = preview_export_bundle(&conn, &agent_ids, &[]).unwrap();
        assert!(preview.requires_confirmation);
        assert_eq!(preview.omitted_relationship_count, 1);
        assert_eq!(preview.omitted_relationship_memory_count, 1);
        assert_eq!(preview.omitted_friendship_count, 1);

        let (bundle, _) = build_bundle(&conn, &agent_ids, &[]).unwrap();
        assert_eq!(bundle.relationships.len(), 1);
        assert_eq!(bundle.relationships[0].observer_id, "agent:a");
        assert_eq!(bundle.relationships[0].target_id, "agent:b");
        assert_eq!(bundle.relationships[0].memory_text, "B memory");
        assert!(bundle.friendships.is_empty());
    }

    #[test]
    fn export_ab_user_persona_includes_agent_to_user_persona_relationship_memory() {
        let conn = init_test_db();
        insert_agent(&conn, "a", "A");
        insert_agent(&conn, "b", "B");
        insert_persona(&conn, "u", "User");
        conn.execute(
            "INSERT INTO agent_relationships (observer_id, target_id, target_type, relationship_text, memory_text, updated_at) VALUES ('a', 'u', 'user_persona', 'A trusts user', 'User likes tea', 0)",
            [],
        ).unwrap();

        let (bundle, _) = build_bundle(
            &conn,
            &["a".to_string(), "b".to_string()],
            &["u".to_string()],
        )
        .unwrap();

        let relationship = bundle
            .relationships
            .iter()
            .find(|r| r.target_type == TARGET_USER_PERSONA)
            .unwrap();
        assert_eq!(relationship.observer_id, "agent:a");
        assert_eq!(relationship.target_id, "user_persona:u");
        assert_eq!(relationship.memory_text, "User likes tea");
    }

    #[test]
    fn import_rebuilds_agent_agent_and_agent_user_persona_relationships_with_new_ids() {
        let conn = init_test_db();
        let mut bundle = minimal_bundle();
        bundle.agents = vec![
            BundleAgent {
                id: "agent:a".to_string(),
                name: "A".to_string(),
                avatar_asset_id: None,
                detailed_persona: "detailed A".to_string(),
                simplified_persona: "simple A".to_string(),
                personality: None,
                scenario: None,
                example_messages: None,
                first_message: None,
                creator_notes: None,
                tags: None,
                long_term_memory: Some("long A".to_string()),
                memory_enabled: true,
                proactive_enabled: false,
                proactive_min_minutes: 90,
                proactive_max_minutes: 180,
            },
            BundleAgent {
                id: "agent:b".to_string(),
                name: "B".to_string(),
                avatar_asset_id: None,
                detailed_persona: "detailed B".to_string(),
                simplified_persona: "simple B".to_string(),
                personality: None,
                scenario: None,
                example_messages: None,
                first_message: None,
                creator_notes: None,
                tags: None,
                long_term_memory: None,
                memory_enabled: true,
                proactive_enabled: false,
                proactive_min_minutes: 90,
                proactive_max_minutes: 180,
            },
        ];
        bundle.user_personas = vec![BundleUserPersona {
            id: "user_persona:u".to_string(),
            name: "U".to_string(),
            description: Some("desc".to_string()),
            avatar_asset_id: None,
        }];
        bundle.relationships = vec![
            BundleRelationship {
                observer_id: "agent:a".to_string(),
                target_id: "agent:b".to_string(),
                target_type: TARGET_AGENT.to_string(),
                relationship_text: "A to B".to_string(),
                memory_text: "B memory".to_string(),
            },
            BundleRelationship {
                observer_id: "agent:a".to_string(),
                target_id: "user_persona:u".to_string(),
                target_type: TARGET_USER_PERSONA.to_string(),
                relationship_text: "A to U".to_string(),
                memory_text: "U memory".to_string(),
            },
        ];

        let result = import_bundle(
            &conn,
            &std::env::temp_dir(),
            &bundle_json(&bundle),
            &[],
            &[],
        )
        .unwrap();
        let new_a = result.agent_id_map.get("agent:a").unwrap();
        let new_b = result.agent_id_map.get("agent:b").unwrap();
        let new_u = result.user_persona_id_map.get("user_persona:u").unwrap();
        assert_ne!(new_a, "a");
        assert_ne!(new_b, "b");
        assert_ne!(new_u, "u");

        let agent_memory: String = conn
            .query_row(
                "SELECT memory_text FROM agent_relationships WHERE observer_id = ?1 AND target_id = ?2 AND target_type = 'agent'",
                params![new_a, new_b],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(agent_memory, "B memory");

        let persona_memory: String = conn
            .query_row(
                "SELECT memory_text FROM agent_relationships WHERE observer_id = ?1 AND target_id = ?2 AND target_type = 'user_persona'",
                params![new_a, new_u],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(persona_memory, "U memory");
    }

    #[test]
    fn import_rebuilds_friendships() {
        let conn = init_test_db();
        let mut bundle = minimal_bundle();
        bundle.agents = vec![
            BundleAgent {
                id: "agent:a".to_string(),
                name: "A".to_string(),
                avatar_asset_id: None,
                detailed_persona: "detailed".to_string(),
                simplified_persona: "simple".to_string(),
                personality: None,
                scenario: None,
                example_messages: None,
                first_message: None,
                creator_notes: None,
                tags: None,
                long_term_memory: None,
                memory_enabled: true,
                proactive_enabled: false,
                proactive_min_minutes: 90,
                proactive_max_minutes: 180,
            },
            BundleAgent {
                id: "agent:b".to_string(),
                name: "B".to_string(),
                avatar_asset_id: None,
                detailed_persona: "detailed".to_string(),
                simplified_persona: "simple".to_string(),
                personality: None,
                scenario: None,
                example_messages: None,
                first_message: None,
                creator_notes: None,
                tags: None,
                long_term_memory: None,
                memory_enabled: true,
                proactive_enabled: false,
                proactive_min_minutes: 90,
                proactive_max_minutes: 180,
            },
        ];
        bundle.friendships = vec![
            BundleFriendship {
                agent_1_id: "agent:a".to_string(),
                agent_2_id: "agent:b".to_string(),
            },
            BundleFriendship {
                agent_1_id: "agent:b".to_string(),
                agent_2_id: "agent:a".to_string(),
            },
        ];

        let result = import_bundle(
            &conn,
            &std::env::temp_dir(),
            &bundle_json(&bundle),
            &[],
            &[],
        )
        .unwrap();
        let new_a = result.agent_id_map.get("agent:a").unwrap();
        let new_b = result.agent_id_map.get("agent:b").unwrap();
        let friendship_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM friendships WHERE participant_type_2 = 'agent' AND ((agent_id_1 = ?1 AND agent_id_2 = ?2) OR (agent_id_1 = ?2 AND agent_id_2 = ?1))",
                params![new_a, new_b],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(friendship_count, 2);
    }

    #[test]
    fn import_auto_renames_against_existing_agent_and_user_persona_names() {
        let conn = init_test_db();
        insert_agent(&conn, "existing-agent", "Same");
        insert_persona(&conn, "existing-persona", "Other");
        let mut bundle = minimal_bundle();
        bundle.agents = vec![BundleAgent {
            id: "agent:a".to_string(),
            name: "Same".to_string(),
            avatar_asset_id: None,
            detailed_persona: "detailed".to_string(),
            simplified_persona: "simple".to_string(),
            personality: None,
            scenario: None,
            example_messages: None,
            first_message: None,
            creator_notes: None,
            tags: None,
            long_term_memory: None,
            memory_enabled: true,
            proactive_enabled: false,
            proactive_min_minutes: 90,
            proactive_max_minutes: 180,
        }];
        bundle.user_personas = vec![BundleUserPersona {
            id: "user_persona:u".to_string(),
            name: "Other".to_string(),
            description: None,
            avatar_asset_id: None,
        }];

        let preview = preview_import_bundle(&conn, &bundle_json(&bundle)).unwrap();
        assert_eq!(preview.agents[0].suggested_name, "Same_1");
        assert_eq!(preview.user_personas[0].suggested_name, "Other_1");
    }

    #[test]
    fn import_rejects_duplicate_final_import_names() {
        let conn = init_test_db();
        let mut bundle = minimal_bundle();
        bundle.agents = vec![
            test_bundle_agent("agent:a", "A"),
            test_bundle_agent("agent:b", "B"),
        ];

        let err = import_bundle_from_bundle(
            &conn,
            &std::env::temp_dir(),
            &bundle,
            &[
                ImportAgentSelection {
                    bundle_id: "agent:a".to_string(),
                    name: "Same Final Name".to_string(),
                    model_config_id: None,
                },
                ImportAgentSelection {
                    bundle_id: "agent:b".to_string(),
                    name: "Same Final Name".to_string(),
                    model_config_id: None,
                },
            ],
            &[],
        )
        .unwrap_err();

        assert!(err.contains("Import name conflicts"));
        let imported_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM agents WHERE name = 'Same Final Name'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(imported_count, 0);
    }

    #[test]
    fn import_rejects_final_agent_name_collision_with_existing_user_persona() {
        let conn = init_test_db();
        insert_persona(&conn, "existing-persona", "Taken");
        let mut bundle = minimal_bundle();
        bundle.agents = vec![test_bundle_agent("agent:a", "A")];

        let err = import_bundle_from_bundle(
            &conn,
            &std::env::temp_dir(),
            &bundle,
            &[ImportAgentSelection {
                bundle_id: "agent:a".to_string(),
                name: "Taken".to_string(),
                model_config_id: None,
            }],
            &[],
        )
        .unwrap_err();

        assert!(err.contains("Import name conflicts"));
        let imported_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM agents WHERE name = 'Taken'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(imported_count, 0);
    }

    #[test]
    fn import_rejects_blank_final_names() {
        let conn = init_test_db();
        let mut bundle = minimal_bundle();
        bundle.user_personas = vec![BundleUserPersona {
            id: "user_persona:u".to_string(),
            name: "User".to_string(),
            description: None,
            avatar_asset_id: None,
        }];

        let err = import_bundle_from_bundle(
            &conn,
            &std::env::temp_dir(),
            &bundle,
            &[],
            &[ImportUserPersonaSelection {
                bundle_id: "user_persona:u".to_string(),
                name: "   ".to_string(),
            }],
        )
        .unwrap_err();

        assert_eq!(err, "Import name cannot be empty.");
        let imported_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM user_personas WHERE name = 'User'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(imported_count, 0);
    }

    #[test]
    fn import_allows_missing_model_selection_and_sets_model_config_id_to_null() {
        let conn = init_test_db();
        let mut bundle = minimal_bundle();
        bundle.agents = vec![BundleAgent {
            id: "agent:a".to_string(),
            name: "A".to_string(),
            avatar_asset_id: None,
            detailed_persona: "detailed".to_string(),
            simplified_persona: "simple".to_string(),
            personality: None,
            scenario: None,
            example_messages: None,
            first_message: None,
            creator_notes: None,
            tags: None,
            long_term_memory: None,
            memory_enabled: true,
            proactive_enabled: false,
            proactive_min_minutes: 90,
            proactive_max_minutes: 180,
        }];

        let result = import_bundle(
            &conn,
            &std::env::temp_dir(),
            &bundle_json(&bundle),
            &[],
            &[],
        )
        .unwrap();
        let new_a = result.agent_id_map.get("agent:a").unwrap();
        let (model_config_id, agent_temperature): (Option<String>, Option<f64>) = conn
            .query_row(
                "SELECT model_config_id, agent_temperature FROM agents WHERE id = ?1",
                [new_a],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert!(model_config_id.is_none());
        assert!(agent_temperature.is_none());
    }

    #[test]
    fn import_does_not_activate_imported_user_persona() {
        let conn = init_test_db();
        insert_persona(&conn, "active", "Active");
        conn.execute(
            "UPDATE app_settings SET active_persona_id = 'active' WHERE id = 1",
            [],
        )
        .unwrap();
        let mut bundle = minimal_bundle();
        bundle.user_personas = vec![BundleUserPersona {
            id: "user_persona:u".to_string(),
            name: "Imported User".to_string(),
            description: None,
            avatar_asset_id: None,
        }];

        import_bundle(
            &conn,
            &std::env::temp_dir(),
            &bundle_json(&bundle),
            &[],
            &[],
        )
        .unwrap();
        let active_id: Option<String> = conn
            .query_row(
                "SELECT active_persona_id FROM app_settings WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(active_id, Some("active".to_string()));
    }

    #[test]
    fn import_rejects_duplicate_bundle_local_ids() {
        let conn = init_test_db();
        let mut bundle = minimal_bundle();
        bundle.agents = vec![BundleAgent {
            id: "agent:dup".to_string(),
            name: "A".to_string(),
            avatar_asset_id: None,
            detailed_persona: "detailed".to_string(),
            simplified_persona: "simple".to_string(),
            personality: None,
            scenario: None,
            example_messages: None,
            first_message: None,
            creator_notes: None,
            tags: None,
            long_term_memory: None,
            memory_enabled: true,
            proactive_enabled: false,
            proactive_min_minutes: 90,
            proactive_max_minutes: 180,
        }];
        bundle.assets = vec![
            BundleAsset {
                id: "asset:dup".to_string(),
                original_path: None,
                mime_type: "image/png".to_string(),
                base64_content: "".to_string(),
            },
            BundleAsset {
                id: "asset:dup".to_string(),
                original_path: None,
                mime_type: "image/png".to_string(),
                base64_content: "".to_string(),
            },
        ];

        let err = preview_import_bundle(&conn, &bundle_json(&bundle)).unwrap_err();
        assert_eq!(err, "瀵煎叆鏂囦欢鍖呭惈閲嶅 ID");
    }

    #[test]
    fn import_preview_omits_large_avatar_data_url_and_warns() {
        let conn = init_test_db();
        let mut bundle = minimal_bundle();
        bundle.assets = vec![BundleAsset {
            id: "asset:large-avatar".to_string(),
            original_path: None,
            mime_type: "image/png".to_string(),
            base64_content: general_purpose::STANDARD.encode(vec![
                1u8;
                PREVIEW_AVATAR_MAX_DECODED_BYTES
                    + 1
            ]),
        }];
        bundle.agents = vec![BundleAgent {
            id: "agent:a".to_string(),
            name: "A".to_string(),
            avatar_asset_id: Some("asset:large-avatar".to_string()),
            detailed_persona: "detailed".to_string(),
            simplified_persona: "simple".to_string(),
            personality: None,
            scenario: None,
            example_messages: None,
            first_message: None,
            creator_notes: None,
            tags: None,
            long_term_memory: None,
            memory_enabled: true,
            proactive_enabled: false,
            proactive_min_minutes: 90,
            proactive_max_minutes: 180,
        }];

        let preview = preview_import_bundle(&conn, &bundle_json(&bundle)).unwrap();

        assert!(preview.agents[0].avatar_data_url.is_none());
        assert!(preview
            .warnings
            .iter()
            .any(|warning| warning.contains("asset:large-avatar")));
    }

    #[test]
    fn import_rolls_back_db_and_cleans_avatars_on_late_failure() {
        let conn = init_test_db();
        let data_dir =
            std::env::temp_dir().join(format!("agentstage-bundle-test-{}", Uuid::new_v4()));
        let mut bundle = minimal_bundle();
        bundle.assets = vec![BundleAsset {
            id: "asset:avatar".to_string(),
            original_path: None,
            mime_type: "image/png".to_string(),
            base64_content: general_purpose::STANDARD.encode(b"\x89PNG\r\n\x1a\n"),
        }];
        let mut agent = test_bundle_agent("agent:a", "Rollback Agent");
        agent.avatar_asset_id = Some("asset:avatar".to_string());
        bundle.agents = vec![agent, test_bundle_agent("agent:b", "Rollback Friend")];
        bundle.user_personas = vec![BundleUserPersona {
            id: "user_persona:u".to_string(),
            name: "Rollback Persona".to_string(),
            description: None,
            avatar_asset_id: None,
        }];
        bundle.relationships = vec![
            BundleRelationship {
                observer_id: "agent:a".to_string(),
                target_id: "agent:b".to_string(),
                target_type: TARGET_AGENT.to_string(),
                relationship_text: "first".to_string(),
                memory_text: "first memory".to_string(),
            },
            BundleRelationship {
                observer_id: "agent:a".to_string(),
                target_id: "agent:b".to_string(),
                target_type: TARGET_AGENT.to_string(),
                relationship_text: "duplicate".to_string(),
                memory_text: "duplicate memory".to_string(),
            },
        ];

        let result = import_bundle(&conn, &data_dir, &bundle_json(&bundle), &[], &[]);

        assert!(result.is_err());
        let agent_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM agents WHERE name IN ('Rollback Agent', 'Rollback Friend')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(agent_count, 0);
        let persona_count: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM user_personas WHERE name = 'Rollback Persona'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(persona_count, 0);
        let avatar_dir = data_dir.join("avatars").join(TARGET_AGENT);
        let written_avatar_count = fs::read_dir(&avatar_dir)
            .map(|entries| entries.count())
            .unwrap_or(0);
        assert_eq!(written_avatar_count, 0);
        let _ = fs::remove_dir_all(&data_dir);
    }
}
