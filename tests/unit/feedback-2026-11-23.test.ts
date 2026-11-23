// Regressions for Omar's 2026-11-23 dev feedback (v0.2.2 draft): the Send
// button resending the previous message, setup repeating after every
// update, and the extra motion on chat search and Settings.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const read = (path: string) => readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");

test("the Send button sends the text just typed, never the previous message", () => {
  const input = read("src/components/ChatInput.svelte");
  assert.doesNotMatch(input, /onclick=\{onSend\}/, "a bare onSend passes the click event as `resend`");
  assert.match(input, /onclick=\{\(\) => onSend\(\)\}/, "the button calls onSend with no argument");

  const chat = read("src/screens/ChatScreen.svelte");
  assert.match(chat, /const resend = resendArg === true/, "only an explicit true resends");
});

test("setup does not repeat for online-only users", () => {
  const app = read("src/App.svelte");
  assert.match(app, /cfg\.setup_complete/, "the startup gate reads the durable flag");
  assert.match(app, /cfg\.has_online_providers/, "saved online providers also skip setup");
  assert.match(app, /markSetupComplete/, "finishing or skipping setup records it");
});

test("chat search opens without motion and Settings sections switch without a fly-in", () => {
  const search = read("src/components/ChatSearchModal.svelte");
  assert.match(search, /motion=\{false\}/, "chat search opens instantly");
  const settings = read("src/screens/SettingsScreen.svelte");
  assert.doesNotMatch(settings, /\{#key activeTab\}/, "sections no longer remount on every switch");
  assert.doesNotMatch(settings, /in:fly=\{enterScreen\(\)\}/, "sections no longer fly in on every switch");
});

test("queued messages can be edited in place and reordered", () => {
  const chat = read("src/screens/ChatScreen.svelte");
  assert.match(chat, /editingNoteId/, "a queued message edits in its own row");
  assert.match(chat, /moveNote\(note\.id, -1\)/, "a queued message moves earlier");
  assert.match(chat, /moveNote\(note\.id, 1\)/, "a queued message moves later");
});
