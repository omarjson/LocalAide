pub mod approval;
pub mod read_only;
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
use crate::config::{AppConfig, CapabilityAutoApprove, HooksConfig, PermissionRule, ToolDecision};
use crate::domain::tool::{
    ToolActionType, ToolAuditEntry, ToolExecutionRequest, ToolExecutionResponse,
};
use crate::services::library::db::LibraryDb;
use approval::ApprovalManager;
use sandbox::WorkspaceJail;
use std::sync::{Arc, Mutex};

/// The policy snapshot used by `ToolService`. It is set from `AppConfig` and
/// kept behind a mutex so a config save takes effect on the next tool call.
#[derive(Debug, Default)]
struct PolicyState {
    rules: Vec<PermissionRule>,
    hooks: HooksConfig,
    /// Per-capability auto-approve (N9). Default is read-only.
    capabilities: CapabilityAutoApprove,
}

#[derive(Debug, Default)]
pub struct ToolPolicy {
    state: Mutex<PolicyState>,
}

impl ToolPolicy {
    pub fn new(rules: Vec<PermissionRule>, hooks: HooksConfig) -> Self {
        Self {
            state: Mutex::new(PolicyState {
                rules,
                hooks,
                capabilities: CapabilityAutoApprove::default(),
            }),
        }
    }

    pub fn set(
        &self,
        rules: Vec<PermissionRule>,
        hooks: HooksConfig,
        capabilities: CapabilityAutoApprove,
    ) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.rules = rules;
        state.hooks = hooks;
        state.capabilities = capabilities;
    }

    /// The per-capability auto-approve snapshot. Defaults to read-only
    /// when no config was ever applied, so a fresh policy asks.
    pub fn capabilities(&self) -> CapabilityAutoApprove {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .capabilities
    }

    /// The last matching rule's decision, or `Ask` when no rule matches.
    pub fn decide(&self, action: &str, resource: &str) -> ToolDecision {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        crate::config::matching_rule(&state.rules, action, resource)
            .map(|rule| ToolDecision::from(rule.decision))
            .unwrap_or(ToolDecision::Ask)
    }

    pub fn hooks(&self) -> HooksConfig {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .hooks
            .clone()
    }

    /// Decide a shell command's approval. An explicit rule (Allow or Deny)
    /// wins; when no rule matches, a fully inert read-only command is allowed
    /// so it can skip the prompt, and everything else still asks.
    ///
    /// Deny is checked first and unconditionally: a user rule that forbids a
    /// command is never overridden by the read-only fast path.
    pub fn decide_shell(&self, command: &str) -> ToolDecision {
        // Both names appear in this app: "shell" is the audit action, while
        // "execute_command" is the tool name the model calls. A rule on
        // either one is honoured so a deny cannot be bypassed by spelling.
        let direct = self.decide("shell", command);
        let tool = self.decide("execute_command", command);
        if direct == ToolDecision::Deny || tool == ToolDecision::Deny {
            return ToolDecision::Deny;
        }
        if direct == ToolDecision::Allow || tool == ToolDecision::Allow {
            return ToolDecision::Allow;
        }
        if read_only::can_skip_approval(command) {
            return ToolDecision::Allow;
        }
        // Per-capability auto-approve (N9): a tame command runs when safe
        // commands are on, any non-blocked command when all commands are
        // on. A blocked command never becomes Allow here — the caller
        // refuses it before consulting the policy.
        if !matches!(shell::classify(command), shell::Verdict::Blocked(_)) {
            let caps = self.capabilities();
            if caps.safe_commands && !shell::risky(command) {
                return ToolDecision::Allow;
            }
            if caps.all_commands {
                return ToolDecision::Allow;
            }
        }
        ToolDecision::Ask
    }
}

pub struct ToolService {
    jail: Arc<WorkspaceJail>,
    db: Arc<LibraryDb>,
    approval: Arc<ApprovalManager>,
    policy: Arc<ToolPolicy>,
}

impl ToolService {
    pub fn new(jail: Arc<WorkspaceJail>, db: Arc<LibraryDb>) -> Self {
        Self {
            jail,
            db,
            approval: Arc::new(ApprovalManager::new()),
            policy: Arc::new(ToolPolicy::default()),
        }
    }

    /// Apply the current config's permission rules and hooks. Call this after
    /// loading or changing the config so the next tool call uses it.
    pub fn apply_config(&self, cfg: &AppConfig) {
        self.policy.set(
            cfg.permissions.clone(),
            cfg.hooks.clone(),
            cfg.auto_approve_capabilities,
        );
    }

    /// The shared policy snapshot, for callers that update it directly.
    pub fn policy(&self) -> Arc<ToolPolicy> {
        self.policy.clone()
    }

