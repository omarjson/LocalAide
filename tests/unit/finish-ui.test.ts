// Guards the frontend-only finish work: the seven Settings groups and the typed
// IPC methods for goals/usage/plugins.
// No Tauri window, no Svelte compiler, no fabricated data.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { SETTINGS_GROUPS, SETTINGS_GROUP_OF } from "../../src/lib/screens.ts";

const read = (path: string) => readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");
const client = read("src/lib/api/client.ts");
const bindings = read("src/lib/api/bindings.ts");
const settings = read("src/screens/SettingsScreen.svelte");

const GROUPS: [string, string][] = [
  ["general", "General"],
  ["online", "Online providers"],
  ["tools", "Tools & MCP"],
  ["integrations", "Integrations"],
  ["privacy", "Privacy & files"],
  ["about", "About & diagnostics"],
  ["developer", "Developer"],
];

test("Settings has exactly the seven groups, in screens.ts order", () => {
  assert.deepEqual(
    SETTINGS_GROUPS.map((g) => [g.id, g.label]),
    GROUPS,
  );
  assert.equal(new Set(SETTINGS_GROUPS.map((g) => g.id)).size, 7);
  const ids = new Set(SETTINGS_GROUPS.map((g) => g.id));
  for (const value of Object.values(SETTINGS_GROUP_OF)) assert.ok(ids.has(value));
});

test("every existing Settings section still belongs to a group", () => {
  const SECTIONS = [
    "general", "voice", "share", "online", "web", "tools",
    "integrations", "privacy", "backups", "activity", "developer", "about",
  ];
  assert.deepEqual(Object.keys(SETTINGS_GROUP_OF).sort(), [...SECTIONS].sort());
  assert.deepEqual(
    SETTINGS_GROUP_OF,
    {
      general: "general", voice: "general", share: "general",
      online: "online", web: "online",
      tools: "tools",
      integrations: "integrations",
      privacy: "privacy", backups: "privacy",
      about: "about", activity: "about",
      developer: "developer",
    },
  );
  // The switcher is rendered from the constants, and no section branch was
  // dropped while regrouping.
  assert.match(settings, /SETTINGS_GROUPS/);
  assert.match(settings, /activeGroup/);
  for (const section of SECTIONS) {
    assert.match(settings, new RegExp(`activeTab === "${section}"`), `${section} section is missing`);
  }
});

const NEW_METHODS: [string, string][] = [
  ["goalStart", "goal_start"],
  ["goalStatus", "goal_status"],
  ["goalStep", "goal_step"],
  ["goalComplete", "goal_complete"],
  ["usageRecord", "usage_record"],
  ["usageList", "usage_list"],
  ["pluginList", "plugin_list"],
];

test("typed client reaches goal, usage and plugin commands", () => {
  for (const [method, command] of NEW_METHODS) {
    assert.match(
      client,
      new RegExp(`async\\s+${method}\\s*\\([^)]*\\):\\s*Promise<[^>]+>`),
      `ApiClient.${method} should be typed`,
    );
    assert.ok(client.includes(`"${command}"`), `ApiClient should invoke ${command}`);
  }
  for (const type of ["Goal", "UsageLedgerEntry", "UsageRecordRequest", "PluginDescriptor"]) {
    assert.match(bindings, new RegExp(`export interface ${type}\\b`), `${type} DTO is missing`);
  }
  // DTO fields mirror the Rust structs, including snake_case response fields.
  assert.match(bindings, /thread_id: string;/);
  assert.match(bindings, /estimated_cost_usd: number;/);
});

test("plugin frames allow scripts only, never same-origin", () => {
  const panels = read("src/components/PluginPanels.svelte");
  assert.match(panels, /sandbox="allow-scripts"/);
  assert.doesNotMatch(panels, /allow-same-origin/);
  assert.match(panels, /try \{[\s\S]*api\.pluginList\(\)/);
  assert.match(panels, /No plugin panels are installed/);
});
