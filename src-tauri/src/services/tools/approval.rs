use crate::domain::tool::{ToolAction, ToolActionType, ToolExecutionRequest};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long "Allow for this folder" lasts.
pub const SESSION_GRANT_TTL: Duration = Duration::from_secs(30 * 60);

/// What one "Allow for this folder" approval covers: writes in one folder of
/// the workspace, until a time.
#[derive(Debug, Clone)]
struct SessionGrant {
    folder: String,
    expires_at: Instant,
}

#[derive(Debug)]
pub struct ApprovalManager {
    session_grants: Mutex<HashMap<String, SessionGrant>>,
}

impl Default for ApprovalManager {
    fn default() -> Self {
        Self::new()
    }
}

/// The folder part of a workspace path, written the same way on every system.
fn folder_of(path: &str) -> String {
    Path::new(&path.replace('\\', "/"))
        .parent()
        .map(|p| p.to_string_lossy().trim_start_matches("./").to_string())
        .unwrap_or_default()
}

impl ApprovalManager {
    pub fn new() -> Self {
        Self {
            session_grants: Mutex::new(HashMap::new()),
        }
    }

    /// Read-only operations (ListDir, ReadFile, SearchFiles) are non-destructive and auto-approved.
    /// Modifying operations (WriteFile) strictly require explicit user approval.
    pub fn requires_user_approval(&self, action: &ToolAction) -> bool {
        matches!(action.action_type, ToolActionType::WriteFile)
    }

    pub fn is_approved(&self, req: &ToolExecutionRequest) -> bool {
        self.is_approved_at(req, Instant::now())
    }

    /// A write is approved when the user approved this one, or when an
    /// earlier "Allow for this folder" with the same token covers the same
    /// folder and has not expired.
    fn is_approved_at(&self, req: &ToolExecutionRequest, now: Instant) -> bool {
        if !self.requires_user_approval(&req.action) {
            return true;
        }
        let mut grants = self
            .session_grants
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        grants.retain(|_, grant| grant.expires_at > now);

        if req.approved == Some(true) {
            if let Some(token) = &req.session_token {
                grants.insert(
                    token.clone(),
                    SessionGrant {
                        folder: folder_of(&req.action.path),
                        expires_at: now + SESSION_GRANT_TTL,
                    },
                );
            }
            return true;
        }

        req.session_token
            .as_ref()
            .and_then(|token| grants.get(token))
            .is_some_and(|grant| grant.folder == folder_of(&req.action.path))
    }
}

#[cfg(test)]
mod tests {
    use super::{ApprovalManager, SESSION_GRANT_TTL};
    use crate::domain::tool::{ToolAction, ToolActionType, ToolExecutionRequest};
    use std::time::{Duration, Instant};

    fn write(path: &str, approved: Option<bool>) -> ToolExecutionRequest {
        ToolExecutionRequest {
            tool_name: "fs_tool".to_string(),
            action: ToolAction {
                action_type: ToolActionType::WriteFile,
                path: path.to_string(),
                content: Some("text".to_string()),
                query: None,
            },
            approved,
            session_token: Some("token-1".to_string()),
        }
    }

    #[test]
    fn a_folder_approval_covers_that_folder_only_and_expires() {
        let approvals = ApprovalManager::new();
        let start = Instant::now();
        assert!(!approvals.is_approved_at(&write("notes/a.txt", None), start));

        // "Allow for this folder" on one write.
        assert!(approvals.is_approved_at(&write("notes/a.txt", Some(true)), start));
        assert!(approvals.is_approved_at(&write("notes/b.txt", None), start));
        assert!(approvals.is_approved_at(&write("notes\\c.txt", None), start));
        assert!(!approvals.is_approved_at(&write("other/c.txt", None), start));
        assert!(!approvals.is_approved_at(&write("notes/deeper/d.txt", None), start));

        let later = start + SESSION_GRANT_TTL + Duration::from_secs(1);
        assert!(!approvals.is_approved_at(&write("notes/b.txt", None), later));
    }

    #[test]
    fn reads_need_no_approval() {
        let approvals = ApprovalManager::new();
        let mut read = write("notes/a.txt", None);
        read.action.action_type = ToolActionType::ReadFile;
        assert!(approvals.is_approved(&read));
    }
}
