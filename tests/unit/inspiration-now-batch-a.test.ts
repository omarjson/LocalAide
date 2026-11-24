// Unit tests for the NOW items from the inspiration backlog, Batch A
// (N6, N7, N12–N18). Run: npm run test:unit
// Node strips the TypeScript types itself, so no build step is needed.
import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

import { fitBadge, recommendedModel, FIT_LABELS } from "../../src/lib/hardware.ts";
import { greetingAt, workedToday } from "../../src/lib/greeting.ts";
import { resolveNewChatModel } from "../../src/lib/lastModel.ts";
import { classifyError, describeError, errorMessage, ERROR_FIXES, type ErrorCause } from "../../src/lib/api/errors.ts";
import { windowCoverage } from "../../src/lib/usage.ts";
import { orderOptionsRecommended } from "../../src/lib/questions.ts";
import { COMPOSER_FOLD_ORDER, foldedLabels, MAX_COMPOSER_FOLD, nextFoldLevel } from "../../src/lib/composerFit.ts";
import type { UsageSummary } from "../../src/lib/api/bindings.ts";

const root = new URL("../..", import.meta.url).pathname;
const read = (p: string) => readFileSync(join(root, p), "utf8");

// N6 — hardware-aware fit. Heuristic: runs well inside the model budget,
// slowly when it fits in free RAM but not the budget, too large when it
// does not fit in free RAM at all.
test("fit badges follow the documented heuristic", () => {
  assert.equal(fitBadge(4, 8, 12), "runs-well");
  assert.equal(fitBadge(8, 8, 12), "runs-well");
  assert.equal(fitBadge(10, 8, 12), "slow");
  assert.equal(fitBadge(13, 8, 12), "too-large");
  // No budget known (probe failed): only free RAM decides, never "slowly"
  // on a guess.
  assert.equal(fitBadge(10, 0, 12), "runs-well");
  assert.equal(fitBadge(13, 0, 12), "too-large");
});

test("every fit has a badge label", () => {
  assert.deepEqual(Object.keys(FIT_LABELS).sort(), ["runs-well", "slow", "too-large"]);
  for (const label of Object.values(FIT_LABELS)) assert.ok(label.length > 0);
});

test("the recommendation is the most capable model that runs well", () => {
  const probe = { budgetGb: 8, availableGb: 12 };
  const models = [
    { id: "tiny", kind: "chat", ramGb: 2, installed: true },
    { id: "mid", kind: "chat", ramGb: 6, installed: true },
    { id: "big", kind: "chat", ramGb: 7.5, installed: false },
    { id: "huge", kind: "chat", ramGb: 11, installed: false },
    { id: "embed", kind: "embedding", ramGb: 1, installed: true },
  ];
  assert.equal(recommendedModel(models, probe)?.id, "big");
  // Nothing runs well: the smallest slow model, not the biggest.
  assert.equal(recommendedModel(models, { budgetGb: 1, availableGb: 12 })?.id, "tiny");
  // Nothing fits at all: no recommendation, not a wrong one.
  assert.equal(recommendedModel(models, { budgetGb: 1, availableGb: 1 }), null);
  assert.equal(recommendedModel([], probe), null);
});

