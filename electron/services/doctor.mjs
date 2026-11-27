// The local "doctor" report and its opt-in repair path. run() is read-only;
// fix() only performs safe changes (create a missing log directory, rebuild
// a missing catalog cache, reopen the library DB). Ported from
// services/doctor.rs.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { DatabaseSync } from "node:sqlite";
import { configPath, catalogCachePath, appDir, dbPath } from "../core/paths.mjs";
import { manifest } from "./models.mjs";
import { engineInstalled } from "./engine.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const BUNDLED_CATALOG = path.join(here, "..", "..", "catalog", "models.json");

const check = (name, ok, detail) => ({ name, ok, detail });

/** True when every required check passed; the keychain probe is excluded. */
const fromChecks = (checks) => ({
  ok: checks.filter((c) => c.name !== "keychain").every((c) => c.ok),
  checks,
});

function jsonCheck(name, file, missingOk) {
  const label = path.basename(file);
  let text;
  try {
    text = fs.readFileSync(file, "utf8");
  } catch (e) {
    if (e?.code === "ENOENT") {
      return check(name, missingOk, missingOk ? `${label} absent (optional)` : `${label} missing`);
    }
    return check(name, false, `Could not read ${label}: ${e}`);
  }
  try {
    JSON.parse(text);
    return check(name, true, `${label} parses`);
  } catch (e) {
    return check(name, false, `${label} is not valid JSON: ${e}`);
  }
}

function libraryCheck() {
  const file = dbPath();
  if (!fs.existsSync(file)) return check("library_db", false, "library.db is missing");
  try {
    const db = new DatabaseSync(file, { readOnly: true });
    db.prepare("SELECT count(*) FROM sqlite_master").get();
    db.close();
    return check("library_db", true, "library.db opens and reads");
  } catch (e) {
    return check("library_db", false, `library.db is unreadable: ${e}`);
  }
}

/** Run all checks. Never throws: a failure is data in the report. */
export function doctorRun() {
  return fromChecks([
    jsonCheck("config", configPath(), true),
    jsonCheck("catalog_cache", catalogCachePath(), true),
    libraryCheck(),
    check("log_dir", fs.existsSync(path.join(appDir(), "logs")), fs.existsSync(path.join(appDir(), "logs")) ? "log directory is present" : "log directory is missing"),
    check("sidecar", engineInstalled(), engineInstalled() ? "model engine binary found" : "model engine binary is not installed"),
    check("keychain", true, "Keys are kept in the local config file in this build."),
  ]);
}

/** Write a valid cache entry from bundled catalog data. */
function rebuildCatalogCache(file) {
  const raw = fs.readFileSync(BUNDLED_CATALOG, "utf8");
  const parsed = JSON.parse(raw);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const payload = { manifest: parsed, fetched_at: 0 };
  const tmp = `${file}.tmp`;
  fs.writeFileSync(tmp, JSON.stringify(payload, null, 2), "utf8");
  fs.renameSync(tmp, file);
}

/** Repair only the safe issues. Never deletes or overwrites user data. */
export function doctorFix() {
  const fixed = [];
  const remaining = [];
  const logDir = path.join(appDir(), "logs");
  if (!fs.existsSync(logDir)) {
    try {
      fs.mkdirSync(logDir, { recursive: true });
      fixed.push(`created log directory ${logDir}`);
    } catch (e) {
      remaining.push(`could not create the log directory: ${e}`);
    }
  }
  if (!fs.existsSync(catalogCachePath())) {
    try {
      rebuildCatalogCache(catalogCachePath());
      fixed.push("recreated the catalog cache from bundled data");
    } catch (e) {
      remaining.push(`could not recreate the catalog cache: ${e}`);
    }
  }
  try {
    fs.mkdirSync(path.dirname(dbPath()), { recursive: true });
    const wasMissing = !fs.existsSync(dbPath());
    const db = new DatabaseSync(dbPath());
    const state = db.prepare("PRAGMA quick_check").get();
    db.close();
    if (state && Object.values(state)[0] === "ok") {
      fixed.push(wasMissing ? "created and opened the library database" : "reopened and verified the library database");
    } else {
      remaining.push(`could not reopen the library database: integrity check reported ${Object.values(state)[0]}`);
    }
  } catch (e) {
    remaining.push(`could not reopen the library database: ${e}`);
  }
  return { fixed, remaining };
}

/** The catalog version the health panel shows. */
export function catalogVersion() {
  return manifest().version ?? 0;
}
