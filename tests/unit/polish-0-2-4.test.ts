// 0.2.4 polish (.kiro/specs/polish-0-2-4): approvals show the mode, plan
// and terminal countdowns, the Settings mode switch, and the setup rework.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import {
  APPROVAL_MODE_EVENT,
  CHAT_OPTIONS_KEY,
  onSharedApprovalMode,
  PERMISSION_MODES,
  permissionLabel,
  readSavedApprovalMode,
  shareApprovalMode,
} from "../../src/lib/chatModes.ts";
import {
  clampTimer,
  DEFAULT_PLAN_SECS,
  DEFAULT_TERMINAL_SECS,
  describeTimer,
  formatCountdown,
  PLAN_TIMER_CHOICES,
  TERMINAL_TIMER_CHOICES,
} from "../../src/lib/countdown.ts";

const read = (p: string) => readFileSync(new URL(`../../${p}`, import.meta.url), "utf8");

const fakeStorage = (initial: Record<string, string> = {}) => {
  const data = new Map(Object.entries(initial));
  return {
    getItem: (k: string) => data.get(k) ?? null,
    setItem: (k: string, v: string) => void data.set(k, v),
    data,
  };
};

// ---- R2.5 / R5: one list of permission modes, one saved state ----

test("permission labels come from one list, legacy chat reads as Ask", () => {
  assert.deepEqual(PERMISSION_MODES.map((m) => m.id), ["plan", "ask", "smart", "auto"]);
  assert.equal(permissionLabel("plan"), "Plan mode");
  assert.equal(permissionLabel("smart"), "Autonomous mode");
  assert.equal(permissionLabel("chat"), "Ask before changes");
  const menu = read("src/components/PermissionMenu.svelte");
  assert.match(menu, /import \{ PERMISSION_MODES, permissionInfo \} from "\.\.\/lib\/chatModes"/);
  assert.doesNotMatch(menu, /const PERMISSION_MODES/);
});

test("Settings writes the mode into the chat options key and tells open chats", () => {
  const storage = fakeStorage({ [CHAT_OPTIONS_KEY]: JSON.stringify({ approvals: "plan", web: true }) });
  assert.equal(readSavedApprovalMode(storage), "plan");
  const target = new EventTarget();
  const seen: string[] = [];
  const stop = onSharedApprovalMode((m) => seen.push(m), target);
  shareApprovalMode("smart", storage, target);
  const saved = JSON.parse(storage.data.get(CHAT_OPTIONS_KEY) ?? "{}");
  assert.equal(saved.approvals, "smart");
  assert.equal(saved.buildMode, "smart", "Build returns to the mode picked, like setApprovalMode");
  assert.equal(saved.web, true, "other chat switches are kept");
  shareApprovalMode("plan", storage, target);
  assert.equal(JSON.parse(storage.data.get(CHAT_OPTIONS_KEY) ?? "{}").buildMode, "smart", "Plan keeps the build mode");
  stop();
  shareApprovalMode("smart", storage, target);
  assert.deepEqual(seen, ["smart", "plan"]);
  // Junk on the event or in storage is ignored.
  target.dispatchEvent(new CustomEvent(APPROVAL_MODE_EVENT, { detail: "root" }));
  assert.equal(readSavedApprovalMode(fakeStorage({ [CHAT_OPTIONS_KEY]: "{bad" })), null);
  assert.equal(readSavedApprovalMode(null), null);
});

test("every chat follows the Settings mode switch", () => {
  const chat = read("src/screens/ChatScreen.svelte");
  assert.match(chat, /return onSharedApprovalMode\(setApprovalMode\);/);
  assert.match(chat, /const OPTIONS_KEY = CHAT_OPTIONS_KEY;/);
  const settings = read("src/components/AgentApprovalSettings.svelte");
  assert.match(settings, /shareApprovalMode\(value\)/);
  assert.match(settings, /\{ value: "plan", label: "Plan mode" \}/);
  assert.match(settings, /\{ value: "smart", label: "Autonomous mode" \}/);
  assert.match(read("src/screens/SettingsScreen.svelte"), /<AgentApprovalSettings \/>/);
});

