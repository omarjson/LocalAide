// Unit tests for the NOW items from the inspiration backlog, Batch B
// (N4, N5, N8–N11). Run: npm run test:unit
// Node strips the TypeScript types itself, so no build step is needed.
import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

import { compactionStart, forcedCompactionStart } from "../../src/lib/contextUsage.ts";
import { resolveChoreModel } from "../../src/lib/modelPicker.ts";
import { decideNext, drive } from "../../src/lib/goalRunner.svelte.ts";
import type { Goal, ModelChoices } from "../../src/lib/api/bindings.ts";

const root = new URL("../..", import.meta.url).pathname;
const read = (p: string) => readFileSync(join(root, p), "utf8");

const goal = (over: Partial<Goal> = {}): Goal => ({
  id: "g1",
  thread_id: "t1",
  objective: "ship it",
  budget: 3,
  status: "active",
  created_at: 1,
  ...over,
});

const choices = (over: Partial<ModelChoices> = {}): ModelChoices => ({
  models: [
    {
      key: "local:qwen3-4b",
      provider_id: null,
      provider_label: "This computer",
      model: "qwen3-4b",
      name: "Qwen3 4B",
      local: true,
      reasoning: false,
      vision: false,
      tools: true,
      context: 8192,
      efforts: ["low", "medium", "high"],
      free: true,
      favorite: false,
      active: false,
      size_bytes: null,
    },
    {
      key: "groq:llama-3",
      provider_id: "groq",
      provider_label: "Groq",
      model: "llama-3",
      name: "Llama 3",
      local: false,
      reasoning: false,
      vision: false,
      tools: true,
      context: 8192,
      efforts: [],
      free: true,
      favorite: false,
      active: false,
      size_bytes: null,
    },
  ],
  groups: [],
  active_key: null,
  local_loaded: false,
  online_allowed: true,
  budget_note: null,
  ...over,
});

// N4 — auto-compact at ~95% + anchored summary + pruning.
// (a) The threshold defaults to 95 when nothing was saved.
test("auto-compact defaults to 95% of the window", () => {
  const screen = read("src/screens/ChatScreen.svelte");
  assert.match(screen, /autoCompactAt.*: 95\)/);
  assert.match(screen, /Slider min=\{50\} max=\{95\}/);
});

