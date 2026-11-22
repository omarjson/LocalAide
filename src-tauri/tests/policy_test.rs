//! Pure policy tests: permission-rule precedence, agent frontmatter parsing,
//! MCP tool gating and credential rotation. None of these need the UI or the
//! network.

use std::collections::BTreeMap;

use app_lib::commands::developer::{FLAGS, MODULES};
use app_lib::config::{
    AppConfig, CredentialPool, HooksConfig, PermissionDecision, PermissionRule, ToolDecision,
    matching_rule, wildcard_match,
};
use app_lib::services::agents::parse_agent;
use app_lib::services::mcp::{McpServerConfig, McpTool, McpTransport, filter_tools, tool_enabled};
use serde_json::json;

fn rule(action: &str, resource: &str, decision: PermissionDecision) -> PermissionRule {
    PermissionRule {
        action: action.into(),
        resource: resource.into(),
        decision,
    }
}

#[test]
fn permission_rules_use_last_match_wins() {
    let rules = vec![
        rule("*", "*", PermissionDecision::Deny),
        rule("read_*", "*", PermissionDecision::Allow),
        rule("read_*", "secrets/*", PermissionDecision::Ask),
        rule("read_*", "secrets/public/*", PermissionDecision::Allow),
    ];

    // The broad deny is overridden by the later allow for reads.
    assert_eq!(
        matching_rule(&rules, "read_file", "notes/a.md").map(|r| r.decision),
        Some(PermissionDecision::Allow),
    );
    // A still later, broader ask wins for the secrets folder.
    assert_eq!(
        matching_rule(&rules, "read_file", "secrets/key.pem").map(|r| r.decision),
        Some(PermissionDecision::Ask),
    );
    // The final, most specific allow wins again.
    assert_eq!(
        matching_rule(&rules, "read_file", "secrets/public/logo.png").map(|r| r.decision),
        Some(PermissionDecision::Allow),
    );
    // A write still hits the first, broad deny.
    assert_eq!(
        matching_rule(&rules, "write_file", "notes/a.md").map(|r| r.decision),
        Some(PermissionDecision::Deny),
    );
}

#[test]
fn config_decide_tool_call_maps_rules_and_defaults_to_ask() {
    let cfg = AppConfig {
        permissions: vec![
            rule("write_file", "*", PermissionDecision::Allow),
            rule("write_file", "protected/*", PermissionDecision::Deny),
        ],
        ..AppConfig::default()
    };
    assert_eq!(
        cfg.decide_tool_call("write_file", "notes/a.md"),
        ToolDecision::Allow
    );
    assert_eq!(
        cfg.decide_tool_call("write_file", "protected/a.md"),
        ToolDecision::Deny
    );
    // No rule for reads: the question still goes to the user.
    assert_eq!(
        cfg.decide_tool_call("read_file", "notes/a.md"),
        ToolDecision::Ask
    );
}

#[test]
fn wildcard_patterns_are_anchored() {
    assert!(wildcard_match("*", "anything/at/all"));
    assert!(wildcard_match("write_*", "write_file"));
    assert!(!wildcard_match("write_*", "write"));
    assert!(wildcard_match("notes/*", "notes/deep/file.txt"));
    assert!(!wildcard_match("notes/*", "other/file.txt"));
    assert!(wildcard_match("a*c", "abc"));
    assert!(!wildcard_match("a*c", "abd"));
}

#[test]
fn agent_frontmatter_parses_all_fields() {
    let raw = "---\nname: Reviewer\nmodel: gpt-4o-mini\ntools: read_file, write_file\n---\n\nLook for bugs.";
    let agent = parse_agent(raw);
    assert_eq!(agent.name, "Reviewer");
    assert_eq!(agent.model, "gpt-4o-mini");
    assert_eq!(agent.tools, vec!["read_file", "write_file"]);
    assert_eq!(agent.prompt, "Look for bugs.");
}

