#!/usr/bin/env node
// Bump the app version in every file that must agree on it.
//
//   node scripts/bump-version.mjs <new-version> [--dry-run] [--check]
//   node scripts/bump-version.mjs --check
//
// The six files (same set scripts/ci/check-versions.mjs gates on, plus the
// three it does not cover):
//   package.json                  "version": "x.y.z"
//                                 ONLY — the unrelated cfg_aliases 0.2.2
//                                 entry is left alone
//   CHANGELOG.md                  the version section heading (see below)
//   README.md                     the version badge (version-vx.y.z-blue)
//
// Bump behaviour:
// - The six files must already agree before a bump; a disagreeing tree is
//   refused (fix it first, don't bump from a broken state).
// - The new version must be x.y.z, and different from the current one.
// - CHANGELOG.md: the `## [Unreleased]` section is promoted to
//   `## [<new>] - <today>` and a fresh empty `## [Unreleased]` is put
//   above it. With no Unreleased section, the latest `## [x.y.z]` heading
//   is renamed to the new version instead.
// - Replacements are textual, never a JSON re-serialisation, so file
//   formatting (indentation, key order, trailing newline) is untouched.
//
// Flags:
//   --dry-run   print what would change, write nothing.
//   --check     read the version from all six files, print them, and exit
//               1 when they disagree or any is missing. No writes.
//   --root <dir>  treat <dir> as the repo root (default: the repo this
//               script lives in). For testing against a temp copy:
//                 cp the six files to /tmp/bump-test/<same layout>
//                 node scripts/bump-version.mjs --root /tmp/bump-test 0.2.4
//
// Exit codes: 0 on success, 1 on bad usage, disagreement, or a missing
// version in any file.

import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";

const SEMVER = /^\d+\.\d+\.\d+$/;
const VERSION_HEADING = /^## \[(\d+\.\d+\.\d+)\]/m;

const read = (root, rel) => readFileSync(join(root, rel), "utf8");
const write = (root, rel, text) => writeFileSync(join(root, rel), text, "utf8");

/** Today's date (UTC) for the promoted CHANGELOG heading. */
const today = () => new Date().toISOString().slice(0, 10);

// --- readers: the current version in each file (null when missing) ---

function packageVersion(text) {
  try {
    return JSON.parse(text).version ?? null;
  } catch {
    return null;
  }
}

function cargoTomlVersion(text) {
  // Same pattern as scripts/ci/check-versions.mjs.
  return /^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m.exec(text)?.[1] ?? null;
}

function tauriConfVersion(text) {
  try {
    return JSON.parse(text).version ?? null;
  } catch {
    return null;
  }
}

/** The ai-desktop-assistant [[package]] block, or null. */
function lockBlock(text) {
  const start = text.indexOf('[[package]]\nname = "ai-desktop-assistant"');
  if (start < 0) return null;
  const end = text.indexOf("[[package]]", start + 1);
  return text.slice(start, end < 0 ? text.length : end);
}

function cargoLockVersion(text) {
  const block = lockBlock(text);
  return block ? (/^version = "([^"]+)"/m.exec(block)?.[1] ?? null) : null;
}

function changelogVersion(text) {
  return VERSION_HEADING.exec(text)?.[1] ?? null;
}

function readmeVersion(text) {
  return /version-v(\d+\.\d+\.\d+)-blue/.exec(text)?.[1] ?? null;
}

const READERS = [
  ["package.json", packageVersion],
  ["CHANGELOG.md", changelogVersion],
  ["README.md", readmeVersion],
];

function readVersions(root) {
  return READERS.map(([rel, readVersion]) => {
    let version = null;
    try {
      version = readVersion(read(root, rel));
    } catch {
      version = null;
    }
    return { rel, version };
  });
}

// --- writers: replace the version in each file's text ---

function replaceJsonVersion(text, next) {
  // First (and only) top-level "version" key; textual, so formatting stays.
  const out = text.replace(/"version":\s*"[^"]+"/, `"version": "${next}"`);
  if (out === text) throw new Error('no "version" key found');
  return out;
}

