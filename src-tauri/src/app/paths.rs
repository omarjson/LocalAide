use std::path::PathBuf;

pub fn app_dir() -> PathBuf {
    dirs_next::home_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(".aiassistant")
}

pub fn models_dir() -> PathBuf {
    app_dir().join("models")
}

pub fn index_dir() -> PathBuf {
    app_dir().join("index")
}

pub fn memory_dir() -> PathBuf {
    app_dir().join("memory")
}

pub fn workspace_dir() -> PathBuf {
    app_dir().join("workspace")
}

pub fn conversations_dir() -> PathBuf {
    app_dir().join("conversations")
}

pub fn config_path() -> PathBuf {
    app_dir().join("config.json")
}
