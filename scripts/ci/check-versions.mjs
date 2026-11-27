#!/usr/bin/env node
// Fails when, on a tag build, the tag (v1.2.3 means 1.2.3) does not match
// package.json's version or CHANGELOG.md has no section for it.
// Writes `version=<x>` to $GITHUB_OUTPUT for later jobs.

import { appendFileSync, readFileSync } from 'node:fs';
import { changelogSection } from './changelog.mjs';

const version = JSON.parse(readFileSync('package.json', 'utf8')).version;
const versions = { 'package.json': version };

if (process.env.GITHUB_REF_TYPE === 'tag') {
  versions[`tag ${process.env.GITHUB_REF_NAME}`] = String(process.env.GITHUB_REF_NAME).replace(/^v/, '');
}

for (const [where, value] of Object.entries(versions)) {
  console.log(`${where}: ${value ?? 'missing'}`);
}

const distinct = new Set(Object.values(versions));
if (distinct.size !== 1 || distinct.has(undefined)) {
  const list = Object.entries(versions)
    .map(([where, value]) => `${where}=${value ?? 'missing'}`)
    .join(', ');
  console.log(`::error title=version::The version numbers differ: ${list}`);
  process.exit(1);
}

// A release needs its notes: CHANGELOG.md must have this version's section.
if (process.env.GITHUB_REF_TYPE === 'tag' && !changelogSection(version)) {
  console.log(`::error title=changelog::CHANGELOG.md has no "## [${version}] - date" section.`);
  process.exit(1);
}
if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, `version=${version}\n`);
console.log(`All files agree on ${version}.`);
