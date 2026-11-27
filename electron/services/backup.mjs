// Encrypted local backups (ADR-0009). An archive holds the config (API keys
// stripped unless asked for), the memory notes and the conversations,
// encrypted with a passphrase: scrypt for the key, AES-256-GCM for the data,
// the whole header authenticated. Ported from services/backup/mod.rs — the
// KDF differs (scrypt instead of Argon2id, since Node has no Argon2), so
// archives made by the Electron build carry a v3 magic and only they can be
// opened here; archives from the Tauri build (v1/v2) honestly refuse.

import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { appDir, backupDir } from "../core/paths.mjs";
import { AppError } from "../core/errors.mjs";
import { loadConfig, invalidateConfig } from "../core/config.mjs";
import { appVersion } from "./updates.mjs";

const MAGIC_V3 = Buffer.from("LOCALAIDE-BACKUP-v3");
const MAGIC_V1 = Buffer.from("LOCALAIDE-BACKUP-v1");
const MAGIC_V2 = Buffer.from("LOCALAIDE-BACKUP-v2");
const SALT_LEN = 16;
const NONCE_LEN = 12;
const ARCHIVE_EXT = ".labak";

/** scrypt for new archives: 32 MiB, 1 lane. A header may not ask for more
 *  than 256 MiB or 8 lanes, so a crafted file cannot exhaust the machine. */
const SCRYPT = { N: 1 << 15, r: 8, p: 1, maxN: 1 << 18, maxP: 8 };

const nowSecs = () => Math.floor(Date.now() / 1000);

// ---- Collect and scrub ---------------------------------------------------

/** Small text files (json/md) under one app-folder directory. */
function collectDir(dir, prefix) {
  let names;
  try {
    names = fs.readdirSync(dir);
  } catch (e) {
    if (e?.code === "ENOENT") return [];
    throw AppError.internal(`Could not read ${prefix}: ${e}`);
  }
  const out = [];
  for (const name of names) {
    const file = path.join(dir, name);
    if (!fs.existsSync(file) || !fs.statSync(file).isFile()) continue;
    const ext = path.extname(file).toLowerCase();
    if (ext !== ".json" && ext !== ".md") continue;
    // Only small text files are backed up; model and database files are
    // reproducible and far too large to embed here.
    const stat = fs.statSync(file);
    if (stat.size > 4 * 1024 * 1024) continue;
    out.push({ rel_path: `${prefix}/${name}`, content: fs.readFileSync(file, "utf8") });
  }
  return out;
}

/** Strips API keys from the config unless they were explicitly included. */
function scrubConfig(config, includeApiKeys, takenAt) {
  const copy = JSON.parse(JSON.stringify(config));
  if (!includeApiKeys) {
    for (const provider of copy.cloud_providers ?? []) {
      if (provider && typeof provider === "object") provider.api_key = "";
    }
    if (copy.web_search && typeof copy.web_search === "object") {
      copy.web_search.exa_api_key = "";
      copy.web_search.firecrawl_api_key = "";
    }
    if (copy.opencode && typeof copy.opencode === "object") copy.opencode.password = null;
    for (const server of copy.mcp_servers ?? []) {
      const transport = server?.transport;
      if (!transport || typeof transport !== "object") continue;
      for (const slot of ["env", "headers"]) {
        if (transport[slot] && typeof transport[slot] === "object") {
          for (const key of Object.keys(transport[slot])) transport[slot][key] = "";
        }
      }
    }
  }
  // Stamp the backup time inside the config so a restore can refuse to
  // clobber a config newer than the archive.
  copy.backup_taken_at = takenAt;
  return copy;
}

// ---- Encryption (v3: scrypt + AES-256-GCM) -------------------------------

const notABackup = () => AppError.invalidInput("This file is not a LocalAide backup");

function deriveKey(passphrase, salt, params) {
  return crypto.scryptSync(String(passphrase), salt, 32, {
    N: params.N,
    r: params.r,
    p: params.p,
    maxmem: 512 * 1024 * 1024,
  });
}

