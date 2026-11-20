import assert from "node:assert/strict";
import { test } from "node:test";
import type { MessagePart } from "../../src/lib/api/bindings.ts";
import {
  appendReasoning,
  appendText,
  contextLabel,
  finishTool,
  partsForSaving,
  setToolStatus,
  settleParts,
  startTool,
  textOf,
  thinkingLabel,
} from "../../src/lib/chatParts.ts";

const call = (id: string) => ({ id, name: "web_search", title: "Web search", detail: "rust", group: "web", arguments: "{}" });

test("thinking, tools and text stay in the order they came", () => {
  const parts: MessagePart[] = [];
  appendReasoning(parts, "Let me ", 0);
  appendReasoning(parts, "check.", 10);
  startTool(parts, call("c1"), 2000);
  finishTool(parts, { id: "c1", ok: true, summary: "5 results", output: "…" });
  appendReasoning(parts, "Found it.", 2100);
  appendText(parts, "Rust 1.90 ", 3100);
  appendText(parts, "is out [1].", 3200);

  assert.deepEqual(
    parts.map((p) => p.type),
    ["reasoning", "tool", "reasoning", "text"],
  );
  const first = parts[0];
  assert.ok(first.type === "reasoning" && first.text === "Let me check." && first.ms === 2000);
  const second = parts[2];
  assert.ok(second.type === "reasoning" && second.ms === 1000);
  assert.equal(textOf(parts), "Rust 1.90 is out [1].");
  const tool = parts[1];
  assert.ok(tool.type === "tool" && tool.status === "done" && tool.summary === "5 results");
});

test("a denied tool stays denied, and stopped tools settle", () => {
  const parts: MessagePart[] = [];
  startTool(parts, call("w"), 0);
  setToolStatus(parts, "w", "denied");
  finishTool(parts, { id: "w", ok: false, summary: "You said no", output: "" });
  startTool(parts, call("x"), 0);
  appendReasoning(parts, "hmm", 0);
  settleParts(parts, 500);
  const [denied, stopped, thinking] = parts;
  assert.ok(denied.type === "tool" && denied.status === "denied");
  assert.ok(stopped.type === "tool" && stopped.status === "failed" && stopped.summary === "Stopped");
  assert.ok(thinking.type === "reasoning" && thinking.ms === 500);
});

test("saved parts drop timers and long output", () => {
  const parts: MessagePart[] = [];
  appendReasoning(parts, "a", 0);
  startTool(parts, call("c"), 5);
  finishTool(parts, { id: "c", ok: true, summary: "", output: "x".repeat(5000) });
  const saved = partsForSaving(parts);
  assert.deepEqual(saved[0], { type: "reasoning", text: "a", ms: 5 });
  assert.ok(saved[1].type === "tool" && saved[1].output.length === 2000);
});

test("labels read naturally", () => {
  assert.equal(thinkingLabel(null), "Thinking…");
  assert.equal(thinkingLabel(400), "Thought for 1 s");
  assert.equal(thinkingLabel(65_000), "Thought for 1 min 5 s");
  assert.equal(contextLabel(8192), "8K");
  assert.equal(contextLabel(131072), "128K");
  assert.equal(contextLabel(1_048_576), "1M");
  assert.equal(contextLabel(null), "");
});
