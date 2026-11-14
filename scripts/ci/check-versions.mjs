#!/usr/bin/env node
// Fails when the app version differs between package.json, Cargo.toml and
// tauri.conf.json, or, on a tag build, from the tag (v1.2.3 means 1.2.3).
// Writes `version=<x>` to $GITHUB_OUTPUT for later jobs.

import { appendFileSync, readFileSync } from 'node:fs';

const read = (path) => readFileSync(path, 'utf8');

const versions = {
  'package.json': JSON.parse(read('package.json')).version,
  'src-tauri/Cargo.toml': /^\[package\][\s\S]*?^version\s*=\s*"([^"]+)"/m.exec(read('src-tauri/Cargo.toml'))?.[1],
  'src-tauri/tauri.conf.json': JSON.parse(read('src-tauri/tauri.conf.json')).version,
};
if (process.env.GITHUB_REF_TYPE === 'tag') {
  versions[`tag ${process.env.GITHUB_REF_NAME}`] = String(process.env.GITHUB_REF_NAME).replace(/^v/, '');
}

for (const [where, version] of Object.entries(versions)) {
  console.log(`${where}: ${version ?? 'missing'}`);
}

const distinct = new Set(Object.values(versions));
if (distinct.size !== 1 || distinct.has(undefined)) {
  const list = Object.entries(versions)
    .map(([where, version]) => `${where}=${version ?? 'missing'}`)
    .join(', ');
  console.log(`::error title=version::The version numbers differ: ${list}`);
  process.exit(1);
}

const [version] = distinct;
if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, `version=${version}\n`);
console.log(`All files agree on ${version}.`);