#[test]
fn agent_frontmatter_supports_bracketed_tools_and_quotes() {
    let raw = "---\nname: \"Helper\"\ntools: [\"read_file\", 'search_files']\nprompt: Use short answers\nunknown: ignored\n---\n\nBody is ignored.";
    let agent = parse_agent(raw);
    assert_eq!(agent.name, "Helper");
    assert_eq!(agent.tools, vec!["read_file", "search_files"]);
    // An explicit prompt key wins over the markdown body.
    assert_eq!(agent.prompt, "Use short answers");
    assert!(agent.model.is_empty());
}

#[test]
fn agent_without_frontmatter_is_all_prompt() {
    let agent = parse_agent("Be concise.");
    assert!(agent.name.is_empty());
    assert!(agent.tools.is_empty());
    assert_eq!(agent.prompt, "Be concise.");
}

#[test]
fn mcp_tool_gates_keep_backward_compatibility() {
    let mut cfg = McpServerConfig {
        id: "m1".into(),
        name: "Docs".into(),
        enabled: true,
        auto_approve: false,
        enabled_tools: Vec::new(),
        disabled_tools: Vec::new(),
        transport: McpTransport::Http {
            url: "https://mcp.example.com".into(),
            headers: BTreeMap::new(),
        },
    };
    let mk = |name: &str| McpTool {
        name: name.to_string(),
        description: String::new(),
        input_schema: json!({}),
    };
    let tools = vec![mk("search"), mk("fetch"), mk("delete")];
    assert_eq!(filter_tools(tools.clone(), &cfg).len(), 3);

    // A deny-list removes tools; an allow-list limits everything else.
    cfg.disabled_tools = vec!["delete".into()];
    cfg.enabled_tools = vec!["search".into(), "fetch".into(), "delete".into()];
    let kept: Vec<&str> = filter_tools(tools, &cfg)
        .iter()
        .map(|t| t.name.as_str())
        .collect();
    assert_eq!(kept, vec!["search", "fetch"]);
    assert!(tool_enabled(&cfg, "search"));
    assert!(!tool_enabled(&cfg, "delete"));
    assert!(!tool_enabled(&cfg, "unknown"));
}

#[test]
fn credential_pool_rotates_on_401_and_429_only() {
    let mut pool = CredentialPool::new(vec!["one".into(), "two".into()]);
    assert_eq!(pool.current(), Some("one"));
    assert!(!pool.on_auth_failure(500));
    assert_eq!(pool.current(), Some("one"));
    assert!(pool.on_auth_failure(401));
    assert_eq!(pool.current(), Some("two"));
    assert!(pool.on_auth_failure(429));
    assert_eq!(pool.current(), Some("one"));
    assert!(!CredentialPool::new(vec!["x".into()]).on_auth_failure(401));
}

#[test]
fn old_configs_load_with_empty_policy_sections() {
    let old = r#"{"selected_models":[],"cloud_llm_enabled":false}"#;
    let cfg: AppConfig = serde_json::from_str(old).unwrap();
    assert!(cfg.permissions.is_empty());
    assert_eq!(cfg.hooks, HooksConfig::default());
    assert!(cfg.routing.provider_priority.is_empty());
    assert!(cfg.routing.key_pools.is_empty());
}

#[test]
fn policy_toggles_are_registered_in_both_arrays() {
    for id in [
        "perf_overlay",
        "trace_ipc_sizes",
        "vector_search_debug",
        "mock_slow_backend",
        "show_keychain_state",
        "log_redaction_preview",
        "agent_debug",
        "tab_debug",
    ] {
        assert!(FLAGS.iter().any(|t| t.id == id), "missing flag {id}");
    }
    for id in [
        "session_search",
        "multi_run",
        "agent_files",
        "goal_mode",
        "remote_ui",
        "plugin_panels",
    ] {
        assert!(MODULES.iter().any(|t| t.id == id), "missing module {id}");
    }
}
