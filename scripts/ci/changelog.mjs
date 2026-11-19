// Reads one version's section from CHANGELOG.md ("## [x.y.z] - date" up to
// the next "## [" heading). Shared by the release notes and the update feed.

import { readFileSync } from "node:fs";

/** The section body without its heading, or null when there is none. */
export function changelogSection(version, path = "CHANGELOG.md") {
  const text = readFileSync(path, "utf8").replace(/\r\n?/g, "\n");
  const heading = new RegExp(`^## \\[${version.replace(/\./g, "\\.")}\\][^\\n]*$`, "m");
  const match = heading.exec(text);
  if (!match) return null;
  const rest = text.slice(match.index + match[0].length);
  const next = rest.search(/^## \[/m);
  const body = (next < 0 ? rest : rest.slice(0, next)).trim();
  return body || null;
}

/** The version just before `version` in the changelog, for a compare link. */
export function previousVersion(version, path = "CHANGELOG.md") {
  const versions = [...readFileSync(path, "utf8").matchAll(/^## \[(\d+\.\d+\.\d+)\]/gm)].map((m) => m[1]);
  const i = versions.indexOf(version);
  return i >= 0 && i + 1 < versions.length ? versions[i + 1] : null;
}
