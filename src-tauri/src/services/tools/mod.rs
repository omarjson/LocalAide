pub mod approval;
pub mod sandbox;

use crate::app::error::AppError;
use crate::domain::tool::{
    ToolActionType, ToolAuditEntry, ToolExecutionRequest, ToolExecutionResponse,
};
use crate::services::library::db::LibraryDb;
use approval::ApprovalManager;
use sandbox::WorkspaceJail;
use std::sync::Arc;

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

    pub fn execute(&self, req: ToolExecutionRequest) -> Result<ToolExecutionResponse, AppError> {
        let action_name = match req.action.action_type {
            ToolActionType::ListDir => "list_dir",
            ToolActionType::ReadFile => "read_file",
            ToolActionType::WriteFile => "write_file",
            ToolActionType::SearchFiles => "search_files",
        };

        // Explicit denial
        if req.approved == Some(false) {
            if let Err(e) = self.db.record_tool_audit(
                &req.tool_name,
                action_name,
                &req.action.path,
                false,
                "denied",
                Some("User explicitly denied execution"),
            ) {
                log::warn!("Could not record a denied tool action: {e}");
            }
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

        // A write changes a file, so its audit row comes first. If the row
        // cannot be written, the file is not touched.
        let write_audit = if matches!(req.action.action_type, ToolActionType::WriteFile) {
            let id = self
                .db
                .record_tool_audit(
                    &req.tool_name,
                    action_name,
                    &req.action.path,
                    true,
                    "started",
                    None,
                )
                .map_err(|e| {
                    AppError::storage(format!(
                        "The activity log could not record this change, so the file was not written: {}",
                        e.message
                    ))
                })?;
            Some(id)
        } else {
            None
        };

        // Errors are kept, not returned early, so a refused or failed action
        // is still recorded below.
        let exec_result = self.run_in_jail(&req);

        let (status, details) = match &exec_result {
            Ok(_) => ("success", None),
            Err(err) => ("error", Some(err.to_string())),
        };
        let recorded = match write_audit {
            Some(id) => self.db.finish_tool_audit(id, status, details.as_deref()),
            None => self
                .db
                .record_tool_audit(
                    &req.tool_name,
                    action_name,
                    &req.action.path,
                    true,
                    status,
                    details.as_deref(),
                )
                .map(|_| ()),
        };
        if let Err(e) = recorded {
            log::warn!("Could not record the outcome of a tool action: {e}");
        }

        match exec_result {
            Ok((output, truncated)) => Ok(ToolExecutionResponse {
                success: true,
                output,
                truncated,
                requires_approval: false,
                error: None,
            }),
            Err(err) => Ok(ToolExecutionResponse {
                success: false,
                output: String::new(),
                truncated: false,
                requires_approval: false,
                error: Some(err.message),
            }),
        }
    }

    /// Runs one approved action inside the workspace jail.
    fn run_in_jail(&self, req: &ToolExecutionRequest) -> Result<(String, bool), AppError> {
        match req.action.action_type {
            ToolActionType::ListDir => {
                let entries = self.jail.list_dir(Some(&req.action.path))?;
                let mut out = format!("Found {} entries:\n", entries.len());
                for e in entries {
                    let kind = if e.is_dir { "DIR " } else { "FILE" };
                    out.push_str(&format!(
                        "  [{kind}] {:<30} ({} bytes)\n",
                        e.path, e.size_bytes
                    ));
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
                Ok((
                    format!("Successfully wrote {bytes} bytes to {}", req.action.path),
                    false,
                ))
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

    /// Clears the activity log, then records that it was cleared, so a gap in
    /// the log is always explained.
    pub fn clear_audits(&self) -> Result<(), AppError> {
        let removed = self.db.clear_tool_audits()?;
        self.db.record_tool_audit(
            "activity_log",
            "clear",
            "",
            true,
            "cleared",
            Some(&format!("{removed} entries removed")),
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ToolService;
    use super::sandbox::WorkspaceJail;
    use crate::domain::tool::{ToolAction, ToolActionType, ToolExecutionRequest};
    use crate::services::library::db::LibraryDb;
    use std::sync::Arc;

    fn service(name: &str) -> (ToolService, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("la-tools-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let jail = Arc::new(WorkspaceJail::new(&dir).unwrap());
        let db = Arc::new(LibraryDb::open_in_memory().unwrap());
        (ToolService::new(jail, db), dir)
    }

    fn read(path: &str) -> ToolExecutionRequest {
        ToolExecutionRequest {
            tool_name: "fs_tool".to_string(),
            action: ToolAction {
                action_type: ToolActionType::ReadFile,
                path: path.to_string(),
                content: None,
                query: None,
            },
            approved: Some(true),
            session_token: None,
        }
    }

    #[test]
    fn a_refused_action_is_still_recorded() {
        let (tools, dir) = service("refused");
        let response = tools.execute(read("../outside.txt")).unwrap();
        assert!(!response.success);

        let audits = tools.list_audits(10).unwrap();
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].status, "error");
        assert_eq!(audits[0].path, "../outside.txt");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn clearing_the_log_leaves_a_note() {
        let (tools, dir) = service("clear");
        std::fs::write(dir.join("a.txt"), "hello").unwrap();
        tools.execute(read("a.txt")).unwrap();
        tools.clear_audits().unwrap();

        let audits = tools.list_audits(10).unwrap();
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].status, "cleared");
        assert_eq!(audits[0].details.as_deref(), Some("1 entries removed"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
