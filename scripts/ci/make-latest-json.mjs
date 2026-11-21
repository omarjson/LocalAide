// Writes the update feed (latest.json) for tauri-plugin-updater from the
// signed build artifacts, and copies the update packages next to the
// installers.
//
//   node scripts/ci/make-latest-json.mjs <artifacts-dir> <release-dir>
//
// <artifacts-dir> holds one folder per build job (macos-<target>,
// windows-<target>, linux-x86_64), as actions/download-artifact lays them
// out. Each signed update package has a "<file>.sig" next to it; builds made
// without TAURI_SIGNING_PRIVATE_KEY have none, and then no feed is written.
//
// The installed app reads the feed from
// https://github.com/omarjson/LocalAide/releases/latest/download/latest.json
// (tauri.conf.json, plugins.updater.endpoints).

import { copyFileSync, existsSync, readFileSync, readdirSync, statSync, writeFileSync } from "node:fs";
import { basename, join, relative, sep } from "node:path";
import { changelogSection } from "./changelog.mjs";

const [artifactsDir, releaseDir] = process.argv.slice(2);
if (!artifactsDir || !releaseDir) {
  console.error("usage: make-latest-json.mjs <artifacts-dir> <release-dir>");
  process.exit(2);
}

const REPO = "omarjson/LocalAide";
const root = process.cwd();
const version = JSON.parse(readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8")).version;
const tag = `v${version}`;

const walk = (dir) =>
  readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    return statSync(path).isDirectory() ? walk(path) : [path];
  });

/** Which updater platform a signed package is for, and its release name. */
function classify(file) {
  const job = relative(artifactsDir, file).split(sep)[0] ?? "";
  const name = basename(file);
  if (name.endsWith(".app.tar.gz")) {
    if (job.includes("aarch64")) return { platform: "darwin-aarch64", asset: `LocalAide_${version}_aarch64.app.tar.gz` };
    if (job.includes("x86_64")) return { platform: "darwin-x86_64", asset: `LocalAide_${version}_x64.app.tar.gz` };
  }
  if (name.endsWith("-setup.exe") && job.startsWith("windows")) {
    return { platform: job.includes("aarch64") ? "windows-aarch64" : "windows-x86_64", asset: name };
  }
  if (name.endsWith(".AppImage") && job.startsWith("linux")) return { platform: "linux-x86_64", asset: name };
  return null;
}

const platforms = {};
for (const sig of walk(artifactsDir).filter((f) => f.endsWith(".sig"))) {
  const file = sig.slice(0, -4);
  if (!existsSync(file)) continue;
  const target = classify(file);
  if (!target) continue;
  if (platforms[target.platform]) {
    console.error(`::error title=updater::Two update packages for ${target.platform}`);
    process.exit(1);
  }
  copyFileSync(file, join(releaseDir, target.asset));
  platforms[target.platform] = {
    signature: readFileSync(sig, "utf8").trim(),
    url: `https://github.com/${REPO}/releases/download/${tag}/${encodeURIComponent(target.asset)}`,
  };
  console.log(`${target.platform}: ${target.asset}`);
}

if (Object.keys(platforms).length === 0) {
  console.log("::warning::No signed update packages were found, so no latest.json was written. Set TAURI_SIGNING_PRIVATE_KEY (docs/release.md).");
  process.exit(0);
}

const feed = {
  version,
  // Shown in Settings → About under "What's new".
  notes: changelogSection(version) ?? `LocalAide ${version}`,
  pub_date: new Date().toISOString(),
  platforms,
};
writeFileSync(join(releaseDir, "latest.json"), JSON.stringify(feed, null, 2) + "\n");
console.log(`Wrote latest.json for ${Object.keys(platforms).join(", ")}`);
