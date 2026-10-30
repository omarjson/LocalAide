use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolActionType {
    ListDir,
    ReadFile,
    WriteFile,
    SearchFiles,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolAction {
    pub action_type: ToolActionType,
    pub path: String,
    pub content: Option<String>,
    pub query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionRequest {
    pub tool_name: String,
    pub action: ToolAction,
    pub approved: Option<bool>,
    pub session_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionResponse {
    pub success: bool,
    pub output: String,
    pub truncated: bool,
    pub requires_approval: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolAuditEntry {
    pub id: i64,
    pub timestamp: i64,
    pub tool_name: String,
    pub action: String,
    pub path: String,
    pub approved: bool,
    pub status: String,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size_bytes: u64,
    pub modified_at: u64,
}