    /// The approval decision for a shell command. Explicit deny rules win;
    /// otherwise an inert read-only command may skip the prompt.
    pub fn shell_decision(&self, command: &str) -> ToolDecision {
        self.policy.decide_shell(command)
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

        // Policy: the last matching rule decides. An explicit denial above
        // always wins, so Allow Once / Session / Deny keep their meaning.
        match self.policy.decide(action_name, &req.action.path) {
            ToolDecision::Deny => {
                if let Err(e) = self.db.record_tool_audit(
                    &req.tool_name,
                    action_name,
                    &req.action.path,
                    false,
                    "denied",
                    Some("Blocked by a permission rule"),
                ) {
                    log::warn!("Could not record a policy-denied tool action: {e}");
                }
                return Ok(ToolExecutionResponse {
                    success: false,
                    output: String::new(),
                    truncated: false,
                    requires_approval: false,
                    error: Some("Tool execution denied by a permission rule".to_string()),
                });
            }
            ToolDecision::Ask => {
                if !self.approval.is_approved(&req) {
                    return Ok(ToolExecutionResponse {
                        success: false,
                        output: String::new(),
                        truncated: false,
                        requires_approval: true,
                        error: Some("Operation requires user approval".to_string()),
                    });
                }
            }
            ToolDecision::Allow => {
                // Allowed by policy. Still run the approval check for its side
                // effect: an approval with a session token grants later calls
                // "Allow for this session" in the same folder.
                let _ = self.approval.is_approved(&req);
            }
        }

        // Pre-tool hook. Never run hooks for a target outside the jail.
        if let Some(reason) = self.pre_hook(jail, &req) {
            if let Err(e) = self.db.record_tool_audit(
                &req.tool_name,
                action_name,
                &req.action.path,
                false,
                "blocked",
                Some(&reason),
            ) {
                log::warn!("Could not record a hook-blocked tool action: {e}");
            }
            return Ok(ToolExecutionResponse {
                success: false,
                output: String::new(),
                truncated: false,
                requires_approval: false,
                error: Some(reason),
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

        // Post-tool hook is informational: a failure here never undoes an
        // action that already ran, it is only logged.
        self.post_hook(jail, &req, exec_result.is_ok());

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

    /// Run the configured pre-tool hook, if any. Returns a block reason when
    /// the hook refuses the call. Hooks never run for paths outside the jail.
    fn pre_hook(&self, jail: &WorkspaceJail, req: &ToolExecutionRequest) -> Option<String> {
        let hooks = self.policy.hooks();
        let command = hooks.pre_tool.as_deref()?;
        if command.trim().is_empty() || !Self::hook_target_is_in_jail(jail, req) {
            return None;
        }
        let outcome = jail.run_hook(command, hooks.timeout());
        if !outcome.blocked {
            return None;
        }
        let reason = if outcome.timed_out {
            "The pre-tool hook timed out".to_string()
        } else {
            let detail = outcome
                .stderr
                .trim()
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(200)
                .collect::<String>();
            if detail.is_empty() {
                "A pre-tool hook blocked this action".to_string()
            } else {
                format!("A pre-tool hook blocked this action: {detail}")
            }
        };
        Some(reason)
    }

    /// Run the configured post-tool hook, if any. Never blocks the tool call.
    fn post_hook(&self, jail: &WorkspaceJail, req: &ToolExecutionRequest, succeeded: bool) {
        let hooks = self.policy.hooks();
        let Some(command) = hooks.post_tool.as_deref() else {
            return;
        };
        if command.trim().is_empty() || !Self::hook_target_is_in_jail(jail, req) {
            return;
        }
        let outcome = jail.run_hook(command, hooks.timeout());
        if outcome.blocked {
            log::warn!(
                "Post-tool hook failed (succeeded={succeeded}): {}",
                outcome.stderr.trim()
            );
        }
    }

    /// An empty path means the workspace root, which is inside the jail.
    fn hook_target_is_in_jail(jail: &WorkspaceJail, req: &ToolExecutionRequest) -> bool {
        let path = req.action.path.trim();
        jail.contains_path(if path.is_empty() { "." } else { path })
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
        // Policy: an explicit deny rule wins over the read-only fast path.
        // This path runs after the caller has asked for approval, so a deny is
        // a hard refusal rather than another prompt.
        if matches!(self.policy.decide_shell(command), ToolDecision::Deny) {
            self.record_event(
                &format!("{source}:execute_command"),
                "shell",
                command,
                "denied",
                Some("Blocked by a permission rule"),
            );
            return Err(AppError::new(
                "COMMAND_DENIED",
                "Command execution denied by a permission rule",
            ));
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
    use crate::config::{
        AppConfig, CapabilityAutoApprove, HooksConfig, PermissionDecision, PermissionRule,
    };
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

    #[test]
    fn a_deny_rule_blocks_even_an_approved_call() {
        let (tools, dir) = service("deny-rule");
        tools.apply_config(&AppConfig {
            permissions: vec![PermissionRule {
                action: "*".into(),
                resource: "*".into(),
                decision: PermissionDecision::Deny,
            }],
            ..AppConfig::default()
        });
        let response = tools.execute(read("a.txt")).unwrap();
        assert!(!response.success);
        assert!(!response.requires_approval);
        assert!(response.error.unwrap().contains("permission rule"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_allow_rule_skips_the_approval_prompt() {
        let (tools, dir) = service("allow-rule");
        tools.apply_config(&AppConfig {
            permissions: vec![PermissionRule {
                action: "write_file".into(),
                resource: "*".into(),
                decision: PermissionDecision::Allow,
            }],
            ..AppConfig::default()
        });
        let mut request = read("a.txt");
        request.action.action_type = ToolActionType::WriteFile;
        request.approved = None;
        let response = tools.execute(request).unwrap();
        assert!(response.success);
        assert!(!response.requires_approval);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_blocking_pre_tool_hook_stops_the_call() {
        let (tools, dir) = service("hook-block");
        tools.apply_config(&AppConfig {
            hooks: HooksConfig {
                pre_tool: Some(if cfg!(windows) {
                    "exit /b 1".to_string()
                } else {
                    "exit 1".to_string()
                }),
                ..HooksConfig::default()
            },
            ..AppConfig::default()
        });
        let response = tools.execute(read("a.txt")).unwrap();
        assert!(!response.success);
        assert!(response.error.unwrap().contains("hook"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn inert_shell_commands_skip_approval() {
        let (tools, dir) = service("shell-inert");
        assert_eq!(
            tools.shell_decision("ls -la && git status"),
            crate::config::ToolDecision::Allow
        );
        assert_eq!(
            tools.shell_decision("pwd; git log --oneline -3"),
            crate::config::ToolDecision::Allow
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_compound_command_with_rm_still_asks() {
        let (tools, dir) = service("shell-rm");
        assert_eq!(
            tools.shell_decision("ls && rm -f notes.txt"),
            crate::config::ToolDecision::Ask
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_deny_rule_beats_the_read_only_fast_path() {
        let (tools, dir) = service("shell-deny");
        tools.apply_config(&AppConfig {
            permissions: vec![PermissionRule {
                action: "shell".into(),
                resource: "*".into(),
                decision: PermissionDecision::Deny,
            }],
            ..AppConfig::default()
        });
        assert_eq!(
            tools.shell_decision("ls -la && git status"),
            crate::config::ToolDecision::Deny
        );
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let err = tools
            .run_command(
                "test",
                "ls -la && git status",
                None,
                std::time::Duration::from_secs(5),
                &cancel,
                None,
            )
            .err()
            .expect("a deny rule must refuse the command");
        assert_eq!(err.code, "COMMAND_DENIED");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_deny_rule_on_the_tool_name_also_wins() {
        let (tools, dir) = service("shell-tool-deny");
        tools.apply_config(&AppConfig {
            permissions: vec![PermissionRule {
                action: "execute_command".into(),
                resource: "*".into(),
                decision: PermissionDecision::Deny,
            }],
            ..AppConfig::default()
        });
        assert_eq!(
            tools.shell_decision("ls -la && git status"),
            crate::config::ToolDecision::Deny
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn capability_toggles_auto_approve_commands_by_tameness() {
        // Default (read-only): a changing command still asks.
        let (tools, dir) = service("shell-caps-default");
        assert_eq!(
            tools.shell_decision("git status"),
            crate::config::ToolDecision::Allow,
            "read-only commands need no toggle"
        );
        assert_eq!(
            tools.shell_decision("mkdir -p notes"),
            crate::config::ToolDecision::Ask,
            "a changing command asks by default"
        );

        // Safe commands on: a tame change runs, a risky one still asks.
        tools.apply_config(&AppConfig {
            auto_approve_capabilities: CapabilityAutoApprove {
                safe_commands: true,
                ..CapabilityAutoApprove::default()
            },
            ..AppConfig::default()
        });
        assert_eq!(
            tools.shell_decision("mkdir -p notes"),
            crate::config::ToolDecision::Allow
        );
        assert_eq!(
            tools.shell_decision("rm -rf /tmp/work"),
            crate::config::ToolDecision::Ask,
            "risky stays asking under safe-commands-only"
        );

        // All commands on: any non-blocked command runs.
        tools.apply_config(&AppConfig {
            auto_approve_capabilities: CapabilityAutoApprove {
                all_commands: true,
                ..CapabilityAutoApprove::default()
            },
            ..AppConfig::default()
        });
        assert_eq!(
            tools.shell_decision("mv notes/a.txt notes/b.txt"),
            crate::config::ToolDecision::Allow,
            "all commands runs a risky-but-not-blocked command"
        );
        assert_eq!(
            tools.shell_decision("rm -rf /tmp/work"),
            crate::config::ToolDecision::Ask,
            "a blocked command stays asking even with all commands on"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
