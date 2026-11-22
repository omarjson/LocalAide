//! Settings-free scheduled prompts: the Automations screen.

use serde::Serialize;
use tauri::State;

use crate::app::error::AppError;
use crate::app::state::AppState;
use crate::services::automations::{Automation, AutomationRun};

/// What the Automations screen renders in one load.
#[derive(Debug, Serialize)]
pub struct AutomationsState {
    pub automations: Vec<Automation>,
    pub runs: Vec<AutomationRun>,
}

fn state_of(state: &State<'_, AppState>) -> AutomationsState {
    let (automations, runs) = state.automations.list();
    AutomationsState { automations, runs }
}

#[tauri::command]
pub fn automations_list(state: State<'_, AppState>) -> AutomationsState {
    state_of(&state)
}

/// Create or update an automation, off the window's thread.
#[tauri::command]
pub async fn automations_save(
    state: State<'_, AppState>,
    automation: Automation,
) -> Result<AutomationsState, AppError> {
    let owned: AppState = (*state).clone();
    crate::app::blocking::run_blocking(move || {
        owned.automations.save(automation)?;
        let (automations, runs) = owned.automations.list();
        Ok(AutomationsState { automations, runs })
    })
    .await
}

#[tauri::command]
pub async fn automations_delete(
    state: State<'_, AppState>,
    id: String,
) -> Result<AutomationsState, AppError> {
    let owned: AppState = (*state).clone();
    crate::app::blocking::run_blocking(move || {
        owned.automations.delete(&id)?;
        let (automations, runs) = owned.automations.list();
        Ok(AutomationsState { automations, runs })
    })
    .await
}

/// Run one automation right now. The answer lands in the feed when the
/// model finishes; the screen hears about it through `automations-changed`.
#[tauri::command]
pub fn automations_run_now(
    state: State<'_, AppState>,
    handle: tauri::AppHandle,
    id: String,
) -> bool {
    state.automations.run_now(&state, handle, &id)
}
