use std::collections::HashSet;
use std::sync::Mutex;
use crate::domain::tool::{ToolAction, ToolActionType, ToolExecutionRequest};

#[derive(Debug)]
pub struct ApprovalManager {
    session_tokens: Mutex<HashSet<String>>,
}

impl Default for ApprovalManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ApprovalManager {
    pub fn new() -> Self {
        Self {
            session_tokens: Mutex::new(HashSet::new()),
        }
    }

    /// Read-only operations (ListDir, ReadFile, SearchFiles) are non-destructive and auto-approved.
    /// Modifying operations (WriteFile) strictly require explicit user approval.
    pub fn requires_user_approval(&self, action: &ToolAction) -> bool {
        matches!(action.action_type, ToolActionType::WriteFile)
    }

    pub fn is_approved(&self, req: &ToolExecutionRequest) -> bool {
        if !self.requires_user_approval(&req.action) {
            return true;
        }

        // Check if single-invocation approval is granted
        if req.approved == Some(true) {
            // If session token was provided, remember it for the session
            if let Some(token) = &req.session_token {
                if let Ok(mut tokens) = self.session_tokens.lock() {
                    tokens.insert(token.clone());
                }
            }
            return true;
        }

        // Check if session approval was previously granted
        if let Some(token) = &req.session_token {
            if let Ok(tokens) = self.session_tokens.lock() {
                if tokens.contains(token) {
                    return true;
                }
            }
        }

        false
    }

    pub fn revoke_session(&self, token: &str) {
        if let Ok(mut tokens) = self.session_tokens.lock() {
            tokens.remove(token);
        }
    }

    pub fn clear_all_sessions(&self) {
        if let Ok(mut tokens) = self.session_tokens.lock() {
            tokens.clear();
        }
    }
}
