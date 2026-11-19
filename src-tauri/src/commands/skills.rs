use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::services::skills::discover::{
    RemoteSkill, RemoteSkillDetail, SOURCES, SkillDescription, SkillSource,
};
use crate::services::skills::{Skill, SkillSuggestion};
use tauri::State;

#[tauri::command]
pub fn skills_list(state: State<'_, AppState>) -> Result<Vec<Skill>, AppError> {
    state.skills.list()
}

#[tauri::command]
pub fn skills_read(state: State<'_, AppState>, path: String) -> Result<Skill, AppError> {
    state.skills.read(&path)
}

#[tauri::command]
pub fn skills_create(
    state: State<'_, AppState>,
    name: String,
    description: String,
    tags: Option<Vec<String>>,
    body: String,
) -> Result<Skill, AppError> {
    state
        .skills
        .create(&name, &description, tags.unwrap_or_default(), &body)
}

#[tauri::command]
pub fn skills_update(
    state: State<'_, AppState>,
    path: String,
    name: String,
    description: String,
    tags: Option<Vec<String>>,
    body: String,
) -> Result<Skill, AppError> {
    state
        .skills
        .update(&path, &name, &description, tags.unwrap_or_default(), &body)
}

#[tauri::command]
pub fn skills_delete(state: State<'_, AppState>, path: String) -> Result<(), AppError> {
    state.skills.delete(&path)
}

/// Prompts used repeatedly but never saved — the save-as-skill hints.
#[tauri::command]
pub fn skills_suggestions(state: State<'_, AppState>) -> Result<Vec<SkillSuggestion>, AppError> {
    state.skills.suggestions(3)
}

/// The public skill collections the Skills screen can browse.
#[tauri::command]
pub fn skills_sources() -> Vec<SkillSource> {
    SOURCES.to_vec()
}

/// List one collection. `source` is a collection id, or "github" with
/// `repo` set to `owner/name` (optionally followed by the skills folder).
#[tauri::command]
pub async fn skills_discover(
    state: State<'_, AppState>,
    source: String,
    repo: Option<String>,
) -> Result<Vec<RemoteSkill>, AppError> {
    state.skill_discovery.list(&source, repo.as_deref()).await
}

/// The full text of one listed skill, for the preview.
#[tauri::command]
pub async fn skills_remote_detail(
    state: State<'_, AppState>,
    source: String,
    repo: Option<String>,
    id: String,
) -> Result<RemoteSkillDetail, AppError> {
    state
        .skill_discovery
        .detail(&source, repo.as_deref(), &id)
        .await
}

/// Save a listed skill as a local skill. It remembers its source address and
/// license, and is tagged "imported".
#[tauri::command]
pub async fn skills_import(
    state: State<'_, AppState>,
    source: String,
    repo: Option<String>,
    id: String,
) -> Result<Skill, AppError> {
    let detail = state
        .skill_discovery
        .detail(&source, repo.as_deref(), &id)
        .await?;
    if detail.body.trim().is_empty() {
        return Err(AppError::invalid_input("That skill has no text to import."));
    }
    let origin = match source.as_str() {
        "anthropic" => "anthropic",
        "copilot" => "copilot",
        "prompts-chat" => "prompts-chat",
        _ => "github",
    };
    state.skills.create_with_origin(
        &detail.name,
        &detail.description,
        vec!["imported".to_string(), origin.to_string()],
        &detail.body,
        Some(detail.url),
        detail.license,
    )
}

/// Descriptions for listed skills (one page at a time), so the Discover
/// list says what each skill does.
#[tauri::command]
pub async fn skills_remote_describe(
    state: State<'_, AppState>,
    source: String,
    repo: Option<String>,
    ids: Vec<String>,
) -> Result<Vec<SkillDescription>, AppError> {
    state
        .skill_discovery
        .describe(&source, repo.as_deref(), &ids)
        .await
}
