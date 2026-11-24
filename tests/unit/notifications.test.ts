// Notification preferences and firing gates, plus the local answer-fusion
// formatter. All pure logic — no Svelte compiler, no Tauri window, no network.
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  DEFAULT_NOTIFICATION_PREFS,
  NOTIFICATION_KEY,
  fireNotification,
  fuseRunAnswers,
  loadNotificationPrefs,
  normalizeNotificationPrefs,
  saveNotificationPrefs,
  shouldNotify,
  type NotificationPrefs,
} from "../../src/lib/notifications.svelte.ts";

/** Swap in an in-memory localStorage for the duration of a test. */
function withStore(fn: () => void): void {
  const original = Object.getOwnPropertyDescriptor(globalThis, "localStorage");
  const mem = new Map<string, string>();
  const fake = {
    getItem: (k: string) => mem.get(k) ?? null,
    setItem: (k: string, v: string) => { mem.set(k, String(v)); },
    removeItem: (k: string) => { mem.delete(k); },
    clear: () => mem.clear(),
    key: (i: number) => [...mem.keys()][i] ?? null,
    get length() { return mem.size; },
  } as unknown as Storage;
  Object.defineProperty(globalThis, "localStorage", { configurable: true, writable: true, value: fake });
  try {
    fn();
  } finally {
    if (original) Object.defineProperty(globalThis, "localStorage", original);
    else Reflect.deleteProperty(globalThis, "localStorage");
  }
}

test("defaults are opt-in: master off, channels on", () => {
  const p = DEFAULT_NOTIFICATION_PREFS;
  assert.equal(p.enabled, false);
  for (const key of ["desktop", "sound", "goal", "turn", "approval", "question"] as const) {
    assert.equal(p[key], true, `${key} should default on`);
  }
});

test("normalizePrefs fills gaps and drops junk, never inventing booleans", () => {
  assert.deepEqual(normalizeNotificationPrefs(null), { ...DEFAULT_NOTIFICATION_PREFS });
  assert.deepEqual(normalizeNotificationPrefs("nope"), { ...DEFAULT_NOTIFICATION_PREFS });
  const p = normalizeNotificationPrefs({
    enabled: true,
    goal: false,
    sound: "yes",
    turn: 1,
    approval: null,
    unknown: true,
  });
  assert.equal(p.enabled, true);
  assert.equal(p.goal, false);
  assert.equal(p.sound, true, "non-boolean values keep the default");
  assert.equal(p.turn, true);
  assert.equal(p.approval, true);
  assert.equal("unknown" in p, false);
});

test("shouldNotify requires the master, the kind and at least one output", () => {
  const base: NotificationPrefs = { ...DEFAULT_NOTIFICATION_PREFS, enabled: true };
  assert.equal(shouldNotify({ ...DEFAULT_NOTIFICATION_PREFS }, "goal"), false, "master off");
  assert.equal(shouldNotify({ ...base, desktop: false, sound: false }, "goal"), false, "no output");
  assert.equal(shouldNotify({ ...base, desktop: true }, "goal"), true);
  assert.equal(shouldNotify({ ...base, sound: false }, "question"), true, "chime alone is enough");
  assert.equal(shouldNotify({ ...base, goal: false }, "goal"), false, "per-kind off");
  assert.equal(shouldNotify({ ...base, approval: false }, "approval"), false);
  assert.equal(shouldNotify({ ...base, turn: false }, "turn"), false);
});

test("preferences round-trip through localStorage under one key", () => {
  withStore(() => {
    const chosen: NotificationPrefs = {
      ...DEFAULT_NOTIFICATION_PREFS,
      enabled: true,
      sound: false,
      question: false,
    };
    saveNotificationPrefs(chosen);
    assert.equal(localStorage.getItem(NOTIFICATION_KEY) !== null, true);
    const loaded = loadNotificationPrefs();
    assert.deepEqual(loaded, {
      ...chosen,
      // Corrupt junk in, sane booleans out.
    });
    assert.equal(loaded.enabled, true);
    assert.equal(loaded.sound, false);
    assert.equal(loaded.question, false);
    assert.equal(loaded.goal, true);
  });
});

test("corrupt stored JSON falls back to defaults instead of throwing", () => {
  withStore(() => {
    localStorage.setItem(NOTIFICATION_KEY, "{not json");
    assert.deepEqual(loadNotificationPrefs(), { ...DEFAULT_NOTIFICATION_PREFS });
  });
});

test("firing is a no-op while opted out, even in a plain Node process", () => {
  withStore(() => {
    assert.equal(fireNotification("goal", "title", "body"), false);
  });
});

test("fusing answers joins labelled blocks with rules and skips failures", () => {
  const fused = fuseRunAnswers([
    { label: "gpt", text: " First answer. " },
    { label: "claude", text: "Second answer." },
    { label: "broken", text: "never shown", error: "timeout" },
    { label: "empty", text: "   " },
  ]);
  assert.match(fused, /^## gpt\n\nFirst answer\./);
  assert.match(fused, /\n\n---\n\n## claude\n\nSecond answer\.$/);
  assert.equal(fused.includes("never shown"), false);
  assert.equal(fused.includes("broken"), false);
  assert.equal(fused.includes("empty"), false);
});

test("fusion handles a single answer, blank labels and no answers", () => {
  assert.equal(fuseRunAnswers([]), "");
  assert.equal(fuseRunAnswers([{ label: "x", text: "   ", error: "nope" }]), "");
  assert.equal(fuseRunAnswers([{ label: "", text: "solo" }]), "## Unnamed target\n\nsolo");
});