// (b) An auto-compact asks the loaded local model for an anchored
// summary (goal, decisions, progress, next steps) and the summary rides
// on the system prompt of later sends. A failed summary never breaks
// the send: the cut stands without it.
test("auto-compact summarises the cut part and keeps it in the request", () => {
  const screen = read("src/screens/ChatScreen.svelte");
  assert.match(screen, /requestCompactionSummary/);
  assert.match(screen, /goal, decisions made, progress so far, and next steps/);
  assert.match(screen, /api\s*\n?\s*\.summarize\(\{ text \}\)/);
  assert.match(screen, /requestSystemPrompt/);
  assert.match(screen, /Summary of the earlier part of this conversation/);
  assert.match(screen, /catch\(\(\) => \{\s*\/\/ Summarising is a nicety/);
});

// The manual compact still always cuts, exactly as before.
test("manual compact still always cuts to the last exchange", () => {
  const messages = [
    { role: "user", content: "one", ts: 1 },
    { role: "assistant", content: "two", ts: 2 },
    { role: "user", content: "three", ts: 3 },
    { role: "assistant", content: "four", ts: 4 },
  ] as never;
  assert.equal(forcedCompactionStart(messages), 2);
  // Auto-compact still keeps the newest two messages.
  assert.equal(compactionStart(messages, 10_000_000, 95), 0);
});

// (c) Tool parts travel with the history so the backend can prune them.
test("history sent to the model carries its tool parts", () => {
  const screen = read("src/screens/ChatScreen.svelte");
  assert.match(screen, /parts: m\.parts,/);
  const context = read("src-tauri/src/services/inference/context.rs");
  assert.match(context, /prune_tool_outputs/);
  assert.match(context, /TOOL_OUTPUT_RECENCY/);
  assert.match(context, /TOOL_OUTPUT_PLACEHOLDER/);
});

// N5 — per-task model roles: chores prefer the loaded local model,
// and fall back to current behaviour when none is loaded.
test("chores prefer the loaded local model, for free", () => {
  assert.equal(resolveChoreModel(null), null);
  assert.equal(resolveChoreModel(choices()), null, "nothing loaded: current behaviour");
  const loaded = choices({
    local_loaded: true,
    active_key: "local:qwen3-4b",
    models: choices().models.map((m) =>
      m.provider_id === null ? { ...m, active: true } : m,
    ),
  });
  assert.deepEqual(resolveChoreModel(loaded), { provider_id: null, model: "qwen3-4b" });
  // Loaded but no active flag: the first local model, never an online one.
  const firstLocal = choices({ local_loaded: true });
  assert.deepEqual(resolveChoreModel(firstLocal), { provider_id: null, model: "qwen3-4b" });
  // No local model at all: no chore target, even when loaded is claimed.
  const noLocal = choices({
    local_loaded: true,
    models: choices().models.filter((m) => m.provider_id !== null),
  });
  assert.equal(resolveChoreModel(noLocal), null);
});

test("titles and suggestions run through the chore resolver", () => {
  const screen = read("src/screens/ChatScreen.svelte");
  assert.match(screen, /resolveChoreModel\(choices\)/);
  assert.match(screen, /chore \?\? \(activeChoice/);
});

// N8 — session goals as a state machine: Active / Paused / Blocked /
// Complete, auto-pause on failure.
test("a paused or blocked goal takes no steps until resumed", () => {
  assert.deepEqual(decideNext(goal({ status: "paused" }), [], false), {
    kind: "stop",
    reason: "paused",
  });
  assert.deepEqual(decideNext(goal({ status: "blocked" }), [], false), {
    kind: "stop",
    reason: "blocked",
  });
  assert.equal(drive(goal({ status: "paused" }), ["a"], false).applied, 0);
  assert.equal(drive(goal({ status: "blocked" }), ["a"], false).applied, 0);
});

test("drive auto-pauses on budget exhaustion and blocks when blocked", () => {
  const spent = drive(goal({ budget: 1 }), ["a", "b"], false);
  assert.equal(spent.applied, 1);
  assert.equal(spent.decision.kind, "stop");
  assert.equal(spent.goal.status, "paused");
  const blocked = drive(goal({ budget: 2 }), ["a"], true);
  assert.equal(blocked.applied, 0);
  assert.equal(blocked.goal.status, "blocked");
  // A finished goal keeps its state.
  assert.equal(drive(goal({ status: "completed", budget: 0 }), ["a"], true).goal.status, "completed");
});

test("the goal store has the new states and the panel shows them", () => {
  const rust = read("src-tauri/src/services/goal.rs");
  assert.match(rust, /Paused,/);
  assert.match(rust, /Blocked,/);
  assert.match(rust, /pub fn pause_at/);
  assert.match(rust, /pub fn block_at/);
  assert.match(rust, /pub fn resume_at/);
  assert.match(rust, /Unknown values are treated as active/);
  const panel = read("src/components/GoalPanel.svelte");
  assert.match(panel, /Pause/);
  assert.match(panel, /Resume/);
  assert.match(panel, /Mark blocked/);
  assert.match(panel, /goalPause/);
  assert.match(panel, /goalBlock/);
  assert.match(panel, /goalResume/);
  assert.match(panel, /Paused by you/);
  assert.match(panel, /Paused: a permission was declined/);
  assert.match(panel, /Step budget used up — goal paused/);
});

// N9 — granular per-capability auto-approve, defaulting to read-only.
test("the policy panel offers plain-language capability toggles", () => {
  const panel = read("src/components/PolicyPanel.svelte");
  for (const label of [
    "Read files",
    "Edit files",
    "Safe commands",
    "All commands",
    "Browser",
    "External tools",
  ]) {
    assert.ok(panel.includes(label), `missing toggle: ${label}`);
  }
  assert.match(panel, /Read-only by default/);
  assert.match(panel, /auto_approve_capabilities/);
});

test("capabilities live in config and policy, defaulting to read-only", () => {
  const config = read("src-tauri/src/config.rs");
  assert.match(config, /pub struct CapabilityAutoApprove/);
  assert.match(config, /read_files: true/);
  assert.match(config, /pub auto_approve_capabilities: CapabilityAutoApprove/);
  assert.match(config, /means ask, never auto-approve/);
  const policy = read("src-tauri/src/commands/policy.rs");
  assert.match(policy, /auto_approve_capabilities: CapabilityAutoApprove/);
  const agent = read("src-tauri/src/services/agent/mod.rs");
  assert.match(agent, /capability_auto_approved/);
  assert.match(agent, /auto-approved in Settings/);
});

// N10 — retry with backoff + plain-English errors. Deliberate deviation:
// the per-target cap stays at 3, not 5 — failover is the outer loop and
// the chain already reaches 5+ attempts in total.
test("the per-target retry cap stays at 3 (failover is the outer loop)", () => {
  const stream = read("src-tauri/src/services/inference/openai_stream.rs");
  assert.match(stream, /const SAME_PROVIDER_RETRIES: u32 = 3;/);
  assert.match(stream, /The cap stays at 3/);
});

test("errors speak plainly and network errors retry with backoff", () => {
  const stream = read("src-tauri/src/services/inference/openai_stream.rs");
  assert.match(stream, /is busy right now/);
  assert.match(stream, /is having trouble right now/);
  assert.match(stream, /Couldn't reach \{\} — check your connection/);
  assert.match(stream, /Couldn't reach \{\} — trying again in \{\} s/);
  assert.match(stream, /pub fn backoff_delay/);
  assert.match(stream, /if e\.retryable\s+&& !saw_token\.load\(/);
  assert.match(stream, /retrying mid-answer would show the start twice/);
  // No bare status code in the user-facing message.
  assert.doesNotMatch(stream, /returned \{status\}/);
});

// N11 — "Use Compact Prompt" toggle for local models, default off.
test("the compact prompt is local-only, default off, and shorter", () => {
  const modes = read("src-tauri/src/services/inference/modes.rs");
  assert.match(modes, /compose_compact_system_prompt/);
  assert.match(modes, /compact_instruction/);
  assert.match(modes, /compact_tool_guidance/);
  const config = read("src-tauri/src/config.rs");
  assert.match(config, /pub use_compact_prompt: bool/);
  assert.match(config, /Off by default/);
  const chat = read("src-tauri/src/commands/chat.rs");
  assert.match(chat, /matches!\(target, ChatTarget::Local\) && cfg\.use_compact_prompt/);
  const settings = read("src/screens/SettingsScreen.svelte");
  assert.match(settings, /Use Compact Prompt/);
  assert.match(settings, /getCompactPrompt/);
  assert.match(settings, /setCompactPrompt/);
  assert.match(settings, /Only affects local models/);
});