function encryptPayload(payload, passphrase) {
  if (String(passphrase ?? "").length < 8) {
    throw AppError.invalidInput("A backup passphrase needs at least 8 characters — there is no recovery without it");
  }
  const plaintext = Buffer.from(JSON.stringify(payload), "utf8");
  const salt = crypto.randomBytes(SALT_LEN);
  const nonce = crypto.randomBytes(NONCE_LEN);
  const header = Buffer.concat([
    MAGIC_V3,
    headerField(SCRYPT.N),
    headerField(SCRYPT.r),
    headerField(SCRYPT.p),
    salt,
    nonce,
  ]);
  const key = deriveKey(passphrase, salt, SCRYPT);
  const cipher = crypto.createCipheriv("aes-256-gcm", key, nonce, { authTagLength: 16 });
  cipher.setAAD(header);
  const ciphertext = Buffer.concat([cipher.update(plaintext), cipher.final(), cipher.getAuthTag()]);
  return Buffer.concat([header, ciphertext]);
}

const headerField = (value) => {
  const buf = Buffer.alloc(4);
  buf.writeUInt32LE(value);
  return buf;
};

function decryptPayload(bytes, passphrase) {
  if (bytes.indexOf(MAGIC_V3) === 0) {
    const at = MAGIC_V3.length;
    const read = (offset) => (at + offset + 4 <= bytes.length ? bytes.readUInt32LE(at + offset) : null);
    const [N, r, p] = [read(0), read(4), read(8)];
    if (N === null || r === null || p === null) throw notABackup();
    if (N > SCRYPT.maxN || p > SCRYPT.maxP || N < 2 || r < 1 || p < 1) {
      throw AppError.invalidInput("This backup asks for more memory than LocalAide allows, so it was not opened.");
    }
    const head = at + 12 + SALT_LEN + NONCE_LEN;
    if (bytes.length <= head) throw notABackup();
    const salt = bytes.subarray(at + 12, at + 12 + SALT_LEN);
    const nonce = bytes.subarray(at + 12 + SALT_LEN, head);
    const body = bytes.subarray(head);
    return openBody(body, bytes.subarray(0, head), passphrase, salt, nonce, { N, r, p });
  }
  if (bytes.indexOf(MAGIC_V1) === 0 || bytes.indexOf(MAGIC_V2) === 0) {
    throw AppError.invalidInput(
      "This backup was made by the older (Tauri) build, which used a different key derivation. Restore it with that build once, then make a new backup here.",
    );
  }
  throw notABackup();
}

function openBody(body, aad, passphrase, salt, nonce, params) {
  const authTag = body.subarray(body.length - 16);
  const ciphertext = body.subarray(0, body.length - 16);
  let key;
  try {
    key = deriveKey(passphrase, salt, params);
  } catch {
    throw AppError.internal("The backup key could not be made");
  }
  try {
    const decipher = crypto.createDecipheriv("aes-256-gcm", key, nonce, { authTagLength: 16 });
    decipher.setAAD(aad);
    decipher.setAuthTag(authTag);
    const plaintext = Buffer.concat([decipher.update(ciphertext), decipher.final()]);
    return JSON.parse(plaintext.toString("utf8"));
  } catch {
    throw AppError.invalidInput("Backup decryption failed — wrong passphrase or a corrupted file");
  }
}

// ---- Archive naming -------------------------------------------------------

/** Compact UTC stamp: YYYYMMDD-HHMMSS from the epoch. */
function timestampName(secs) {
  // Compact UTC stamp: YYYYMMDD-HHMMSS from the epoch, civil-from-days.
  const days = Math.floor(secs / 86_400);
  const rem = secs % 86_400;
  const [h, m, s] = [Math.floor(rem / 3600), Math.floor((rem % 3600) / 60), rem % 60];
  const z = days + 719_468;
  const era = Math.floor(z / 146_097);
  const doe = z - era * 146_097;
  const yoe = Math.floor((doe - Math.floor(doe / 1460) + Math.floor(doe / 36524) - Math.floor(doe / 146096)) / 365);
  const y = yoe + era * 400;
  const doy = doe - (365 * yoe + Math.floor(yoe / 4) - Math.floor(yoe / 100));
  const mp = Math.floor((5 * doy + 2) / 153);
  const d = doy - Math.floor((153 * mp + 2) / 5) + 1;
  const mth = mp < 10 ? mp + 3 : mp - 9;
  const year = mth <= 2 ? y + 1 : y;
  return `${String(year).padStart(4, "0")}${String(mth).padStart(2, "0")}${String(d).padStart(2, "0")}-${String(h).padStart(2, "0")}${String(m).padStart(2, "0")}${String(s).padStart(2, "0")}`;
}

