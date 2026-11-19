// Turns on signed update packages for a CI build when the repository has
// the update signing key (docs/release.md, "Updates").
//
// Reads HAS_SIGNING_KEY ("true" when the TAURI_SIGNING_PRIVATE_KEY secret
// is set) and UPDATER_PUBKEY (the public key, from the repository variable
// LOCALAIDE_UPDATER_PUBKEY; optional when tauri.conf.json already carries
// it). Writes a Tauri config overlay and exports its path as UPDATER_CONFIG
// for the build step. Without the key it does nothing, and the build stays
// as before: installers only, no in-app updates.

import { appendFileSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const hasKey = process.env.HAS_SIGNING_KEY === "true";
const pubkey = (process.env.UPDATER_PUBKEY ?? "").trim();
const baseKey = JSON.parse(readFileSync(join("src-tauri", "tauri.conf.json"), "utf8"))?.plugins?.updater?.pubkey ?? "";

if (!hasKey) {
  console.log("::notice::No update signing key is set, so this build cannot update itself in the app.");
  process.exit(0);
}
if (!pubkey && !baseKey.trim()) {
  console.log(
    "::warning::The update signing key is set but no public key is: add the repository variable LOCALAIDE_UPDATER_PUBKEY or plugins.updater.pubkey in tauri.conf.json. Packages are signed, but the app will not offer in-app updates.",
  );
}

const overlay = { bundle: { createUpdaterArtifacts: true } };
if (pubkey) overlay.plugins = { updater: { pubkey } };

const dir = process.env.RUNNER_TEMP ?? ".";
const path = join(dir, "updater.conf.json");
writeFileSync(path, JSON.stringify(overlay, null, 2));
if (process.env.GITHUB_ENV) appendFileSync(process.env.GITHUB_ENV, `UPDATER_CONFIG=${path}\n`);
console.log(`Signed update packages on (${path}).`);
