import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

// Evidence sweep: every local link in the README must resolve, so claims
// that point at an ADR or an evidence file cannot rot silently.

// fileURLToPath, because URL.pathname on Windows yields /C:/... which
// path.resolve turns into C:\C:\... and every link reads as missing.
const readmePath = fileURLToPath(new URL("../../README.md", import.meta.url));
const root = dirname(readmePath);
const text = readFileSync(readmePath, "utf8");

test("README local links resolve", () => {
  const missing: string[] = [];
  for (const match of text.matchAll(/\[[^\]]*\]\(([^)]+)\)/g)) {
    const target = match[1].split("#")[0].trim();
    if (!target || /^(https?:|mailto:)/.test(target)) continue;
    if (!existsSync(resolve(root, decodeURIComponent(target)))) missing.push(target);
  }
  assert.deepEqual(missing, [], `broken README links: ${missing.join(", ")}`);
});
