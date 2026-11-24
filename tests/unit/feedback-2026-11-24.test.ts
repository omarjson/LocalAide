// Regressions for Omar's 2026-11-24 chat feedback (v0.2.2 draft): sources
// under the final answer and collapsed, the tab saying "Chat" while the
// header says "New chat", the pulsing stop button with no way back, a manual
// compact that refused when the chat already fit, steered notes that never
// showed in the transcript, progress that did not scroll into view, and the
// removal of skins and hand-typed model prices in favour of OpenRouter.
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";
import type { ChatMessage } from "../../src/lib/api/bindings.ts";
import { forcedCompactionStart } from "../../src/lib/contextUsage.ts";

const read = (path: string) => readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");
const msg = (role: string): ChatMessage => ({ role, content: "x" });

test("an explicit compact always cuts, even when the conversation already fits", () => {
  const five = Array.from({ length: 5 }, (_, i) => msg(i % 2 ? "assistant" : "user"));
  assert.equal(forcedCompactionStart(five), 3, "keeps only the last exchange");
  assert.equal(forcedCompactionStart(five, 2), 3, "never moves the cut backwards");
  assert.equal(forcedCompactionStart(five.slice(0, 2)), 0, "two messages leave nothing to cut");
  assert.equal(forcedCompactionStart([]), 0, "an empty chat leaves nothing to cut");

  const chat = read("src/screens/ChatScreen.svelte");
  assert.match(chat, /compactConversation\(Math\.min\(50, autoCompactAt\), "", true\)/, "the manual compact forces the cut");
  assert.match(chat, /const compactConversation = \(threshold = autoCompactAt, draft = "", force = false\)/, "the auto path still takes the threshold only");
});

