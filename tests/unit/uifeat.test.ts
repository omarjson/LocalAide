// Static wiring checks for Stream B. The Svelte components and the Tauri
// client cannot be imported in a plain Node process, so this reads them as
// source and asserts the real contracts the UI relies on.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const read = (path: string) => readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");

const app = read("src/App.svelte");
const settings = read("src/screens/SettingsScreen.svelte");
const client = read("src/lib/api/client.ts");
const bindings = read("src/lib/api/bindings.ts");
const picker = read("src/components/SessionPicker.svelte");
const search = read("src/components/ChatSearchModal.svelte");
const policy = read("src/components/PolicyPanel.svelte");
const notificationsPanel = read("src/components/NotificationsPanel.svelte");
const fusion = read("src/components/MultiRunFusion.svelte");
const notifier = read("src/lib/notifications.svelte.ts");

test("session picker keeps FTS and shows busy/open-in-tab state", () => {
  assert.match(picker, /api\.sessionSearch\(q, 50\)/);
  assert.match(picker, /api\.sessionReindex\(\)/);
  assert.match(picker, /chats\.tabs\.find/);
  assert.match(picker, /tab\.busy/);
  assert.match(picker, /session-busy/);
  assert.match(picker, /Open in tab/);
  assert.match(picker, /onOpen\(hit\)/);
  // The original FTS modal is extended too, not left inconsistent.
  assert.match(search, /api\.sessionSearch\(q, 50\)/);
  assert.match(search, /t\.id === hit\.thread_id && t\.busy/);
  assert.match(search, /Open in tab/);
});

test("App wires the picker and fusion dialog without dropping the old ones", () => {
  assert.match(app, /import SessionPicker from/);
  assert.match(app, /import MultiRunFusion from/);
  assert.match(app, /<SessionPicker[\s\S]*?onOpen=\{openChatHit\}/);
  assert.match(app, /<MultiRunFusion/);
  assert.match(app, /<ChatSearchModal[\s\S]*?onOpen=\{openChatHit\}/);
  assert.match(app, /<MultiRunModal/);
  assert.match(app, /id: "session-picker"/);
  assert.match(app, /id: "fuse-models"/);
});

test("policy panel edits rules and hooks through the real IPC methods", () => {
  assert.match(policy, /api\.policyConfigGet\(\)/);
  assert.match(policy, /api\.policyConfigSet\(/);
  assert.match(policy, /permissions: clean/);
  assert.match(policy, /pre_tool: preTool\.trim\(\)/);
  assert.match(policy, /post_tool: postTool\.trim\(\)/);
  assert.match(policy, /timeout_secs/);
  for (const decision of ["allow", "ask", "deny"]) {
    assert.match(policy, new RegExp(`"${decision}"`), `${decision} decision is missing`);
  }
  assert.match(client, /async policyConfigGet\(\): Promise<PolicySettings>/);
  assert.match(client, /async policyConfigSet\(update: PolicyUpdate\): Promise<PolicySettings>/);
  assert.match(bindings, /export interface PermissionRule/);
  assert.match(bindings, /export interface HooksConfig/);
  assert.match(settings, /<PolicyPanel \/>/);
});

test("notifications panel is opt-in, local-only and rendered in Settings", () => {
  assert.match(notificationsPanel, /loadNotificationPrefs\(\)/);
  assert.match(notificationsPanel, /saveNotificationPrefs\(prefs\)/);
  assert.match(notificationsPanel, /requestNotificationPermission\(\)/);
  assert.match(notificationsPanel, /fireNotification\("goal", "LocalAide test"/);
  assert.match(notifier, /new Notification\(title/);
  assert.match(notifier, /createOscillator\(\)/);
  assert.match(notifier, /localStorage/);
  assert.doesNotMatch(notifier, /fetch\(|XMLHttpRequest|WebSocket/);
  assert.match(settings, /<NotificationsPanel \/>/);
});

test("notifications hook the real event flow, never an invented Tauri event", () => {
  // Only these event names are actually emitted by this app's Rust side.
  const realEvents = new Set(["engine-download", "engine-status", "automations-changed"]);
  const listens = [...app.matchAll(/listen(?:<[^>]*>)?\(\s*"([^"]+)"/g)].map((m) => m[1]);
  assert.ok(listens.length > 0, "there should be at least one real listener");
  for (const name of listens) {
    assert.ok(realEvents.has(name), `invented listen event: ${name}`);
  }
  for (const invented of ["goal-complete", "turn-finished", "tool-approval-requested", "model-question"]) {
    assert.equal(app.includes(invented), false, `should not listen for a made-up ${invented} event`);
  }
  // The real signals: the busy callback in App, and the actual stream tags.
  assert.match(app, /document\.hidden/);
  assert.match(app, /notifyTurnFinished\(\)/);
  assert.match(client, /event\.type === "tool_approval"/);
  assert.match(client, /event\.type === "question"/);
  assert.match(client, /notifyGoalComplete\(`Goal finished/);
  assert.match(client, /onEvent\(event\)/);
});

test("fusion is a client-side join with a keep-or-fuse choice", () => {
  assert.match(fusion, /Keep best/);
  assert.match(fusion, /Fuse answers/);
  assert.match(fusion, /fuseRunAnswers\(answers\)/);
  assert.match(fusion, /choice === "fuse"/);
  assert.match(fusion, /api\.multiRunChat\(targets/);
  // No new backend surface: the join lives in the local helper.
  assert.match(notifier, /export function fuseRunAnswers/);
  assert.doesNotMatch(fusion, /invoke\(|fuse_run|fuse_answers/);
});
