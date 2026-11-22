// The Rust commands are only useful if the UI can actually reach them. These
// checks guard the typed API surface and the command palette wiring without
// needing a Tauri window or a Svelte compiler.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const read = (path: string) => readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");

const client = read("src/lib/api/client.ts");
const bindings = read("src/lib/api/bindings.ts");
const app = read("src/App.svelte");

/** [client method, Tauri command name] for every newly reachable command. */
const COMMANDS: [string, string][] = [
  ["sessionSearch", "session_search"],
  ["sessionReindex", "session_reindex"],
  ["doctorRun", "doctor_run"],
  ["agentsList", "agents_list"],
  ["agentsSave", "agents_save"],
  ["multiRunChat", "multi_run_chat"],
  ["contextFilesList", "context_files_list"],
  ["contextRefsResolve", "context_refs_resolve"],
  ["policyConfigGet", "policy_config_get"],
  ["policyConfigSet", "policy_config_set"],
];

test("typed client exposes a method for every new command", () => {
  for (const [method, command] of COMMANDS) {
    assert.match(
      client,
      new RegExp(`async\\s+${method}\\s*\\([^)]*\\):\\s*Promise<[^>]+>`),
      `ApiClient.${method} should be typed`,
    );
    assert.ok(client.includes(`"${command}"`), `ApiClient should invoke ${command}`);
  }
});

test("bindings define the DTO shapes used by the new methods", () => {
  for (const type of [
    "SessionHit",
    "DoctorReport",
    "DoctorCheck",
    "Agent",
    "RunTarget",
    "RunAnswer",
    "ContextFilesResult",
    "RefResult",
    "PolicySettings",
    "PolicyUpdate",
  ]) {
    assert.match(bindings, new RegExp(`export interface ${type}\\b`), `${type} DTO is missing`);
  }
  // The Rust field names are part of the IPC contract; keep the key spellings.
  assert.match(bindings, /thread_id: string;/);
  assert.match(bindings, /api_key\?: string \| null;/);
});

test("command palette builds its Go to entries from SCREEN_REGISTRY", () => {
  assert.match(app, /import \{ SCREEN_REGISTRY, screenForKey, type ScreenId \} from "\.\/lib\/screens"/);
  assert.match(app, /SCREEN_REGISTRY\.map\(\(s\) => \(\{/);
  // The action list is separate, but still lives in the same palette source.
  assert.match(app, /id: "search-chats"/);
  assert.match(app, /id: "compare-models"/);
});

test("the two new palette actions open their real dialogs", () => {
  assert.match(app, /<ChatSearchModal[\s\S]*?onOpen=\{openChatHit\}/);
  assert.match(app, /<MultiRunModal/);
  assert.match(app, /openChat\(saved \?\? \{/);
});

test("title bar tabs use roving tabindex and keep close out of the tab", () => {
  const bar = read("src/components/TitleBar.svelte");
  assert.match(bar, /ArrowLeft/);
  assert.match(bar, /ArrowRight/);
  assert.match(bar, /tabindex=\{tab\.key === rovingKey \? 0 : -1\}/);
  // Close is a sibling of the role=tab element, never nested inside it.
  assert.match(bar, /role="tab"[\s\S]*?<\/div>\s*<button[\s\S]*?Close the tab/);
});

test("all new icons are in the Icon union and the if/else chain", () => {
  const icon = read("src/components/Icon.svelte");
  for (const name of [
    "compare", "target", "calendar", "puzzle", "route",
    "shield_check", "keychain", "stethoscope", "panel", "history", "merge",
  ]) {
    assert.match(icon, new RegExp(`\\| "${name}"`), `"${name}" missing from IconName`);
    assert.match(icon, new RegExp(`name === "${name}"`), `"${name}" has no icon path`);
  }
});
