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

// rustup takes the Rust version from rust-toolchain.toml, whatever CI
// installs. If the two differ, CI installs targets for a version that is
// never used, and a cross build (x86_64 on Apple Silicon) cannot find them.
const toolchainFile = /^channel\s*=\s*"([^"]+)"/m.exec(read('rust-toolchain.toml'))?.[1];
const ciToolchain = /^\s*RUST_TOOLCHAIN:\s*'([^']+)'/m.exec(read('.github/workflows/ci.yml'))?.[1];
console.log(`rust-toolchain.toml: ${toolchainFile ?? 'missing'}; ci.yml RUST_TOOLCHAIN: ${ciToolchain ?? 'missing'}`);
if (!toolchainFile || toolchainFile !== ciToolchain) {
  console.log('::error title=toolchain::rust-toolchain.toml and RUST_TOOLCHAIN in ci.yml must name the same Rust version.');
  process.exit(1);
}

const [version] = distinct;
if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, `version=${version}\n`);
console.log(`All files agree on ${version}.`);
