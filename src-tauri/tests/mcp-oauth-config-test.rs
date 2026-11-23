//! MCP PKCE wiring: the public config DTOs and the pure helpers behind the
//! two sign-in commands. No keychain, network or Tauri window is involved.

use app_lib::commands::mcp::McpOAuthStatusDto;
use app_lib::config::{
    AppConfig, McpOAuthConfig, McpServerConfig, McpTransport, validate_base_url,
};
use app_lib::services::mcp::{
    DEFAULT_OAUTH_REDIRECT_URI, authorize_url, oauth_token_key, parse_token_response,
    pkce_challenge, token_request, verify_state,
};
use std::collections::BTreeMap;

fn oauth() -> McpOAuthConfig {
    McpOAuthConfig {
        server_id: "mcp-1".into(),
        client_id: "client-1".into(),
        authorize_url: "https://auth.example.com/authorize".into(),
        token_url: "https://auth.example.com/token".into(),
        redirect_uri: DEFAULT_OAUTH_REDIRECT_URI.into(),
        scopes: vec!["mcp.read".into(), "mcp.write".into()],
        audience: Some("mcp://docs".into()),
    }
}

#[test]
fn oauth_config_round_trips_and_defaults_absent_fields() {
    let full = oauth();
    let json = serde_json::to_string(&full).unwrap();
    let back: McpOAuthConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(back, full);
    // Only public client metadata is on the wire; no token field exists.
    assert!(!json.contains("token_secret"));
    assert!(!json.contains("access_token"));

    let partial = r#"{
        "client_id": "client-2",
        "authorize_url": "https://auth.example.com/authorize",
        "token_url": "https://auth.example.com/token"
    }"#;
    let cfg: McpOAuthConfig = serde_json::from_str(partial).unwrap();
    assert_eq!(cfg.client_id, "client-2");
    assert_eq!(cfg.server_id, "");
    assert_eq!(cfg.redirect_uri, "");
    assert!(cfg.scopes.is_empty());
    assert!(cfg.audience.is_none());
}

#[test]
fn server_config_keeps_oauth_and_still_loads_legacy_files() {
    let mut app = AppConfig::default();
    app.mcp_servers.push(McpServerConfig {
        id: "mcp-1".into(),
        name: "Docs".into(),
        enabled: true,
        auto_approve: false,
        enabled_tools: vec![],
        disabled_tools: vec![],
        transport: McpTransport::Http {
            url: "https://mcp.example.com/mcp".into(),
            headers: BTreeMap::new(),
        },
        oauth: Some(oauth()),
    });
    let json = serde_json::to_string(&app).unwrap();
    let back: AppConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(back.mcp_servers.len(), 1);
    let saved = back.mcp_servers[0].oauth.as_ref().unwrap();
    assert_eq!(saved.client_id, "client-1");
    assert_eq!(saved.scopes, vec!["mcp.read", "mcp.write"]);

    let legacy = r#"{"selected_models":[],"cloud_llm_enabled":false,"mcp_servers":[{"id":"m1","name":"Docs","enabled":true,"auto_approve":false,"transport":{"type":"http","url":"https://mcp.example.com/mcp"}}]}"#;
    let old: AppConfig = serde_json::from_str(legacy).unwrap();
    assert!(old.mcp_servers[0].oauth.is_none());
    assert!(matches!(
        old.mcp_servers[0].transport,
        McpTransport::Http { .. }
    ));
}

#[test]
fn oauth_urls_obey_the_https_or_loopback_rule() {
    assert!(validate_base_url("https://auth.example.com/authorize").is_ok());
    assert!(validate_base_url("https://auth.example.com/token").is_ok());
    assert!(validate_base_url("http://127.0.0.1:8765/callback").is_ok());
    assert!(validate_base_url("http://localhost:9000/token").is_ok());
    assert!(validate_base_url("http://auth.example.com/token").is_err());
    assert!(validate_base_url("http://127.evil.com/token").is_err());
    assert!(validate_base_url("http://0.0.0.0:8765/callback").is_err());
    assert!(validate_base_url("https://user:pass@auth.example.com/token").is_err());
    assert!(validate_base_url("ftp://auth.example.com/token").is_err());
    assert!(validate_base_url("not a url").is_err());
}

#[test]
fn pure_pkce_helpers_carry_the_expected_parameters() {
    let cfg = oauth();
    let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    assert_eq!(
        pkce_challenge(verifier),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );

    let url = authorize_url(&cfg, "state-1", "challenge-1").unwrap();
    let pairs: Vec<(String, String)> = url::Url::parse(&url)
        .unwrap()
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    let get = |key: &str| {
        pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .unwrap_or("")
    };
    assert_eq!(get("response_type"), "code");
    assert_eq!(get("client_id"), "client-1");
    assert_eq!(get("redirect_uri"), DEFAULT_OAUTH_REDIRECT_URI);
    assert_eq!(get("state"), "state-1");
    assert_eq!(get("code_challenge"), "challenge-1");
    assert_eq!(get("code_challenge_method"), "S256");
    assert_eq!(get("scope"), "mcp.read mcp.write");
    assert_eq!(get("audience"), "mcp://docs");

    assert!(verify_state("state-1", "state-1").is_ok());
    assert!(verify_state("state-1", "attacker").is_err());
    assert!(verify_state("", "").is_err());

    let request = token_request(&cfg, "code-1", verifier).unwrap();
    assert_eq!(request.url, cfg.token_url);
    let form: Vec<(String, String)> = url::form_urlencoded::parse(request.body.as_bytes())
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    assert!(form.iter().any(|(k, v)| k == "code_verifier" && v == verifier));
    assert!(form.iter().any(|(k, v)| k == "code" && v == "code-1"));

    let token = parse_token_response(r#"{"access_token":"tok","token_type":"Bearer"}"#).unwrap();
    assert_eq!(token.access_token, "tok");
    assert!(parse_token_response(r#"{"error":"access_denied"}"#).is_err());
}

#[test]
fn token_key_is_stable_and_status_never_carries_a_token() {
    assert_eq!(oauth_token_key("mcp-1"), "mcp-oauth:mcp-1");
    assert_eq!(oauth_token_key("mcp-1"), oauth_token_key("mcp-1"));
    assert_ne!(oauth_token_key("mcp-1"), oauth_token_key("mcp-2"));

    let status = McpOAuthStatusDto {
        server_id: "mcp-1".into(),
        signed_in: true,
        message: "Signed in. Kept in the OS keychain.".into(),
        token_type: "Bearer".into(),
        expires_in: Some(3600),
    };
    let json = serde_json::to_string(&status).unwrap();
    assert!(!json.contains("access_token"));
    assert!(!json.contains("code_verifier"));
    assert!(!json.contains("sentinel-token-must-not-appear"));
}
