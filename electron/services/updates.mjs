// Update check (ADR-0005). Nothing runs on its own: the user presses
// "Check for updates". This build has no signed updater, so the result
// links to the release page; can_install stays false, like the Rust build
// without an update key.

import { createRequire } from "node:module";
import { AppError } from "../core/errors.mjs";

const require = createRequire(import.meta.url);

const RELEASES_API = "https://api.github.com/repos/omarjson/LocalAide/releases/latest";
const RELEASES_PAGE = "https://github.com/omarjson/LocalAide/releases/latest";
const MAX_NOTES_CHARS = 4000;

/** The app version, from package.json, without reading it on every call. */
let cachedVersion = null;
export function appVersion() {
  if (cachedVersion) return cachedVersion;
  try {
    const pkg = require("../../package.json");
    cachedVersion = String(pkg.version ?? "0.0.0");
  } catch {
    cachedVersion = "0.0.0";
  }
  return cachedVersion;
}

const trimNotes = (notes) => {
  const trimmed = String(notes ?? "").trim();
  if (!trimmed) return null;
  return [...trimmed].length <= MAX_NOTES_CHARS ? trimmed : `${[...trimmed].slice(0, MAX_NOTES_CHARS).join("")}…`;
};

/** True when latest is strictly newer than current (dotted numerics). */
export function versionNewer(current, latest) {
  const parse = (v) => v.split(".").map((p) => Number(p.trim()) || 0);
  const [a, b] = [parse(current), parse(latest)];
  for (let i = 0; i < Math.max(a.length, b.length); i += 1) {
    const x = a[i] ?? 0;
    const y = b[i] ?? 0;
    if (y > x) return true;
    if (y < x) return false;
  }
  return false;
}

/**
 * The newest published release's version and notes. Offline or any HTTP
 * failure yields (null, null), and the UI shows "already newest".
 */
export async function latestRelease() {
  try {
    const response = await fetch(RELEASES_API, {
      headers: {
        "User-Agent": "LocalAide-update-check",
        Accept: "application/vnd.github+json",
      },
      signal: AbortSignal.timeout(15_000),
    });
    if (!response.ok) return [null, null];
    const body = await response.json();
    const tag = typeof body?.tag_name === "string" ? body.tag_name.replace(/^v/, "") : null;
    return [tag, trimNotes(body?.body)];
  } catch {
    return [null, null];
  }
}

export async function updateCheck() {
  const current = appVersion();
  const [latest, notes] = await latestRelease();
  const available = latest ? versionNewer(current, latest) : false;
  return {
    current_version: current,
    latest_version: available ? latest : latest ?? current,
    update_available: available,
    release_url: RELEASES_PAGE,
    can_install: false,
    notes: available ? notes : null,
  };
}

/** In-app install: not available in this build. */
export function updateInstall() {
  throw new AppError(
    "UPDATE_UNAVAILABLE",
    "This copy of LocalAide cannot install updates by itself. Download the new version from the release page.",
  );
}
