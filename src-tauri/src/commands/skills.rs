use crate::app::error::AppError;
use crate::app::state::AppState;
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