function parseTimestamp(fileName) {
  const stem = fileName.replace(/^localaide-backup-/, "").replace(ARCHIVE_EXT, "");
  const match = /^(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})$/.exec(stem);
  if (!match) return null;
  const [y, mth, d, h, m, s] = match.slice(1).map(Number);
  // days-from-civil (Howard Hinnant), the inverse of timestampName.
  const yy = mth <= 2 ? y - 1 : y;
  const era = Math.floor(yy / 400);
  const yoe = yy - era * 400;
  const mp = (mth + 9) % 12;
  const doy = Math.floor((153 * mp + 2) / 5) + d - 1;
  const doe = yoe * 365 + Math.floor(yoe / 4) - Math.floor(yoe / 100) + doy;
  const days = era * 146_097 + doe - 719_468;
  return days * 86_400 + h * 3600 + m * 60 + s;
}

/** Only memory/*.md and conversations/*.json make it back to disk. */
function restoreDestination(relPath) {
  const parts = String(relPath ?? "").split("/");
  const ok =
    parts.length === 2 &&
    (parts[0] === "memory" || parts[0] === "conversations") &&
    /\.(md|json)$/.test(parts[1]) &&
    !parts[1].includes("\\") &&
    parts[1] !== "..";
  if (!ok) {
    throw AppError.invalidInput(`The backup contains an unexpected file path: ${relPath}`);
  }
  return path.join(appDir(), parts[0], parts[1]);
}

/**
 * A key the backup left out (an empty string) keeps the current key, when
 * the current provider has the same id and the same address; a provider
 * whose address fails the base URL check is dropped; online chat is switched
 * off when its provider is gone.
 */
function mergeRestoredConfig(current, restored) {
  const text = (value, field) => (value && typeof value === "object" ? value[field] : undefined);
  const merged = JSON.parse(JSON.stringify(restored));
  const currentProviders = Array.isArray(current?.cloud_providers) ? current.cloud_providers : [];

  const validBaseUrl = (url) => {
    try {
      const parsed = new URL(String(url));
      if (parsed.username || parsed.password) return false;
      if (parsed.protocol === "https:") return true;
      if (parsed.protocol === "http:") {
        const name = parsed.hostname.toLowerCase();
        return name === "localhost" || /^127\./.test(name) || name === "[::1]";
      }
      return false;
    } catch {
      return false;
    }
  };

  if (Array.isArray(merged.cloud_providers)) {
    merged.cloud_providers = merged.cloud_providers.filter((p) => validBaseUrl(p?.base_url));
    for (const provider of merged.cloud_providers) {
      if (typeof provider.api_key === "string" && provider.api_key) continue;
      const kept = currentProviders.find(
        (c) => c?.id === provider.id && c?.base_url === provider.base_url && c?.api_key,
      );
      if (kept) provider.api_key = kept.api_key;
    }
    const active = merged.active_cloud_provider;
    const activeExists =
      active && merged.cloud_providers.some((p) => p.id === active);
    if (!activeExists) {
      merged.active_cloud_provider = null;
      merged.cloud_llm_enabled = false;
    }
  }

  const currentWeb = current?.web_search;
  if (merged.web_search && typeof merged.web_search === "object") {
    for (const slot of ["exa_api_key", "firecrawl_api_key"]) {
      const empty = !merged.web_search[slot];
      const kept = currentWeb?.[slot];
      if (empty && kept) merged.web_search[slot] = kept;
    }
  }
  return merged;
}

// ---- The commands ----------------------------------------------------------