// ---- R2: approvals show the mode ----

test("approval prompts show the permission mode the answer was sent in", () => {
  const chat = read("src/screens/ChatScreen.svelte");
  assert.match(chat, /mode: sentInMode,\s+countdown: approvalTimers\.terminal,/);
  const card = read("src/components/ToolCallCard.svelte");
  assert.match(card, /\{#if approval\.mode\}<ModeChip mode=\{approval\.mode\} \/>\{\/if\}/);
  const modal = read("src/components/ToolApprovalModal.svelte");
  assert.match(modal, /\{#if mode\}<div class="mt-2"><ModeChip \{mode\} \/><\/div>\{\/if\}/);
  // Approve and deny actions are unchanged.
  assert.match(modal, /onclick=\{onDeny\}/);
  assert.match(modal, /onclick=\{onApproveOnce\}/);
  assert.match(modal, /onclick=\{onApproveSession\}/);
  assert.match(read("src/components/PlanReadyCard.svelte"), /<ModeChip mode="plan" \/>/);
});

// ---- R3 / R4: timers ----

test("timer choices and defaults match the spec", () => {
  assert.deepEqual(PLAN_TIMER_CHOICES.map((c) => c.seconds), [0, 60, 300]);
  assert.equal(PLAN_TIMER_CHOICES[0].label, "Off");
  assert.deepEqual(TERMINAL_TIMER_CHOICES.map((c) => c.seconds), [0, 30, 60, 120, 300]);
  assert.equal(TERMINAL_TIMER_CHOICES[0].label, "Wait for me");
  assert.equal(DEFAULT_PLAN_SECS, 0);
  assert.equal(DEFAULT_TERMINAL_SECS, 60);
  assert.equal(clampTimer(9999, 60), 600);
  assert.equal(clampTimer(-5, 60), 0);
  assert.equal(clampTimer("x", 60), 60);
  assert.equal(formatCountdown(42), "0:42");
  assert.equal(formatCountdown(300), "5:00");
  assert.equal(formatCountdown(0.2), "0:01", "rounds up so 0:00 means done");
  assert.equal(formatCountdown(-3), "0:00");
  assert.equal(describeTimer(60, "Off"), "1 minute");
  assert.equal(describeTimer(300, "Off"), "5 minutes");
  assert.equal(describeTimer(30, "Off"), "30 seconds");
  assert.equal(describeTimer(0, "Off"), "Off");
});

test("config defaults: plan auto-accept off, terminal countdown 60 s, clamped on save", () => {
  const config = read("src-tauri/src/config.rs");
  assert.match(config, /#\[serde\(default\)\]\s+pub plan_auto_accept_secs: u32,/);
  assert.match(config, /#\[serde\(default = "default_terminal_approval_secs"\)\]\s+pub terminal_approval_secs: u32,/);
  assert.match(config, /fn default_terminal_approval_secs\(\) -> u32 \{\s+60\s+\}/);
  const cmd = read("src-tauri/src/commands/settings.rs");
  assert.match(cmd, /timers\.plan_auto_accept_secs\.min\(MAX_APPROVAL_TIMER_SECS\)/);
  assert.match(cmd, /timers\.terminal_approval_secs\.min\(MAX_APPROVAL_TIMER_SECS\)/);
  const lib = read("src-tauri/src/lib.rs");
  assert.match(lib, /commands::settings_approval_timers,/);
  assert.match(lib, /commands::settings_set_approval_timers,/);
});

test("the terminal countdown runs the command through the Run decision, shell only", () => {
  const card = read("src/components/ToolCallCard.svelte");
  assert.match(card, /const countdown = \$derived\(isShell && approval\?\.countdown \? approval\.countdown : 0\);/);
  assert.match(card, /onExpire=\{\(\) => onDecide\?\.\("allow"\)\}/);
  assert.match(card, /suffix="unless you choose Don't run\."/);
  // Wait for me keeps today's words.
  assert.match(card, /\{#if !countdown\} <span class="font-normal text-ink-400">Nothing happens until you choose\.<\/span>\{\/if\}/);
  // Held while the chat tab is off screen.
  assert.match(read("src/screens/ChatScreen.svelte"), /approvalsPaused=\{!active\}/);
});

test("the plan countdown builds like the Build button and holds while typing", () => {
  const card = read("src/components/PlanReadyCard.svelte");
  assert.match(card, /\{#if autoAcceptSecs > 0\}/);
  assert.match(card, /onExpire=\{onBuild\}/);
  assert.match(card, /paused=\{typing \|\| !active\}/);
  const composer = read("src/components/ChatComposer.svelte");
  assert.match(composer, /autoAcceptSecs=\{planAutoAcceptSecs\}/);
  assert.match(composer, /typing=\{input\.trim\(\)\.length > 0\}/);
  assert.match(composer, /onBuild=\{onBuildPlan\}/);
  assert.match(read("src/screens/ChatScreen.svelte"), /planAutoAcceptSecs=\{approvalTimers\.plan\}/);
});

test("the countdown fires once, skips paused and hidden time, and animates transform only", () => {
  const c = read("src/components/ApprovalCountdown.svelte");
  assert.match(c, /if \(fired \|\| isPaused\) return;/);
  assert.match(c, /document\.visibilityState === "hidden"/);
  assert.match(c, /fired = true;\s+onExpire\(\);/);
  assert.match(c, /transform: scaleX\(\$\{fraction\}\)/);
  assert.match(c, /aria-live="polite"/);
});

// ---- R1: setup rework, presentation only ----

test("setup keeps every setup call and the step machine (R1.H)", () => {
  const w = read("src/screens/SetupWizard.svelte");
  for (const call of [
    "await api.systemProbe()",
    "api.getModelPresets()",
    "api.listModels()",
    "await api.downloadModelPreset(preset.id, (evt) => {",
    "await api.cancelPresetDownload(selected.id)",
    "wfRepos = await api.hfSearch(wfQuery.trim(), 10)",
    "const files = await api.hfRepoFiles(repo)",
    'const res = await api.hfCustomAdd(repo, file.path, "chat", license, withMmproj)',
    "await api.downloadModel(res.id, (evt) => {",
    "await api.activateModel(res.id)",
    "await api.cancelModelDownload(communityModelId)",
    "await api.setTheme(value)",
    "onMount(detect)",
  ]) {
    assert.ok(w.includes(call), `setup still calls ${call}`);
  }
  assert.match(w, /type Step = "detect" \| "preset" \| "confirm" \| "online" \| "download" \| "done";/);
});

test("setup: Tiny is the one recommended path, plain words, one More options", () => {
  const w = read("src/screens/SetupWizard.svelte");
  assert.match(w, /presets\.find\(\(p\) => p\.id === "tiny"\) \?\? fittingPresets\[fittingPresets\.length - 1\] \?\? presets\[0\]/);
  assert.match(w, /"Too big for this computer" : p\.ramGb <= budgetGb \/ 2 \? "Fits easily" : "Fits"/);
  assert.doesNotMatch(w, /formatBytes\(ramTotal\)|formatBytes\(budget\)|recommended\.ram\b/, "no RAM figures on the default view");
  assert.equal(w.match(/<details/g)?.length, 1, "one disclosure");
  assert.match(w, /<details class="more mt-5" bind:open=\{moreOpen\}>/);
  assert.match(w, /if \(tightMemory\) moreOpen = true;/);
  assert.match(w, /<GhostMascot \{mood\}/);
  assert.match(w, /Start chatting/);
});

test("the mascot never animates an SVG child and only floats while working", () => {
  const g = read("src/components/GhostMascot.svelte");
  assert.match(g, /\.ghost-working \{\s+animation: ghost-float 2\.4s ease-in-out infinite;/);
  assert.equal(g.match(/infinite/g)?.length, 1);
  assert.doesNotMatch(g, /<(path|circle|rect|ellipse)[^>]*class=/, "SVG children carry no animation classes");
  assert.match(g, /prefers-reduced-motion: reduce/);
});
