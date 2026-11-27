// Where LocalAide keeps its files. One place, so every module agrees.
// Mirrors the Rust app::paths layout, so an existing install keeps its data.

import os from "node:os";
import path from "node:path";
import fs from "node:fs";

/** Root of the app's data, ~/.aiassistant (kept from the Tauri build). */
export const appDir = () => process.env.LOCALAIDE_HOME || path.join(os.homedir(), ".aiassistant");

const ensure = (dir) => {
  fs.mkdirSync(dir, { recursive: true });
  return dir;
};

export const configPath = () => path.join(appDir(), "config.json");
export const modelsDir = () => ensure(path.join(appDir(), "models"));
export const indexDir = () => ensure(path.join(appDir(), "index"));
export const memoryDir = () => ensure(path.join(appDir(), "memory"));
export const workspaceDir = () => ensure(path.join(appDir(), "workspace"));
/** Saved chats, one JSON file each (the Rust build called this conversations). */
export const conversationsDir = () => ensure(path.join(appDir(), "conversations"));
export const skillsDir = () => ensure(path.join(appDir(), "skills"));
export const logsDir = () => ensure(path.join(appDir(), "logs"));
export const engineDir = () => ensure(path.join(appDir(), "engine"));
export const notesSuggestionsDir = () => ensure(path.join(appDir(), "note_suggestions"));
export const catalogCachePath = () => path.join(appDir(), "catalog-cache.json");
export const customModelsPath = () => path.join(appDir(), "custom-models.json");
export const usageLogPath = () => path.join(appDir(), "usage.jsonl");
export const backupDir = () => ensure(path.join(appDir(), "backups"));
export const dbPath = () => path.join(indexDir(), "library.db");

/** A path inside the workspace, refusing anything that escapes it. */
export function insideWorkspace(candidate) {
  const root = path.resolve(workspaceDir());
  const full = path.resolve(root, candidate);
  if (full !== root && !full.startsWith(root + path.sep)) {
    throw new Error("That path is outside the workspace folder.");
  }
  return full;
}

