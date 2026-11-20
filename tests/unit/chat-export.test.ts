import assert from "node:assert/strict";
import { test } from "node:test";
import type { ChatMessage } from "../../src/lib/api/bindings.ts";
import { conversationMarkdown, exportFileName } from "../../src/lib/chatExport.ts";

const chat: ChatMessage[] = [
  {
    role: "user",
    content: "What is new in Rust?",
    attachments: [{ name: "notes.md", kind: "text", text: "x", chars: 1, truncated: true }],
  },
  {
    role: "assistant",
    content: "Rust 1.91 is out [1].",
    model: "Qwen3 4B",
    parts: [
      { type: "reasoning", text: "Check the web.\nThen answer.", ms: 2214.0000000000146 },
      { type: "tool", id: "c1", name: "web_search", title: "Web search", detail: "rust release", group: "web", arguments: '{"query":"rust"}', status: "done", summary: "5 results", output: "…" },
      { type: "tool", id: "c2", name: "write_file", title: "Write a file", detail: "a.md", group: "files", arguments: "{}", status: "denied", summary: "You said no", output: "" },
      { type: "text", text: "Rust 1.91 is out [1]." },
    ],
    citations: [
      { index: 2, source: "C:\\notes\\rust.md", title: "My notes", snippet: "", score: 1 },
      { index: 1, source: "https://blog.rust-lang.org/", title: "Rust Blog", snippet: "", score: 1 },
    ],
  },
  { role: "assistant", content: "I could not answer that.", error: true },
];

test("copy and export include thinking, tools, attachments and ordered references", () => {
  const md = conversationMarkdown(chat, { title: "Rust", systemPrompt: "Be brief." });
  assert.ok(md.startsWith("# Rust\n\n**System prompt:** Be brief."));
  assert.ok(md.includes("> Attached: notes.md (shortened)"));
  assert.ok(md.includes("## LocalAide · Qwen3 4B"));
  assert.ok(md.includes("<summary>Thinking (2 s)</summary>"));
  assert.ok(md.includes("> Check the web.\n> Then answer."));
  assert.ok(md.includes("**Tool: Web search** · rust release · done · 5 results"));
  assert.ok(md.includes('```json\n{"query":"rust"}\n```'));
  assert.ok(md.includes("**Tool: Write a file** · a.md · not allowed"));
  assert.ok(md.indexOf("1. [Rust Blog](https://blog.rust-lang.org/)") < md.indexOf("2. My notes (C:\\notes\\rust.md)"));
  assert.ok(md.includes("## LocalAide (failed)"));
  // The answer text appears once, from its part, not again from content.
  assert.equal(md.split("Rust 1.91 is out [1].").length, 2);
});

test("thinking can be left out, and code fences cannot be broken", () => {
  const md = conversationMarkdown(chat, { title: "", thinking: false });
  assert.ok(md.startsWith("# Chat\n"));
  assert.ok(!md.includes("Thinking"));
  const tricky = conversationMarkdown(
    [{ role: "assistant", content: "", parts: [{ type: "tool", id: "x", name: "t", title: "T", detail: "", group: "mcp", arguments: "a ``` b", status: "done", summary: "", output: "" }] }],
    { title: "x" },
  );
  assert.ok(tricky.includes("````json\na ``` b\n````"));
});

test("file names are safe", () => {
  assert.equal(exportFileName("Plan / my: week?", "md"), "Plan_my_week.md");
  assert.equal(exportFileName("...", "json"), "chat.json");
});
