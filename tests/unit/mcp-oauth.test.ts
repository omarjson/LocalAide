// Guards the MCP PKCE plumbing without a Tauri window: the typed DTOs, the
// two reachable commands, and the Settings panel that never shows a token.
// The real serde round-trip and URL validation live in the Rust test file.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const read = (path: string) => readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");

const bindings = read("src/lib/api/bindings.ts");
const client = read("src/lib/api/client.ts");
const panel = read("src/components/McpOAuthPanel.svelte");
const settings = read("src/screens/SettingsScreen.svelte");
const rustConfig = read("src-tauri/src/config.rs");
const rustCommands = read("src-tauri/src/commands/mcp.rs");
const rustService = read("src-tauri/src/services/mcp/mod.rs");

test("bindings carry the OAuth DTOs and no raw token field", () => {
  for (const name of [
    "McpOAuthConfig",
    "McpOAuthStartDto",
    "McpOAuthFinishRequest",
    "McpOAuthStatusDto",
  ]) {
    assert.match(bindings, new RegExp(`export interface ${name}\\b`), `${name} is missing`);
  }
  assert.match(bindings, /oauth: McpOAuthConfig \| null;/);
  assert.match(bindings, /oauth\?: McpOAuthConfig \| null;/);
  assert.match(bindings, /authorize_url: string;/);
  assert.match(bindings, /token_url: string;/);
  assert.match(bindings, /redirect_uri: string;/);
  assert.match(bindings, /scopes: string\[\];/);
  assert.ok(!bindings.includes("access_token:"), "no token may cross IPC");
});

test("the typed client reaches both new commands", () => {
  assert.match(
    client,
    /async mcpOAuthStart\(serverId: string\): Promise<McpOAuthStartDto>/,
  );
  assert.match(
    client,
    /async mcpOAuthFinish\(req: McpOAuthFinishRequest\): Promise<McpOAuthStatusDto>/,
  );
  assert.ok(client.includes('"mcp_oauth_start"'), "start command is not invoked");
  assert.ok(client.includes('"mcp_oauth_finish"'), "finish command is not invoked");
});

test("the panel collects public client fields and only renders status", () => {
  for (const label of [
    "Authorization URL",
    "Token URL",
    "Client ID",
    "Scopes",
    "Finish sign-in",
  ]) {
    assert.ok(panel.includes(label), `${label} is missing from the panel`);
  }
  assert.match(panel, /api\.mcpOAuthStart\(selectedId\)/);
  assert.match(panel, /openInBrowser\(event, started\.authorize_url\)/);
  assert.match(panel, /api\.mcpOAuthFinish\(\{/);
  assert.match(panel, /status = done\.message/);
  assert.ok(!panel.includes("access_token"), "the panel never names a token field");
  assert.ok(!panel.includes("code_verifier"), "the panel never sees the verifier");
});

test("the panel lives on the Tools & MCP settings tab", () => {
  assert.match(
    settings,
    /import McpOAuthPanel from "\.\.\/components\/McpOAuthPanel\.svelte"/,
  );
  assert.match(
    settings,
    /activeTab === "tools"[\s\S]{0,240}<McpSettings \/>\s*<McpOAuthPanel \/>/,
  );
});

test("the backend keeps tokens in the keychain, not in config or DTOs", () => {
  assert.match(rustConfig, /pub oauth: Option<McpOAuthConfig>/);
  assert.match(rustCommands, /pub async fn mcp_oauth_start/);
  assert.match(rustCommands, /pub async fn mcp_oauth_finish/);
  assert.match(
    rustCommands,
    /secrets::save\(&oauth_token_key\(&req\.server_id\), &token\.access_token\)/,
  );
  assert.match(rustCommands, /verify_state\(&pkce\.state/);
  assert.match(rustService, /pub fn oauth_token_key\(server_id: &str\) -> String/);
  assert.match(rustService, /fn take_sign_in\(server_id: &str\)/);
  assert.match(rustConfig, /pub fn validate_base_url\(url: &str\) -> Result<\(\), String>/);
});
