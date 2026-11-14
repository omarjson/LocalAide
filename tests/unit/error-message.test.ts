// Unit tests for frontend helpers that need no window. Run: npm run test:unit
// Node strips the TypeScript types itself, so no build step is needed.
import assert from "node:assert/strict";
import { test } from "node:test";
import { errorMessage } from "../../src/lib/api/client.ts";

test("backend errors show their message, not [object Object]", () => {
  const appError = { code: "INVALID_INPUT", message: "Give the provider a name.", detail: null, retryable: false };
  assert.equal(errorMessage(appError), "Give the provider a name.");
});

test("error codes in text are removed", () => {
  assert.equal(errorMessage("[MODEL_LOAD_FAILED] The model did not load."), "The model did not load.");
  assert.equal(errorMessage(new Error("[INTERNAL] Disk full")), "Disk full");
});

test("technical codes get plain words", () => {
  assert.equal(
    errorMessage({ code: "DB_BUSY", message: "Database is currently busy." }),
    "LocalAide is busy. Try again in a moment.",
  );
});

test("cancel messages still say cancel, so screens can detect them", () => {
  assert.match(errorMessage({ code: "CANCELLED", message: "The operation was cancelled." }), /cancel/i);
});

test("unknown values fall back to a plain sentence", () => {
  for (const value of [undefined, null, 42, {}, "", { message: "   " }]) {
    assert.equal(errorMessage(value), "Something went wrong. Try again.");
  }
});