test("sources sit under the finished answer, folded until opened", () => {
  const bubble = read("src/components/MessageBubble.svelte");
  assert.match(bubble, /\{#if citations\.length > 0 && !streaming\}/, "no sources mid-stream");
  assert.match(bubble, /let sourcesOpen = \$state\(false\)/, "folded by default");
  assert.match(bubble, /Sources \(\{citations\.length\}\)/, "the toggle shows the count");
  assert.match(bubble, /\{#if sourcesOpen\}/, "the chips open on a tap");
});

test("a new chat's tab says what the header says", () => {
  const tabs = read("src/lib/chats.svelte.ts");
  assert.doesNotMatch(tabs, /tab\.title = "Chat"/, "no rename to Chat once the thread starts");
  assert.match(tabs, /title: thread\.title \|\| "New chat"/, "an untitled thread is a New chat");
});

test("the stop button does not pulse, and stopping offers a resume", () => {
  const input = read("src/components/ChatInput.svelte");
  assert.doesNotMatch(input, /stop-breathe/, "no pulsing ring on the stop button");
  assert.match(input, /canResume\?: boolean/, "the composer knows when a resume is possible");
  assert.match(input, /onResume\?: \(\) => void/, "the composer can ask for a resume");
  assert.match(input, /aria-label="Resume the stopped answer"/, "the resume button is there");
  assert.match(input, /Icon name="play"/, "resume reads as continue, not restart");

  const composer = read("src/components/ChatComposer.svelte");
  assert.match(composer, /canResume=\{run\.stoppedAnswer\}\s+onResume=\{onResume\}/, "the chat wires the resume button");

  const chat = read("src/screens/ChatScreen.svelte");
  assert.match(chat, /stoppedAnswer = true/, "stopping marks the answer resumable");
  assert.match(chat, /void send\(false, undefined, true\)/, "resuming reuses send with the resume flag");
  assert.match(chat, /resume,\n\s+history,/, "the request carries the resume flag");
  assert.match(chat, /const assistantMsg = messages\[assistantIdx\]/, "the running answer is held by object, so a steer inserted before it cannot shift it");

  const bindings = read("src/lib/api/bindings.ts");
  assert.match(bindings, /resume\?: boolean/, "the IPC request carries the resume flag");
  const domain = read("src-tauri/src/domain/chat.rs");
  assert.match(domain, /pub resume: Option<bool>/, "the backend request carries the resume flag");
  const backend = read("src-tauri/src/commands/chat.rs");
  assert.match(backend, /RESUME_MESSAGE/, "the backend tells the model to pick up where it stopped");
  assert.match(backend, /if req\.resume\.unwrap_or\(false\)/, "the resume turn replaces the new message");
});

test("a steered note shows in the chat, in full, and is never sent as history", () => {
  const chat = read("src/screens/ChatScreen.svelte");
  assert.match(chat, /const steerMsg = \{ role: "user", content: text, ts: Date\.now\(\), steer: true \}/, "the steer lands in the transcript");
  assert.match(chat, /messages\.splice\(aiIdx, 0, steerMsg\)/, "it sits before the answer it steers");
  assert.match(chat, /!m\.steer && !m\.content\.startsWith\("\[Chat Error"\)/, "it is not sent back as history");
  assert.match(chat, /steer=\{msg\.steer\}/, "the bubble knows it is a steer");
  assert.match(chat, /onEdit=\{msg\.role === "user" && !msg\.steer/, "a steer cannot be edited and resent as a question");
  assert.match(chat, /const sentMessages = \$derived\(messages\.filter\(\(m\) => !m\.steer\)\)/, "the meter counts only what is sent");

  const bubble = read("src/components/MessageBubble.svelte");
  assert.match(bubble, /Steered this answer/, "the steer is labelled in the transcript");

  const domain = read("src-tauri/src/domain/chat.rs");
  assert.match(domain, /pub steer: Option<bool>/, "the steer flag survives saving the thread");
  const bindings = read("src/lib/api/bindings.ts");
  assert.match(bindings, /steer\?: boolean/, "the frontend knows the steer flag");
});

test("the chat follows progress, not just the last message's text", () => {
  const chat = read("src/screens/ChatScreen.svelte");
  assert.match(chat, /p\.steps\?\.length \?\? 0/, "subagent steps and to-dos move the view");
  assert.match(chat, /scrollToBottom\(\);/, "progress scrolls to the newest line");
});

test("skins are gone", () => {
  assert.ok(!existsSync(new URL("../../src/components/SkinPanel.svelte", import.meta.url)), "no skin panel");
  const settings = read("src/screens/SettingsScreen.svelte");
  assert.doesNotMatch(settings, /SkinPanel/, "Settings no longer shows skins");
  const theme = read("src/lib/theme.ts");
  assert.doesNotMatch(theme, /Skin|skin/, "no skin helpers left in the theme module");
  const app = read("src/App.svelte");
  assert.doesNotMatch(app, /applySkin|activeSkinOverrides/, "no skin is restored at startup");
});

test("model prices come from OpenRouter, not a hand-typed table", () => {
  const settings = read("src/screens/SettingsScreen.svelte");
  assert.doesNotMatch(settings, /Model costs|priceRows|loadPrices/, "Settings no longer asks for prices by hand");
  const usage = read("src/screens/UsageScreen.svelte");
  assert.match(usage, /api\.openrouterModelPrices\(\)/, "the Usage screen prices from OpenRouter");
  const client = read("src/lib/api/client.ts");
  assert.match(client, /openrouterModelPrices/, "the client exposes the OpenRouter prices");
  assert.doesNotMatch(client, /modelPrices\(\)|setModelPrices/, "the manual price calls are gone");
  const cloud = read("src-tauri/src/commands/cloud.rs");
  assert.match(cloud, /pub async fn openrouter_model_prices/, "the backend fetches OpenRouter's list");
  assert.match(cloud, /openrouter\.ai\/api\/v1/, "the list is OpenRouter's public one, no key needed");
  const settingsBackend = read("src-tauri/src/commands/settings.rs");
  assert.doesNotMatch(settingsBackend, /settings_model_prices|settings_set_model_prices/, "the manual price commands are gone");
  const config = read("src-tauri/src/config/mod.rs");
  assert.doesNotMatch(config, /model_prices/, "the manual price table is gone from the config");
  const models = read("src-tauri/src/services/inference/provider_models.rs");
  assert.match(models, /prompt_price_per_million/, "the model list keeps the prices OpenRouter sends");
});
