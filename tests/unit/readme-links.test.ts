import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { test } from "node:test";

// Evidence sweep: every local link in the README must resolve, so claims
// that point at an ADR or an evidence file cannot rot silently.

const readme = new URL("../../README.md", import.meta.url);
const root = dirname(resolve(readme.pathname));
const text = readFileSync(readme, "utf8");

test("README local links resolve", () => {
  const missing: string[] = [];
  for (const match of text.matchAll(/\[[^\]]*\]\(([^)]+)\)/g)) {
    const target = match[1].split("#")[0].trim();
    if (!target || /^(https?:|mailto:)/.test(target)) continue;
    if (!existsSync(resolve(root, decodeURIComponent(target)))) missing.push(target);
  }
  assert.deepEqual(missing, [], `broken README links: ${missing.join(", ")}`);
});
