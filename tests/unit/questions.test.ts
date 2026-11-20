import assert from "node:assert/strict";
import { test } from "node:test";
import { parseQuestion } from "../../src/lib/questions.ts";

test("a question before a numbered list becomes options", () => {
  const q = parseQuestion("I can do this two ways.\n\nWhich do you prefer?\n1. **Quick fix**\n2. Full rewrite.\n3. `Leave it`");
  assert.deepEqual(q, { question: "Which do you prefer?", options: ["Quick fix", "Full rewrite", "Leave it"], multiple: false });
});

test("a question after a bulleted list also counts", () => {
  const q = parseQuestion("Options:\n- Paris\n- Rome\n\nWhere should we go?");
  assert.equal(q?.question, "Where should we go?");
  assert.deepEqual(q?.options, ["Paris", "Rome"]);
});

test("multiple choice is detected from the wording", () => {
  const q = parseQuestion("Which ones should I include?\n1. Tests\n2. Docs\n3. Changelog");
  assert.equal(q?.multiple, true);
});

test("yes/no questions get Yes and No", () => {
  assert.deepEqual(parseQuestion("The file is ready. Should I save it to the workspace?")?.options, ["Yes", "No"]);
  assert.equal(parseQuestion("Is there anything else I can help with?"), null, "closing pleasantries get no buttons");
  assert.equal(parseQuestion("Do you want the short or the long version?"), null, "an either/or question is not yes/no");
});

test("plain lists and statements get no buttons", () => {
  assert.equal(parseQuestion("Steps:\n1. Open the app\n2. Click Save"), null);
  assert.equal(parseQuestion("All done."), null);
  assert.equal(parseQuestion("Pick one?\n1. A"), null, "one option is not a choice");
  assert.equal(parseQuestion("```\nWhich?\n1. a\n2. b\n```"), null, "code blocks are ignored");
});