function replaceCargoTomlVersion(text, next) {
  const out = text.replace(
    /^(\[package\][\s\S]*?^version\s*=\s*")[^"]+(")/m,
    `$1${next}$2`,
  );
  if (out === text) throw new Error("no [package] version found");
  return out;
}

function replaceCargoLockVersion(text, next) {
  const block = lockBlock(text);
  if (!block) throw new Error('no ai-desktop-assistant package in Cargo.lock');
  const replaced = block.replace(/^version = "[^"]+"/m, `version = "${next}"`);
  if (replaced === block) throw new Error("no version in the ai-desktop-assistant block");
  // Reassemble: only this block changes, so cfg_aliases (and every other
  // package) is untouched by construction.
  const start = text.indexOf(block);
  return text.slice(0, start) + replaced + text.slice(start + block.length);
}

function replaceChangelogVersion(text, next) {
  const unreleased = /^## \[Unreleased\]\n/m.exec(text);
  if (unreleased) {
    // Promote the section and start a fresh Unreleased above it.
    return text.replace(
      /^## \[Unreleased\]\n/m,
      `## [Unreleased]\n\n## [${next}] - ${today()}\n`,
    );
  }
  // No Unreleased section: rename the latest version heading, consuming
  // the whole line (version and date) so nothing is left doubled.
  const heading = /^## \[\d+\.\d+\.\d+\][^\n]*/m.exec(text);
  if (!heading) throw new Error("no version heading in CHANGELOG.md");
  return text.replace(/^## \[\d+\.\d+\.\d+\][^\n]*/m, () => `## [${next}] - ${today()}`);
}

function replaceReadmeVersion(text, next) {
  const out = text.replace(/version-v\d+\.\d+\.\d+-blue/, `version-v${next}-blue`);
  if (out === text) throw new Error("no version badge found");
  return out;
}

const WRITERS = [
  ["package.json", replaceJsonVersion],
  ["CHANGELOG.md", replaceChangelogVersion],
  ["README.md", replaceReadmeVersion],
];

// --- CLI ---

function usageError(message) {
  console.error(`bump-version: ${message}`);
  console.error("usage: node scripts/bump-version.mjs <new-version> [--dry-run] [--check]");
  console.error("       node scripts/bump-version.mjs --check [--root <dir>]");
  process.exit(1);
}

function main() {
  const args = process.argv.slice(2);
  let root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  let dryRun = false;
  let check = false;
  let target = null;

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (arg === "--dry-run") dryRun = true;
    else if (arg === "--check") check = true;
    else if (arg === "--root") {
      root = resolve(args[++i] ?? usageError("--root needs a directory"));
    } else if (arg.startsWith("--")) usageError(`unknown flag ${arg}`);
    else if (target === null) target = arg;
    else usageError(`unexpected argument ${arg}`);
  }

  if (check) {
    const versions = readVersions(root);
    for (const { rel, version } of versions) {
      console.log(`${rel}: ${version ?? "missing"}`);
    }
    const distinct = new Set(versions.map((v) => v.version));
    if (distinct.size !== 1 || distinct.has(null)) {
      console.error("bump-version: the version numbers differ (or one is missing).");
      process.exit(1);
    }
    console.log(`All files agree on ${[...distinct][0]}.`);
    return;
  }

  if (target === null) usageError("a new version is required");
  if (!SEMVER.test(target)) usageError(`"${target}" is not x.y.z`);

  const versions = readVersions(root);
  const distinct = new Set(versions.map((v) => v.version));
  if (distinct.size !== 1 || distinct.has(null)) {
    for (const { rel, version } of versions) {
      console.log(`${rel}: ${version ?? "missing"}`);
    }
    console.error("bump-version: refusing to bump a disagreeing tree — fix the versions first.");
    process.exit(1);
  }
  const current = [...distinct][0];
  if (current === target) usageError(`already at ${target}`);

  for (const [rel, replace] of WRITERS) {
    const before = read(root, rel);
    const after = replace(before, target);
    if (dryRun) {
      console.log(`${rel}: ${current} -> ${target}`);
    } else {
      write(root, rel, after);
      console.log(`${rel}: ${current} -> ${target}`);
    }
  }
  console.log(dryRun ? `Dry run: ${current} -> ${target} (nothing written).` : `Bumped ${current} -> ${target}.`);
}

main();
