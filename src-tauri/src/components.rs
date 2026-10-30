use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentManifest {
    pub name: String,
    pub version: String,
    pub url: String,
    pub sha256: String,
    pub platforms: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentUpdate {
    pub current_version: Option<String>,
    pub latest_version: String,
    pub manifest: ComponentManifest,
}

pub fn components_dir() -> PathBuf {
    let mut dir = dirs_next::home_dir().unwrap_or_default();
    dir.push(".aiassistant");
    dir.push("components");
    dir
}
