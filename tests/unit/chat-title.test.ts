import assert from "node:assert/strict";
import { test } from "node:test";
import { cleanTitle, isGoodTitle, titleEchoesQuestion } from "../../src/lib/chatTitle.ts";

test("cleanTitle strips quotes and caps the length", () => {
  assert.equal(cleanTitle(null), null);
  assert.equal(cleanTitle("   "), null);
  assert.equal(cleanTitle('"Plan my week"'), "Plan my week");
  assert.equal(cleanTitle("  'Budget'  "), "Budget");
  assert.equal(cleanTitle("No quotes"), "No quotes");
  const long = cleanTitle("x".repeat(200));
  assert.ok(long !== null && long.length <= 80);
});

test("a title that echoes the question is rejected", () => {
  assert.equal(titleEchoesQuestion("hi", "hi"), true);
  assert.equal(titleEchoesQuestion("Hi", "hi"), true);
  assert.equal(titleEchoesQuestion("Plan my week", "plan my week"), true);
  assert.equal(titleEchoesQuestion("Plan my week", "Plan my week\nplease"), true);
  // A short question repeated with a word added is still an echo.
  assert.equal(titleEchoesQuestion("hi there", "hi"), true);
  assert.equal(titleEchoesQuestion("plan my", "plan my week please"), true);
});

test("a rephrased title passes", () => {
  assert.equal(titleEchoesQuestion("Weekly planning", "Help me plan my week."), false);
  assert.equal(titleEchoesQuestion("Budget review", "Can you look at my budget?"), false);
  assert.equal(titleEchoesQuestion("Trip to Rome", "I want to visit Rome in June"), false);
});

test("isGoodTitle rejects empty and echo titles", () => {
  assert.equal(isGoodTitle(null, "hi"), false);
  assert.equal(isGoodTitle("  ", "hi"), false);
  assert.equal(isGoodTitle("hi", "hi"), false);
  assert.equal(isGoodTitle('"hi"', "hi"), false);
  assert.equal(isGoodTitle("Week plan", "Help me plan my week."), true);
});
