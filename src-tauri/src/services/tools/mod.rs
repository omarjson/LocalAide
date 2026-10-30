pub mod approval;
pub mod sandbox;

use std::sync::Arc;
use crate::app::error::AppError;
use crate::domain::tool::{
    ToolActionType, ToolAuditEntry, ToolExecutionRequest, ToolExecutionResponse,
};
use crate::services::library::db::LibraryDb;
use approval::ApprovalManager;
use sandbox::WorkspaceJail;

pub struct ToolService {
    jail: Arc<WorkspaceJail>,
    db: Arc<LibraryDb>,
    approval: Arc<ApprovalManager>,
}

impl ToolService {
    pub fn new(jail: Arc<WorkspaceJail>, db: Arc<LibraryDb>) -> Self {
        Self {
            jail,
            db,
            approval: Arc::new(ApprovalManager::new()),
        }
    }

    pub fn jail(&self) -> &WorkspaceJail {
        &self.jail
    }

    pub fn approval(&self) -> &ApprovalManager {
        &self.approval
    }

    pub fn execute(&self, req: ToolExecutionRequest) -> Result<ToolExecutionResponse, AppError> {
        let action_name = match req.action.action_type {
            ToolActionType::ListDir => "list_dir",
            ToolActionType::ReadFile => "read_file",
            ToolActionType::WriteFile => "write_file",
            ToolActionType::SearchFiles => "search_files",
        };

        // Explicit denial
        if req.approved == Some(false) {
            let _ = self.db.record_tool_audit(
                &req.tool_name,
                action_name,
                &req.action.path,
                false,
                "denied",
                Some("User explicitly denied execution"),
            );
            return Ok(ToolExecutionResponse {
                success: false,
                output: String::new(),
                truncated: false,
                requires_approval: false,
                error: Some("Tool execution denied by user".to_string()),
            });
        }

        // Approval check
        if !self.approval.is_approved(&req) {
            return Ok(ToolExecutionResponse {
                success: false,
                output: String::new(),
                truncated: false,
                requires_approval: true,
                error: Some("Operation requires user approval".to_string()),
            });
        }

        // Execute inside jail
        let exec_result: Result<(String, bool), AppError> = match req.action.action_type {
            ToolActionType::ListDir => {
                let entries = self.jail.list_dir(Some(&req.action.path))?;
                let mut out = format!("Found {} entries:\n", entries.len());
                for e in entries {
                    let kind = if e.is_dir { "DIR " } else { "FILE" };
                    out.push_str(&format!("  [{kind}] {:<30} ({} bytes)\n", e.path, e.size_bytes));
                }
                Ok((out, false))
            }
            ToolActionType::ReadFile => {
                let (content, truncated) = self.jail.read_file(&req.action.path)?;
                Ok((content, truncated))
            }
            ToolActionType::WriteFile => {
                let content = req.action.content.as_deref().unwrap_or("");
                let bytes = self.jail.write_file(&req.action.path, content)?;
                Ok((format!("Successfully wrote {bytes} bytes to {}", req.action.path), false))
            }
            ToolActionType::SearchFiles => {
                let query = req.action.query.as_deref().unwrap_or("");
                let matches = self.jail.search_files(query, Some(&req.action.path))?;
                let mut out = format!("Search for '{query}' returned {} matches:\n", matches.len());
                for m in matches {
                    out.push_str(&format!("  - {m}\n"));
                }
                Ok((out, false))
            }
        };

        match exec_result {
            Ok((output, truncated)) => {
                let _ = self.db.record_tool_audit(
                    &req.tool_name,
                    action_name,
                    &req.action.path,
                    true,
                    "success",
                    None,
                );
                Ok(ToolExecutionResponse {
                    success: true,
                    output,
                    truncated,
                    requires_approval: false,
                    error: None,
                })
            }
            Err(err) => {
                let err_msg = err.to_string();
                let _ = self.db.record_tool_audit(
                    &req.tool_name,
                    action_name,
                    &req.action.path,
                    true,
                    "error",
                    Some(&err_msg),
                );
                Ok(ToolExecutionResponse {
                    success: false,
                    output: String::new(),
                    truncated: false,
                    requires_approval: false,
                    error: Some(err_msg),
                })
            }
        }
    }

    pub fn list_audits(&self, limit: usize) -> Result<Vec<ToolAuditEntry>, AppError> {
        let db_audits = self.db.list_tool_audits(limit)?;
        let entries = db_audits
            .into_iter()
            .map(|a| ToolAuditEntry {
                id: a.id,
                timestamp: a.timestamp,
                tool_name: a.tool_name,
                action: a.action,
                path: a.path,
                approved: a.approved,
                status: a.status,
                details: a.details,
            })
            .collect();
        Ok(entries)
    }

    pub fn clear_audits(&self) -> Result<(), AppError> {
        self.db.clear_tool_audits()?;
        Ok(())
    }
}
