use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfigDto {
    /// Models the user downloaded or loaded. Empty means first run.
    pub selected_models: Vec<String>,
}