/** Creates an encrypted archive and returns its details. */
export function backupRun({ passphrase, includeApiKeys = false, targetDir } = {}) {
  const dir = targetDir ? String(targetDir) : backupDir();
  const takenAt = nowSecs();
  const payload = {
    created_at: takenAt,
    app_version: appVersion(),
    config: scrubConfig(loadConfig(), includeApiKeys === true, takenAt),
    memories: collectDir(path.join(appDir(), "memory"), "memory"),
    conversations: collectDir(path.join(appDir(), "conversations"), "conversations"),
  };
  fs.mkdirSync(dir, { recursive: true });
  const bytes = encryptPayload(payload, passphrase);
  const fileName = `localaide-backup-${timestampName(takenAt)}${ARCHIVE_EXT}`;
  const file = path.join(dir, fileName);
  fs.writeFileSync(file, bytes);
  return {
    path: file,
    created_at: takenAt,
    memories: payload.memories.length,
    conversations: payload.conversations.length,
    api_keys_included: includeApiKeys === true,
  };
}

/** Lists archive files in the backup folder, newest first. */
export function backupList(targetDir) {
  const dir = targetDir ? String(targetDir) : backupDir();
  let names;
  try {
    names = fs.readdirSync(dir);
  } catch (e) {
    if (e?.code === "ENOENT") return [];
    throw AppError.internal(`Could not read backup folder: ${e}`);
  }
  return names
    .filter((name) => name.endsWith(ARCHIVE_EXT))
    .map((name) => {
      const file = path.join(dir, name);
      const stat = fs.existsSync(file) ? fs.statSync(file) : { size: 0, mtimeMs: 0 };
      return {
        file_name: name,
        created_at: parseTimestamp(name) ?? Math.floor(stat.mtimeMs / 1000),
        size_bytes: stat.size,
        app_version: "",
      };
    })
    .sort((a, b) => b.created_at - a.created_at);
}

/**
 * Restores memories and conversations (always) and, when requested, the
 * config — but never over a config newer than the backup's.
 */
export function backupRestore({ fileName, passphrase, restoreConfig = false, targetDir } = {}) {
  // The file name must be a bare file name inside the backup folder — no
  // traversal into arbitrary paths.
  if (path.basename(String(fileName)) !== String(fileName) || !String(fileName).endsWith(ARCHIVE_EXT)) {
    throw AppError.invalidInput("Restore takes a backup file name from the backup folder");
  }
  const dir = targetDir ? String(targetDir) : backupDir();
  const file = path.join(dir, String(fileName));
  if (!fs.existsSync(file)) {
    throw AppError.invalidInput("That backup file is not in the backup folder any more.");
  }
  const bytes = Buffer.from(fs.readFileSync(file));
  const payload = decryptPayload(bytes, passphrase);

  // Verify the passphrase against this exact archive before touching any
  // file, so a wrong passphrase restores nothing at all.
  let memoriesRestored = 0;
  let conversationsRestored = 0;
  const dests = [...payload.memories ?? [], ...payload.conversations ?? []].map((f) => ({
    file: f,
    dest: restoreDestination(f.rel_path),
  }));

  for (const { file: f, dest } of dests) {
    fs.mkdirSync(path.dirname(dest), { recursive: true });
    fs.writeFileSync(dest, f.content, "utf8");
    if (f.rel_path.startsWith("memory/")) memoriesRestored += 1;
    else conversationsRestored += 1;
  }

  // Config restore is opt-in and refuses to clobber a newer file.
  let configRestored = false;
  let skippedNewerConfig = false;
  if (restoreConfig === true && payload.config && typeof payload.config === "object") {
    const backupTime = payload.config.backup_taken_at;
    const configFile = path.join(appDir(), "config.json");
    let current = null;
    try {
      current = JSON.parse(fs.readFileSync(configFile, "utf8"));
    } catch {
      current = null;
    }
    if (typeof backupTime === "number" && current?.backup_taken_at && current.backup_taken_at > backupTime) {
      skippedNewerConfig = true;
    } else {
      const merged = mergeRestoredConfig(current, payload.config);
      fs.writeFileSync(configFile, JSON.stringify(merged, null, 2), "utf8");
      invalidateConfig();
      configRestored = true;
    }
  }

  return {
    memories_restored: memoriesRestored,
    conversations_restored: conversationsRestored,
    config_restored: configRestored,
    skipped_newer_config: skippedNewerConfig,
  };
}
