//! Settings → Plugins: list the HTML plugins on this computer. Listing only
//! reads directory entries and metadata; no plugin code is run in Rust.

use crate::app::error::AppError;
use crate::services::plugins::{self, Plugin};

/// The plugins available to the UI, newest call sees the current folder.
#[tauri::command]
pub fn plugin_list() -> Result<Vec<Plugin>, AppError> {
    plugins::list()
}
