use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfigDto {
    /// Models the user downloaded or loaded. Empty means first run.
    pub selected_models: Vec<String>,
    /// True once the setup wizard has finished or been skipped. The startup
    /// gate uses this (plus the two fields below) instead of
    /// `selected_models` alone, so an online-only user does not see setup
    /// on every launch.
    pub setup_complete: bool,
    /// True when at least one online provider is saved.
    pub has_online_providers: bool,
}
