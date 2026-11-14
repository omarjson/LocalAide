// Verifies every catalog/models.json pin against the Hugging Face API:
// for each model and companion, the LFS sha256 and size recorded in the
// catalog must match what HF currently serves. Downloads nothing.
//
// Usage: node scripts/verify-catalog.mjs [path/to/models.json]
// Exit 0 = every pin matches; exit 1 = at least one mismatch or
// unreachable repo (fail closed, so CI catches a drifted pin).

import { readFileSync } from "node:fs";

const catalogPath = process.argv[2] ?? "catalog/models.json";
const catalog = JSON.parse(readFileSync(catalogPath, "utf8"));

const HF_API = "https://huggingface.co/api/models";
const repoCache = new Map();

async function repoTree(org, repo) {
  const key = `${org}/${repo}`;
  if (repoCache.has(key)) return repoCache.get(key);
  const res = await fetch(`${HF_API}/${key}/tree/main?recursive=true`);
  if (!res.ok) {
    throw new Error(`repo ${key} unreachable (HTTP ${res.status})`);
  }
  const tree = await res.json();
  repoCache.set(key, tree);
  return tree;
}

function findFile(tree, path) {
  return tree.find((f) => f.path === path);
}

function lfsMeta(file) {
  if (file?.lfs) return { sha256: file.lfs.oid, size: file.lfs.size };
  return { sha256: null, size: file?.size ?? 0 };
}

let checked = 0;
let failures = 0;

async function checkPin(label, url, expectedSha, expectedSize) {
  const match = url.match(/^https:\/\/huggingface\.co\/([^/]+)\/([^/]+)\/resolve\/main\/(.+)$/);
  if (!match) {
    console.error(`FAIL ${label}: URL is not an HF resolve link: ${url}`);
    failures += 1;
    return null;
  }
  const [, org, repo, path] = match;
  try {
    const tree = await repoTree(org, repo);
    const meta = lfsMeta(findFile(tree, path));
    checked += 1;
    // expectedSize === null skips the size comparison (split-shard primaries
    // are verified via the total check below instead).
    const sizeOk = expectedSize === null || meta.size === expectedSize;
    const shaOk = meta.sha256 === null || meta.sha256.toLowerCase() === expectedSha.toLowerCase();
    if (!sizeOk || !shaOk) {
      console.error(
        `FAIL ${label}: catalog pin drifted from HF\n` +
          `  size: catalog=${expectedSize ?? "(total check)"} hf=${meta.size} (${sizeOk ? "ok" : "MISMATCH"})\n` +
          `  sha256: catalog=${expectedSha} hf=${meta.sha256 ?? "(non-lfs)"} (${shaOk ? "ok" : "MISMATCH"})`
      );
      failures += 1;
      return meta.sha256 === null ? null : meta.size;
    }
    console.log(`ok   ${label}`);
    return meta.size;
  } catch (e) {
    console.error(`FAIL ${label}: ${e.message}`);
    failures += 1;
    return null;
  }
}

// Split GGUF shards follow the llama.cpp naming convention; companions
// like an mmproj projector must not be counted into the shard total.
const isShard = (dest) => /-\d{5}-of-\d{5}\.gguf$/.test(dest);

for (const m of catalog.models) {
  const split = Boolean(m.dest);
  const primarySize = await checkPin(m.id, m.url, m.sha256, split ? null : m.file_size);
  let total = primarySize ?? 0;
  for (const c of m.companions ?? []) {
    const size = await checkPin(`${m.id} :: ${c.dest}`, c.url, c.sha256, c.file_size);
    if (split && size !== null && isShard(c.dest)) total += size;
  }
  if (split && total !== m.file_size) {
    console.error(
      `FAIL ${m.id}: split-shard total mismatch\n` +
        `  catalog total=${m.file_size} hf sum=${total}`
    );
    failures += 1;
  }
}

console.log(`\n${checked} pins checked, ${failures} failed, ${catalog.models.length} models.`);
if (failures > 0) process.exit(1);
