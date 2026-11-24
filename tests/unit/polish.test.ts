import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { capTabs, MAX_TABS, type RecencyTab } from "../../src/lib/tabCap.ts";

const read = (p: string) => readFileSync(new URL(`../../${p}`, import.meta.url), "utf8");

// ---------------------------------------------------------------------------
// Tab cap. The store cannot be imported under plain Node ($state is a Svelte
// macro), so the pure decider in lib/tabCap.ts is tested directly.
// ---------------------------------------------------------------------------

const tab = (key: string, lastUsed: number): RecencyTab => ({ key, lastUsed });
const keys = (tabs: readonly RecencyTab[]) => tabs.map((t) => t.key);

test("capTabs appends while under the limit and keeps the active key", () => {
  const existing = [tab("a", 1), tab("b", 2)];
  const result = capTabs(existing, "c", tab("c", 3));
  assert.deepEqual(keys(result.tabs), ["a", "b", "c"]);
  assert.equal(result.activeKey, "c");
  assert.deepEqual(result.dropped, []);
  assert.deepEqual(keys(existing), ["a", "b"], "input is not mutated");
});

test("capTabs evicts the least-recently-used inactive tab on the ninth open", () => {
  const existing = Array.from({ length: 8 }, (_, i) => tab(`t${i}`, i + 1));
  const incoming = tab("t8", 9);
  const result = capTabs(existing, incoming.key, incoming);
  assert.equal(MAX_TABS, 8);
  assert.equal(result.tabs.length, 8);
  assert.deepEqual(result.dropped, ["t0"], "oldest inactive tab goes");
  assert.ok(result.tabs.some((t) => t.key === "t8"), "new active tab is kept");
  assert.ok(!result.tabs.some((t) => t.key === "t0"), "evicted tab is gone");
});

test("capTabs tracks recency, not creation order", () => {
  // t0 was just focused, so t2 is now the oldest inactive tab.
  const existing = [tab("t0", 8), tab("t1", 2), tab("t2", 3), tab("t3", 4),
    tab("t4", 5), tab("t5", 6), tab("t6", 7), tab("t7", 1)];
  const incoming = tab("t8", 9);
  const result = capTabs(existing, incoming.key, incoming);
  assert.deepEqual(result.dropped, ["t7"]);
  assert.ok(result.tabs.some((t) => t.key === "t0"), "recently focused tab survives");
});

test("capTabs never drops the active tab, even if it is the oldest", () => {
  const existing = Array.from({ length: 9 }, (_, i) => tab(`t${i}`, i + 1));
  const result = capTabs(existing, "t0"); // restoring an over-full session
  assert.equal(result.tabs.length, 8);
  assert.ok(result.tabs.some((t) => t.key === "t0"));
  assert.deepEqual(result.dropped, ["t1"]);
  assert.equal(result.activeKey, "t0");
});

test("capTabs truncates an over-full restore down to the cap", () => {
  const tabs = Array.from({ length: 12 }, (_, i) => tab(`r${i}`, i));
  const result = capTabs(tabs, "r11");
  assert.equal(result.tabs.length, MAX_TABS);
  assert.equal(result.dropped.length, 4);
  assert.ok(result.tabs.some((t) => t.key === "r11"));
});

test("capTabs respects a smaller limit and stops at one tab", () => {
  const r = capTabs([tab("a", 1)], "b", tab("b", 2), 1);
  assert.deepEqual(keys(r.tabs), ["b"]);
  assert.deepEqual(r.dropped, ["a"]);
});

// ---------------------------------------------------------------------------
// Icon registry. The union is derived from the registry keys, so these checks
// guard against silently dropping an icon during the if/else -> registry move.
// ---------------------------------------------------------------------------

