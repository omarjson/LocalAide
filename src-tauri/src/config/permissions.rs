//! Permission policy: what a tool call may do without asking (Settings →
//! Permissions). Rules match an action and a resource with `*` wildcards;
//! the last matching rule wins and no rule means ask. Moved out of
//! `config.rs` unchanged in 0.2.4; re-exported from `crate::config`.

use serde::{Deserialize, Serialize};

use super::default_true;

/// What a matching permission rule says to do with a tool call.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionDecision {
    Allow,
    Ask,
    Deny,
}

/// The backend's view of a tool decision. `Ask` keeps the existing
/// Allow Once / Allow for this session / Deny approval flow in place.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolDecision {
    Allow,
    Ask,
    Deny,
}

/// One capability a tool call can belong to, for per-capability
/// auto-approve (N9). Serialized snake_case for config.json.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    ReadFiles,
    EditFiles,
    SafeCommands,
    AllCommands,
    Browser,
    ExternalTools,
}

/// Which capabilities run without asking. Default is read-only: only
/// reads are auto-approved, everything else asks first. A missing field
/// in an old config file falls back to the same default, so an unknown
/// or missing config never auto-approves a change.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct CapabilityAutoApprove {
    #[serde(default = "default_true")]
    pub read_files: bool,
    #[serde(default)]
    pub edit_files: bool,
    #[serde(default)]
    pub safe_commands: bool,
    #[serde(default)]
    pub all_commands: bool,
    #[serde(default)]
    pub browser: bool,
    #[serde(default)]
    pub external_tools: bool,
}

impl Default for CapabilityAutoApprove {
    fn default() -> Self {
        Self {
            read_files: true,
            edit_files: false,
            safe_commands: false,
            all_commands: false,
            browser: false,
            external_tools: false,
        }
    }
}

impl CapabilityAutoApprove {
    /// Whether this capability runs without asking. "All commands"
    /// covers safe commands too, so a safe command is auto-approved
    /// when either toggle is on.
    pub fn approves(&self, capability: Capability) -> bool {
        match capability {
            Capability::ReadFiles => self.read_files,
            Capability::EditFiles => self.edit_files,
            Capability::SafeCommands => self.safe_commands || self.all_commands,
            Capability::AllCommands => self.all_commands,
            Capability::Browser => self.browser,
            Capability::ExternalTools => self.external_tools,
        }
    }
}

impl From<PermissionDecision> for ToolDecision {
    fn from(value: PermissionDecision) -> Self {
        match value {
            PermissionDecision::Allow => ToolDecision::Allow,
            PermissionDecision::Ask => ToolDecision::Ask,
            PermissionDecision::Deny => ToolDecision::Deny,
        }
    }
}

/// One policy rule. `action` and `resource` are glob patterns where `*` matches
/// any run of characters. Rules are evaluated in order and the last match wins.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionRule {
    pub action: String,
    pub resource: String,
    pub decision: PermissionDecision,
}

impl PermissionRule {
    /// True when this rule applies to the given tool action and resource.
    pub fn matches(&self, action: &str, resource: &str) -> bool {
        wildcard_match(&self.action, action) && wildcard_match(&self.resource, resource)
    }
}

/// Glob-style match: `*` matches zero or more characters, all other characters
/// match literally. Pure and allocation-light enough for a handful of rules.
pub fn wildcard_match(pattern: &str, value: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let s: Vec<char> = value.chars().collect();
    let (mut pi, mut si) = (0usize, 0usize);
    let mut star: Option<usize> = None;
    let mut retry = 0usize;
    while si < s.len() {
        if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            retry = si;
            pi += 1;
        } else if pi < p.len() && p[pi] == s[si] {
            pi += 1;
            si += 1;
        } else if let Some(star_idx) = star {
            pi = star_idx + 1;
            retry += 1;
            si = retry;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// The last rule that matches, if any. Later entries override earlier ones.
pub fn matching_rule<'a>(
    rules: &'a [PermissionRule],
    action: &str,
    resource: &str,
) -> Option<&'a PermissionRule> {
    rules.iter().rev().find(|r| r.matches(action, resource))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    fn rule(action: &str, resource: &str, decision: PermissionDecision) -> PermissionRule {
        PermissionRule {
            action: action.to_string(),
            resource: resource.to_string(),
            decision,
        }
    }

    #[test]
    fn wildcards_match_exactly_as_expected() {
        assert!(wildcard_match("*", "anything"));
        assert!(wildcard_match("*", ""));
        assert!(wildcard_match("write_*", "write_file"));
        assert!(!wildcard_match("write_*", "write"));
        assert!(!wildcard_match("write_*", "read_file"));
        assert!(wildcard_match("notes/*", "notes/a/b.txt"));
        assert!(!wildcard_match("notes/*", "other/a.txt"));
        assert!(wildcard_match("a*c", "abc"));
        assert!(wildcard_match("a*c", "ac"));
        assert!(!wildcard_match("a*c", "ab"));
    }

    #[test]
    fn last_matching_rule_wins() {
        let rules = vec![
            rule("*", "*", PermissionDecision::Deny),
            rule("read_*", "*", PermissionDecision::Allow),
            rule("read_*", "secret/*", PermissionDecision::Deny),
        ];
        let cfg = AppConfig {
            permissions: rules,
            ..AppConfig::default()
        };
        assert_eq!(
            cfg.decide_tool_call("read_file", "notes/a.txt"),
            ToolDecision::Allow
        );
        assert_eq!(
            cfg.decide_tool_call("read_file", "secret/key.txt"),
            ToolDecision::Deny,
            "the later, more specific deny overrides the earlier allow"
        );
        assert_eq!(
            cfg.decide_tool_call("write_file", "notes/a.txt"),
            ToolDecision::Deny
        );
    }

    #[test]
    fn no_rule_means_ask() {
        let cfg = AppConfig::default();
        assert_eq!(
            cfg.decide_tool_call("write_file", "anything"),
            ToolDecision::Ask
        );
    }
}