test("the Models page shows hardware and one recommendation, fail-open", () => {
  const screen = read("src/screens/ModelManagerScreen.svelte");
  assert.match(screen, /This computer/);
  assert.match(screen, /probe\.cpu_brand/);
  assert.match(screen, /probe\.gpu_vendor \?\? "Not detected"/);
  assert.match(screen, /Recommended for you/);
  assert.match(screen, /recommendedModel\(/);
  assert.match(screen, /recommendedOne/);
  assert.doesNotMatch(screen, /recommended\.length/);
  // A missing probe hides the card and the badges, it never errors.
  assert.match(screen, /\{#if probe\}/);
  assert.match(screen, /fitOf\(model\)/);
});

test("the memory chip under the composer shows GB in use with Load/Unload", () => {
  const input = read("src/components/ChatInput.svelte");
  assert.match(input, /memory\?: \{ usedGb: number; totalGb: number; loaded: boolean; modelName\?: string \| null \} \| null/);
  assert.match(input, /\{memory\.usedGb\.toFixed\(1\)\} \/ \{memory\.totalGb\.toFixed\(1\)\} GB/);
  assert.match(input, />Unload<\/button>/);
  assert.match(input, />Load<\/button>/);
  const chat = read("src/screens/ChatScreen.svelte");
  assert.match(chat, /memory=\{activeChoice\?\.local && localRam/);
  assert.match(chat, /onUnloadModel=\{activeChoice\?\.local && engineLoaded/);
});

// N7 — typed empty states + honesty rules.
test("no error ever reads as a generic 'something went wrong'", () => {
  const errors = read("src/lib/api/errors.ts");
  assert.doesNotMatch(errors, /Something went wrong/);
  assert.equal(errorMessage(undefined), "Unknown error — LocalAide couldn't tell what went wrong. Try again.");
});

test("every cause class has exactly one fixing action", () => {
  const causes: ErrorCause[] = ["network", "not-found", "permission", "auth", "unavailable", "invalid", "cancelled", "unknown"];
  assert.deepEqual(Object.keys(ERROR_FIXES).sort(), [...causes].sort());
  for (const cause of causes) assert.match(ERROR_FIXES[cause], /[A-Z]/, `${cause} has a fix`);
});

test("errors are classified by cause, with the one fix", () => {
  assert.equal(classifyError({ code: "CANCELLED", message: "stopped" }), "cancelled");
  assert.equal(classifyError(new Error("fetch failed")), "network");
  assert.equal(classifyError({ code: "NOT_FOUND", message: "no such model" }), "not-found");
  assert.equal(classifyError({ code: "INVALID_INPUT", message: "bad value" }), "invalid");
  assert.equal(classifyError("permission denied"), "permission");
  assert.equal(classifyError({ code: "AUTH", message: "invalid api key" }), "auth");
  assert.equal(classifyError("the model is not loaded"), "unavailable");
  assert.equal(classifyError({}), "unknown");
  const described = describeError(new Error("fetch failed"));
  assert.equal(described.cause, "network");
  assert.equal(described.fix, ERROR_FIXES.network);
  assert.ok(described.message.length > 0);
});

test("the error banner shows the fix line", () => {
  const banner = read("src/components/ErrorBanner.svelte");
  assert.match(banner, /fix\?: string/);
  assert.match(banner, /\{#if fix\}/);
  const usage = read("src/screens/UsageScreen.svelte");
  assert.match(usage, /describeError\(e\)/);
  assert.match(usage, /fix=\{errorFix/);
});

const summaryWith = (days: number, perDay: string[]): UsageSummary =>
  ({
    days,
    answers: perDay.length,
    cancelled: 0,
    input_tokens: 0,
    output_tokens: 0,
    cache_tokens: 0,
    ms: 0,
    tool_calls: 0,
    sessions: 1,
    active_days: perDay.length,
    longest_streak: 1,
    per_day: perDay.map((date) => ({ date, answers: 1, input: 0, output: 0 })),
    per_model: [],
    per_folder: [],
  }) as UsageSummary;

test("usage totals name the window they actually cover", () => {
  const today = new Date(2026, 10, 24);
  // 30 days asked, history only from Nov 20: the totals say so.
  const short = windowCoverage(summaryWith(30, ["2026-11-20", "2026-11-24"]), today);
  assert.ok(short?.short);
  // "Nov 20" in the user's locale — day and month both named, either order.
  assert.match(short?.from ?? "", /20/);
  assert.match(short?.from ?? "", /Nov/);
  assert.match(short?.to ?? "", /24/);
  // History reaches the window start: no caveat.
  const full = windowCoverage(summaryWith(30, ["2026-10-26", "2026-11-24"]), today);
  assert.equal(full?.short, false);
  // All time claims no window.
  assert.equal(windowCoverage(summaryWith(0, ["2026-11-24"]), today), null);
  const usage = read("src/screens/UsageScreen.svelte");
  assert.match(usage, /Totals cover \{coverage\.from\} – \{coverage\.to\}\. Nothing is recorded before \{coverage\.from\}\./);
  assert.match(usage, /Cost leaves out \{unpricedCount\}/);
});

// N12 — interactive-question conventions.
test("recommended options stand first, the rest keep the model's order", () => {
  const options = [
    { label: "A", recommended: false },
    { label: "B", recommended: true },
    { label: "C" },
  ];
  assert.deepEqual(orderOptionsRecommended(options).map((o) => o.label), ["B", "A", "C"]);
  assert.deepEqual(orderOptionsRecommended([{ label: "A" }, { label: "B" }]).map((o) => o.label), ["A", "B"]);
  assert.deepEqual(orderOptionsRecommended([]), []);
});

test("question UIs mark the recommended option and offer a free-text answer", () => {
  for (const file of ["src/components/OptionButtons.svelte", "src/components/QuestionDock.svelte", "src/components/A2uiNode.svelte"]) {
    const src = read(file);
    assert.match(src, /orderOptionsRecommended\(/, `${file} sorts recommended first`);
    assert.match(src, /Recommended/, `${file} marks it`);
  }
  const card = read("src/components/QuestionCard.svelte");
  assert.match(card, /recommended\?: string \| null/);
  assert.match(card, /allowOther=\{active\}/, "the free-text Other… is always offered while live");
  const dock = read("src/components/QuestionDock.svelte");
  assert.match(dock, /Other<\/span>/, "the dock keeps its typed-answer box");
  // The model can mark a recommendation through the ask_user tool.
  // The tool schemas moved to tools/specs.rs in the 0.2.4 split.
  const tools = read("src-tauri/src/services/agent/tools/specs.rs");
  assert.match(tools, /"recommended": \{"type": "boolean"/);
  const agent = read("src-tauri/src/services/agent/handlers.rs");
  assert.match(agent, /recommended: o\s*\.get\("recommended"\)/);
  assert.match(agent, /recommended_option_is_carried_not_invented/);
});

// N13 — composer bar-fit.
test("composer pills fold in a fixed order and never cover Send", () => {
  assert.deepEqual([...COMPOSER_FOLD_ORDER], ["tools", "model", "effort"]);
  assert.equal(MAX_COMPOSER_FOLD, 3);
  assert.deepEqual(foldedLabels(0), []);
  assert.deepEqual(foldedLabels(1), ["tools"]);
  assert.deepEqual(foldedLabels(2), ["tools", "model"]);
  assert.deepEqual(foldedLabels(3), ["tools", "model", "effort"]);
  assert.deepEqual(foldedLabels(99), ["tools", "model", "effort"]);
  assert.equal(nextFoldLevel(true, 0), 1);
  assert.equal(nextFoldLevel(true, 3), 3);
  assert.equal(nextFoldLevel(false, 2), 2);
  const css = read("src/main.css");
  assert.match(css, /\.composer\[data-fold="1"\] \[data-fold-label="tools"\]/);
  assert.match(css, /\.composer\[data-fold="3"\] \[data-fold-label="effort"\]/);
  const input = read("src/components/ChatInput.svelte");
  assert.match(input, /data-fold=\{fold\}/);
  assert.match(input, /data-fold-label="tools"/);
  assert.match(input, /new ResizeObserver\(\(\) => void fitRow\(\)\)/);
  const picker = read("src/components/ModelPicker.svelte");
  assert.match(picker, /data-fold-label="model"/);
  assert.match(picker, /data-fold-label="effort"/);
  assert.match(picker, /aria-label=\{`Model: \$\{chipLabel\}/, "a folded model pill keeps its name");
});

// N14 — new chats start on the last-used model, with an honest fallback.
test("new chats start on the last-used model when it is still usable", () => {
  const models = [
    { key: "local:a", local: true, active: true },
    { key: "local:b", local: true, active: false },
    { key: "prov:x", local: false, active: false },
  ];
  assert.deepEqual(resolveNewChatModel("local:b", models, true), { key: "local:b", fellBack: false });
  assert.deepEqual(resolveNewChatModel("prov:x", models, true), { key: "prov:x", fellBack: false });
  // An unloaded local model is not usable: fall back, and say so.
  assert.deepEqual(resolveNewChatModel("local:b", models, false), { key: "local:a", fellBack: true });
  // A removed model is not usable: fall back, and say so.
  assert.deepEqual(resolveNewChatModel("gone", models, true), { key: "local:a", fellBack: true });
  // No last-used model: the active model, with no fallback line.
  assert.deepEqual(resolveNewChatModel(null, models, true), { key: "local:a", fellBack: false });
  const chat = read("src/screens/ChatScreen.svelte");
  assert.match(chat, /saveLastUsedModel\(choice\.key\)/);
  assert.match(chat, /resolveNewChatModel\(lastUsedKey, choices\.models, choices\.local_loaded\)/);
  assert.match(chat, /Your last model isn't available, so this chat starts on/);
});

// N15 — header manners.
test("the chat title is a button, renames on double-click, and shows whole on hover", () => {
  const header = read("src/components/ChatHeader.svelte");
  assert.match(header, /title=\{title === "New Conversation" \? "New chat" : title\}/);
  assert.match(header, /onclick=\{onTitleClick\}/);
  assert.match(header, /ondblclick=\{onTitleDblClick\}/);
  // "New chat" until the model titles it — the Batch 3 behaviour stands.
  assert.match(header, /"New chat" : title/);
  const chat = read("src/screens/ChatScreen.svelte");
  assert.match(chat, /setTimeout\(\(\) => \(showMoreActions = true\), 250\)/);
  const bar = read("src/components/TitleBar.svelte");
  assert.match(bar, /title=\{tab\.title\}/);
});

// N16 — time-of-day greeting that knows whether you worked today.
test("the greeting knows the time of day and whether you worked today", () => {
  const morning = new Date(2026, 10, 24, 8, 0);
  const noon = new Date(2026, 10, 24, 13, 0);
  const evening = new Date(2026, 10, 24, 19, 0);
  const late = new Date(2026, 10, 24, 23, 30);
  assert.equal(greetingAt(morning, false), "Good morning.");
  assert.equal(greetingAt(noon, false), "Good afternoon.");
  assert.equal(greetingAt(evening, false), "Good evening.");
  assert.equal(greetingAt(evening, true), "Good evening. Nice work today.");
  assert.equal(greetingAt(late, true), "It's late — take care of yourself.");
  assert.equal(greetingAt(new Date(2026, 10, 24, 3, 0), false), "Working late?");
  const threads = [{ updated_at: new Date(2026, 10, 24, 10, 0).getTime() / 1000 }];
  assert.equal(workedToday(threads, evening), true);
  assert.equal(workedToday([{ updated_at: new Date(2026, 10, 23, 10, 0).getTime() / 1000 }], evening), false);
  assert.equal(workedToday([], evening), false);
  const chat = read("src/screens/ChatScreen.svelte");
  assert.match(chat, /greetingAt\(greetingNow, workedToday\(chats\.threads, greetingNow\)\)/);
  const empty = read("src/components/EmptyState.svelte");
  assert.match(empty, /greeting\?: string/);
});

// N17 — Tauri shell rules.
test("the toast is portaled so reporting costs the page no layout", () => {
  const app = read("src/App.svelte");
  assert.match(app, /import \{ portal \} from "\.\/lib\/portal"/);
  assert.match(app, /<div use:portal class="fixed bottom-10 left-1\/2 z-50 -translate-x-1\/2"/);
});

test("status is Offline by default, never a default Online", () => {
  const header = read("src/components/ChatHeader.svelte");
  // The header chip only says Online when an online model is actually
  // chosen; the fallback branch is Offline on this computer.
  assert.match(header, /Offline · this computer/);
  assert.match(header, /\{#if onlineLabel\}/);
  // No autostart feature exists in the app at all, so the one-time marker
  // rule has nothing to attach to (see the evidence doc, §11).
  const walkRs = (dir: string): string[] => {
    const out: string[] = [];
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const p = join(dir, entry.name);
      if (entry.isDirectory()) out.push(...walkRs(p));
      else if (p.endsWith(".rs")) out.push(readFileSync(p, "utf8"));
    }
    return out;
  };
  const backend = walkRs(join(root, "src-tauri", "src")).join("\n");
  assert.doesNotMatch(backend, /autostart/i);
  assert.doesNotMatch(read("src-tauri/tauri.conf.json"), /autostart/i);
  assert.doesNotMatch(read("src-tauri/Cargo.toml"), /autostart/i);
});

// N18 — technical text is never translated.
test("no Arabic-script text in the UI source; technical text stays English", () => {
  const arabic = /[\u0600-\u06FF\u0750-\u077F\u08A0-\u08FF\uFB50-\uFDFF\uFE70-\uFEFF]/;
  const walk = (dir: string): string[] => {
    const out: string[] = [];
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const p = join(dir, entry.name);
      if (entry.isDirectory()) out.push(...walk(p));
      else if (p.endsWith(".svelte") || p.endsWith(".ts")) out.push(p);
    }
    return out;
  };
  const offenders = walk(join(root, "src")).filter((p) => arabic.test(readFileSync(p, "utf8")));
  assert.deepEqual(offenders, [], "Arabic script in UI source — technical text (model ids, commands, paths, URLs) is never translated");
});
