import assert from "node:assert/strict";
import { test } from "node:test";
import { parseInline, parseMarkdown } from "../../src/lib/markdown.ts";

test("headings, paragraphs and lists become blocks", () => {
  const blocks = parseMarkdown("# Title\n\nOne line\nand more.\n\n- a\n- b\n\n1. first\n2. second\n");
  assert.deepEqual(
    blocks.map((b) => b.kind),
    ["heading", "paragraph", "list", "list"],
  );
  assert.deepEqual(blocks[1], { kind: "paragraph", text: [{ kind: "text", text: "One line and more." }] });
  assert.equal(blocks[2].kind === "list" && blocks[2].ordered, false);
  assert.equal(blocks[3].kind === "list" && blocks[3].ordered, true);
});

test("tables keep header and rows", () => {
  const blocks = parseMarkdown("| What | Where |\n|---|---|\n| Chats | `~/.aiassistant` |\n");
  assert.equal(blocks.length, 1);
  const table = blocks[0];
  assert.ok(table.kind === "table");
  assert.equal(table.header.length, 2);
  assert.deepEqual(table.rows[0][1], [{ kind: "code", text: "~/.aiassistant" }]);
});

test("inline links, bold and code", () => {
  assert.deepEqual(parseInline("see **this** and [docs](https://example.com)."), [
    { kind: "text", text: "see " },
    { kind: "bold", text: "this" },
    { kind: "text", text: " and " },
    { kind: "link", text: "docs", href: "https://example.com" },
    { kind: "text", text: "." },
  ]);
});

test("bare links drop trailing punctuation", () => {
  assert.deepEqual(parseInline("Go to https://github.com/x/y."), [
    { kind: "text", text: "Go to " },
    { kind: "link", text: "https://github.com/x/y", href: "https://github.com/x/y" },
    { kind: "text", text: "." },
  ]);
});

test("html stays text", () => {
  assert.deepEqual(parseInline("<script>alert(1)</script>"), [{ kind: "text", text: "<script>alert(1)</script>" }]);
});