const iconSource = read("src/components/Icon.svelte");
const registryBody = iconSource.match(/export const iconPaths = \{([\s\S]*?)\} as const;/)?.[1] ?? "";
const registryKeys = [...registryBody.matchAll(/^\s*"?([a-z0-9_-]+)"?:\s*`/gm)].map((m) => m[1]);

// The complete set of names shipped before the conversion (git base commit).
const LEGACY_NAMES: string[] = [
  "chat", "search", "memory", "skills", "models", "settings", "usage", "pin", "close",
  "menu", "alert", "lock", "shield", "check", "arrow-down", "plus", "file", "file-plus",
  "folder", "folder-plus", "more", "sliders", "refresh", "download", "trash", "info",
  "sun", "computer", "cloud", "database", "globe", "chevron-right", "chevron-down",
  "flame", "heart", "clock", "automations", "external", "scale", "key", "sparkle",
  "sidebar", "arrow-up", "stop", "play", "copy", "pencil", "image", "bolt", "unlock", "code",
  "flask", "plug", "eye", "layers", "target", "brain", "wrench", "paperclip", "star",
  "star-filled", "x-circle", "window", "terminal", "volume", "mic", "link", "lightbulb",
  "hand", "shield-check", "users", "list-check", "undo", "help", "chart", "folder-open",
  "wand", "compare", "calendar", "puzzle", "route", "shield_check", "keychain",
  "stethoscope", "panel", "history", "merge",
];

test("icon registry keeps every legacy IconName, exactly once", () => {
  assert.equal(registryKeys.length, LEGACY_NAMES.length);
  for (const name of LEGACY_NAMES) {
    assert.equal(registryKeys.filter((k) => k === name).length, 1, `registry has exactly one ${name}`);
  }
  assert.deepEqual([...registryKeys].sort(), [...LEGACY_NAMES].sort());
});

test("icon registry entries are non-empty SVG fragments", () => {
  const entries = [...registryBody.matchAll(/"?([a-z0-9_-]+)"?:\s*`([\s\S]*?)`/g)];
  assert.equal(entries.length, LEGACY_NAMES.length);
  for (const [, name, fragment] of entries) {
    assert.match(name, /^[a-z0-9_-]+$/);
    assert.ok(fragment.includes("<"), `${name} must contain SVG markup`);
    assert.ok(/<(path|circle|rect|ellipse|line|polygon|polyline)\b/.test(fragment), `${name} draws a shape`);
  }
  // Filled variants must keep their explicit fill so they stay visually equal.
  assert.match(registryBody, /"star-filled":[\s\S]*?fill="currentColor"/);
  assert.match(registryBody, /stop:[\s\S]*?stroke="none"/);
});

test("Icon renders one svg from the registry and keeps its a11y contract", () => {
  assert.match(iconSource, /export type IconName = keyof typeof iconPaths;/);
  assert.equal((iconSource.match(/^<svg\b/gm) ?? []).length, 1, "a single <svg> is rendered");
  assert.match(iconSource, /\{@html path\}/);
  assert.match(iconSource, /aria-hidden="true"/);
  assert.match(iconSource, /focusable="false"/);
  assert.match(iconSource, /stroke-width=\{strokeWidth\}/);
  assert.match(iconSource, /strokeWidth\?: number \| string/);
  assert.match(iconSource, /size\?: number \| string/);
});

// ---------------------------------------------------------------------------
// Motion / touch polish (source-level gates, since no build step is available).
// ---------------------------------------------------------------------------

test("a shared surface-enter preset is applied to two panels", () => {
  const css = read("src/main.css");
  assert.match(css, /\.surface-enter\s*,/);
  assert.match(css, /\.surface-raised\s*,/);
  assert.match(css, /\.tab-strip\s*\{/);
  assert.match(css, /@keyframes surface-in/);
  assert.match(css, /animation:\s*surface-in var\(--dur-base\)/);
  assert.match(read("src/components/TitleBar.svelte"), /surface-enter tab-strip/);
});

test("animation durations come from motion tokens", () => {
  const css = read("src/main.css");
  for (const hardcoded of ["skeleton-shimmer 1.4s", "progress-slide 1.2s", "nav-pop 320ms", "transform 120ms"]) {
    assert.ok(!css.includes(hardcoded), `${hardcoded} should use a --dur-* token`);
  }
  const bar = read("src/components/TitleBar.svelte");
  assert.ok(!bar.includes("tab-pulse 1.4s"));
  assert.ok(!bar.includes("background-color 120ms"));
});

test("tab close affordance is visible on touch", () => {
  const bar = read("src/components/TitleBar.svelte");
  assert.ok(!bar.includes("opacity-0 transition"), "no hover-only opacity-0 close button");
  assert.match(bar, /tab-close[\s\S]*?opacity-40/);
  assert.match(bar, /@media \(pointer: coarse\), \(hover: none\)/);
  assert.match(bar, /\.tab-close\s*\{\s*opacity:\s*1/);
});
