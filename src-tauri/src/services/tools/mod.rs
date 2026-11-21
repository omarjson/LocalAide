pub mod approval;
pub mod sandbox;
pub mod shell;

/// A finished terminal command and where it ran.
pub struct CommandRun {
    pub output: shell::RunOutput,
    /// The folder inside the workspace, empty for the top.
    pub cwd: String,
    pub workspace: std::path::PathBuf,
}

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

    /// The shared workspace folder's jail.
    pub fn workspace_jail(&self) -> Arc<WorkspaceJail> {
        self.jail.clone()
    }

    pub fn execute(&self, req: ToolExecutionRequest) -> Result<ToolExecutionResponse, AppError> {
        let jail = self.jail.clone();
        self.execute_in(&jail, req)
    }

    /// Run a file action inside `jail`: the workspace, or the folder a chat
    /// works in.
    pub fn execute_in(
        &self,
        jail: &WorkspaceJail,
        req: ToolExecutionRequest,
    ) -> Result<ToolExecutionResponse, AppError> {
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
        let exec_result = Self::run_in_jail(jail, &req);

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
    fn run_in_jail(
        jail: &WorkspaceJail,
        req: &ToolExecutionRequest,
    ) -> Result<(String, bool), AppError> {
        match req.action.action_type {
            ToolActionType::ListDir => {
                let entries = jail.list_dir(Some(&req.action.path))?;
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
                let (content, truncated) = jail.read_file(&req.action.path)?;
                Ok((content, truncated))
            }
            ToolActionType::WriteFile => {
                let content = req.action.content.as_deref().unwrap_or("");
                let bytes = jail.write_file(&req.action.path, content)?;
                Ok((
                    format!("Successfully wrote {bytes} bytes to {}", req.action.path),
                    false,
                ))
            }
            ToolActionType::SearchFiles => {
                let query = req.action.query.as_deref().unwrap_or("");
                let matches = jail.search_files(query, Some(&req.action.path))?;
                let mut out = format!("Search for '{query}' returned {} matches:\n", matches.len());
                for m in matches {
                    out.push_str(&format!("  - {m}\n"));
                }
                Ok((out, false))
            }
        }
    }

    /// Record a chat tool that is not a workspace action (web search, notes,
    /// MCP), so the activity log shows everything the model did.
    pub fn record_event(
        &self,
        tool_name: &str,
        action: &str,
        path: &str,
        status: &str,
        details: Option<&str>,
    ) {
        if let Err(e) = self
            .db
            .record_tool_audit(tool_name, action, path, true, status, details)
        {
            log::warn!("Could not record a chat tool action: {e}");
        }
    }

    /// Run a terminal command in the workspace (or a folder inside it).
    /// The audit row is written before the command starts, so a run that
    /// crashes the app is still on record, and finished afterwards.
    /// `source` names who asked, for example "chat" or "share".
    pub fn run_command(
        &self,
        source: &str,
        command: &str,
        cwd: Option<&str>,
        timeout: std::time::Duration,
        cancel: &std::sync::atomic::AtomicBool,
        note: Option<&str>,
    ) -> Result<CommandRun, AppError> {
        let jail = self.jail.clone();
        self.run_command_in(&jail, source, command, cwd, timeout, cancel, note)
    }

    /// Run a terminal command in `jail`'s folder (or a folder inside it).
    #[allow(clippy::too_many_arguments)]
    pub fn run_command_in(
        &self,
        jail: &WorkspaceJail,
        source: &str,
        command: &str,
        cwd: Option<&str>,
        timeout: std::time::Duration,
        cancel: &std::sync::atomic::AtomicBool,
        note: Option<&str>,
    ) -> Result<CommandRun, AppError> {
        if let shell::Verdict::Blocked(reason) = shell::classify(command) {
            self.record_event(
                &format!("{source}:execute_command"),
                "shell",
                command,
                "blocked",
                Some(&reason),
            );
            return Err(AppError::new("COMMAND_BLOCKED", reason));
        }
        let folder = match cwd.map(str::trim).filter(|c| !c.is_empty() && *c != ".") {
            Some(dir) => {
                let path = jail.resolve_path(dir)?;
                if !path.is_dir() {
                    return Err(AppError::invalid_input(format!(
                        "There is no folder called {dir} in the workspace."
                    )));
                }
                path
            }
            None => jail.root_path().to_path_buf(),
        };
        let shown_cwd = folder
            .strip_prefix(jail.root_path())
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let audit_id = self.db.record_tool_audit(
            &format!("{source}:execute_command"),
            "shell",
            command,
            true,
            "started",
            Some(&format!(
                "in workspace/{shown_cwd}{}",
                note.map(|n| format!(" · {n}")).unwrap_or_default()
            )),
        )?;
        let output = match shell::run(command, &folder, timeout, cancel) {
            Ok(o) => o,
            Err(message) => {
                let _ = self.db.finish_tool_audit(audit_id, "error", Some(&message));
                return Err(AppError::invalid_input(message));
            }
        };
        let status = if output.timed_out {
            "timeout"
        } else if output.cancelled {
            "cancelled"
        } else if output.exit_code == Some(0) {
            "success"
        } else {
            "error"
        };
        let summary = format!(
            "exit {} · {} ms · {} bytes of output",
            output
                .exit_code
                .map(|c| c.to_string())
                .unwrap_or_else(|| "none".into()),
            output.millis,
            output.total_bytes
        );
        let _ = self.db.finish_tool_audit(audit_id, status, Some(&summary));
        Ok(CommandRun {
            output,
            cwd: shown_cwd,
            workspace: jail.root_path().to_path_buf(),
        })
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
