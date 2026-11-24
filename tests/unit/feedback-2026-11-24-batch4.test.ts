// Regressions for Omar's 2026-11-24 screenshot feedback (v0.2.2 draft):
// a settings section that opened scrolled past its content, settings rows
// whose labels wrapped a word a line, answer stats that showed with the
// switch off, a context window with no cache hit rate, and subagents with
// no token count and no way to see what they were asked and did.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { addSubagentStep } from "../../src/lib/chatParts.ts";
import type { MessagePart } from "../../src/lib/api/bindings.ts";

const read = (path: string) => readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");

type ToolPart = Extract<MessagePart, { type: "tool" }>;
const subagent = (): ToolPart => ({
  type: "tool",
  id: "a1",
  name: "run_subagent",
  title: "Subagent",
  detail: "Black Friday deals",
  group: "subagent",
  arguments: "{}",
  status: "running",
  summary: "",
  output: "",
});

test("switching settings sections starts at the top", () => {
  const settings = read("src/screens/SettingsScreen.svelte");
  assert.match(settings, /onSectionChange\?: \(\) => void/, "the screen takes a section-change hook");
  assert.match(settings, /onSectionChange\?\.\(\)/, "changing sections calls the hook");
  const app = read("src/App.svelte");
  assert.match(app, /<SettingsScreen onSectionChange=\{\(\) => \{ if \(canvasEl\) canvasEl\.scrollTop = 0; \}\} \/>/, "the app scrolls the canvas to the top on a section change");
});

test("settings rows lay out by their own column, not the viewport", () => {
  const settings = read("src/screens/SettingsScreen.svelte");
  assert.match(settings, /<div class="min-w-0 @container">/, "the settings content column is a container");
  const row = read("src/components/SettingsRow.svelte");
  assert.match(row, /@md:flex-row/, "the row goes side by side only when its column is wide enough");
  assert.match(row, /@md:ml-auto/, "the control sits at the end of a wide row");
  assert.doesNotMatch(row, /sm:flex-row/, "no viewport breakpoint decides the row");
  assert.doesNotMatch(row, /sm:ml-auto/, "no viewport breakpoint places the control");
});

test("the answer-statistics switch is the control, not the developer flag", () => {
  const chat = read("src/screens/ChatScreen.svelte");
  assert.match(chat, /const showStats = \$derived\(answerStats\)/, "the switch alone decides whether stats show");
  assert.doesNotMatch(chat, /answerStats \|\| flag\("answer_stats"\)/, "the flag no longer forces stats on");
  assert.match(chat, /if \(localStorage\.getItem\(STATS_KEY\) === null\) answerStats = flag\("answer_stats"\)/, "the flag only seeds the default until the user picks");
});

test("the context window shows a cache hit rate, or says it has none", () => {
  const meter = read("src/components/ContextMeter.svelte");
  assert.match(meter, /cacheRateAverage\?: number \| null/, "the meter takes the ledger average too");
  assert.match(meter, /Cache hit rate/, "the row is labelled as a cache hit rate");
  assert.match(meter, /\(recent average\)/, "an average is marked as one, not passed off as the last answer");
  assert.match(meter, /This model does not report cached tokens\./, "a model that reports nothing says so instead of hiding the row");
  const chat = read("src/screens/ChatScreen.svelte");
  assert.match(chat, /const refreshLedgerCacheRate = async \(\) => \{/, "the chat loads the ledger average");
  assert.match(chat, /await api\.usageSummary\(\)/, "the average comes from the usage log");
  assert.match(chat, /cacheRateAverage=\{ledgerCacheRate\}/, "the average reaches the meter");
});

test("a subagent step carries its token total, and it sticks", () => {
  const parts: MessagePart[] = [subagent()];
  addSubagentStep(parts, "a1", "Searching the web");
  assert.equal((parts[0] as ToolPart).tokens, undefined, "no total before the run finishes");
  addSubagentStep(parts, "a1", "Done", 4606);
  assert.equal((parts[0] as ToolPart).tokens, 4606, "the total lands with the final step");
  assert.deepEqual((parts[0] as ToolPart).steps, ["Searching the web", "Done"], "the step log ends with Done");
  addSubagentStep(parts, "a1", "Done", 4606);
  assert.equal((parts[0] as ToolPart).steps?.length, 2, "a repeated final step does not duplicate");
  addSubagentStep(parts, "a1", "Done", 5000);
  assert.equal((parts[0] as ToolPart).tokens, 5000, "a newer total still updates the part");

  const events = read("src-tauri/src/app/events.rs");
  assert.match(events, /tokens: Option<usize>/, "the step event carries a token total");
  const agent = read("src-tauri/src/services/agent/subagent.rs");
  assert.match(agent, /let used = done\.prompt_tokens\.unwrap_or\(0\) \+ done\.tokens;/, "the total is prompt plus answer tokens");
  assert.match(agent, /tokens: Some\(used\)/, "the finished subagent reports its total");
  const bindings = read("src/lib/api/bindings.ts");
  assert.match(bindings, /subagent_step"; payload: \{ id: string; step: string; tokens\?: number \| null \}/, "the IPC event carries the total");
  assert.match(bindings, /tokens\?: number \| null/, "the saved tool part keeps the total");
  const domain = read("src-tauri/src/domain/chat.rs");
  assert.match(domain, /tokens: Option<usize>/, "the backend keeps the total with the saved chat");
  assert.match(domain, /steps: Option<Vec<String>>/, "the backend keeps the step log with the saved chat");
  const card = read("src/components/ToolCallCard.svelte");
  assert.match(card, /tokens\?: number \| null/, "the card takes the total");
  assert.match(card, /\{tokenLabel\}/, "the card shows the total by the subagent's step");
});

test("a subagent opens a read-only side view, with no way to talk to it", () => {
  const card = read("src/components/ToolCallCard.svelte");
  assert.match(card, /onViewTranscript\?: \(\) => void/, "the card offers the side view");
  assert.match(card, /title="See what this subagent was asked and did — read-only"/, "the button says what it opens");
  const bubble = read("src/components/MessageBubble.svelte");
  assert.match(bubble, /onViewSubagent\?: \(part: Extract<MessagePart, \{ type: "tool" \}>\) => void/, "the bubble passes the part up");
  assert.match(bubble, /onViewTranscript=\{part\.group === "subagent" && onViewSubagent \? \(\) => onViewSubagent\?\.?\(part\) : undefined\}/, "only subagents get the side view");
  const transcript = read("src/components/SubagentTranscript.svelte");
  assert.match(transcript, /Read-only\. Subagents can't be talked to or steered/, "the view says it is read-only");
  assert.match(transcript, /Asked to do/, "the view shows the task");
  assert.match(transcript, /What it did/, "the view shows the step log");
  assert.match(transcript, /Report/, "the view shows the report");
  assert.doesNotMatch(transcript, /textarea|<input/, "there is no composer in the view");
  const chat = read("src/screens/ChatScreen.svelte");
  assert.match(chat, /let transcriptAgent = \$state<ToolPart \| null>\(null\)/, "the chat holds the viewed subagent");
  assert.match(chat, /<SubagentTranscript agent=\{transcriptAgent\} onClose=\{\(\) => \(transcriptAgent = null\)\} \/>/, "the chat renders the side view");
});
