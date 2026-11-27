// Writes the GitHub Release notes for the app version from CHANGELOG.md.
//
//   node scripts/ci/release-notes.mjs <output-file>
//
// Fails when CHANGELOG.md has no section for the version, so a release can
// never go out with empty or generic notes.

import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { changelogSection, previousVersion } from "./changelog.mjs";

const out = process.argv[2];
if (!out) {
  console.error("usage: release-notes.mjs <output-file>");
  process.exit(2);
}

const REPO = "omarjson/LocalAide";
const version = JSON.parse(readFileSync("package.json", "utf8")).version;
const section = changelogSection(version);
if (!section) {
  console.log(`::error title=changelog::CHANGELOG.md has no "## [${version}] - date" section. Write it before releasing.`);
  process.exit(1);
}

const previous = previousVersion(version);
const notes = `${section}

## Download

| Your computer | File |
|---|---|
| Windows 10 or 11 | \`LocalAide Setup ${version}.exe\` |
| Windows on ARM | \`LocalAide Setup ${version} arm64.exe\` |
| Mac with Apple Silicon (M1 or newer) | \`LocalAide-${version}-arm64.dmg\` |
| Mac with an Intel processor | \`LocalAide-${version}.dmg\` |
| Linux | \`.AppImage\` or \`.deb\` |

The first time a model loads, LocalAide downloads the llama.cpp engine built for your processor (13 to 25 MB, from github.com).

The installers are not code-signed yet. Windows may show "Windows protected your PC": choose **More info**, then **Run anyway**. On a Mac, if it says the app cannot be opened, right-click LocalAide in Applications and choose **Open**.

Check a download against \`SHA256SUMS.txt\`. \`localaide-sbom.spdx.json\` lists every package in the build.
${previous ? `\n**Full list of changes:** https://github.com/${REPO}/compare/v${previous}...v${version}\n` : ""}`;

writeFileSync(out, notes);
console.log(`Wrote release notes for ${version} (${section.length} characters from CHANGELOG.md).`);
