import fs from "node:fs";
const src = fs.readFileSync("src/lib/api/client.ts", "utf8");
const re = /invoke<[^>]*>\(\s*"([a-z0-9_]+)"((?:\s*,\s*\{[\s\S]*?\})?)\s*\)/g;
const out = [];
for (const m of src.matchAll(re)) {
  const keys = m[2] ? [...m[2].matchAll(/[{,]\s*([A-Za-z_][A-Za-z0-9_]*)\s*[:,]/g)].map((k) => k[1]) : [];
  out.push([m[1], [...new Set(keys)].join(",")]);
}
console.log(out.length);
for (const [n, k] of out) console.log(n + "|" + k);

