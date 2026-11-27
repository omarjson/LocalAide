// Append-only app log, one file per day, capped. The Activity screen tails it.

import fs from "node:fs";
import path from "node:path";
import { logsDir } from "./paths.mjs";

const MAX_BYTES = 4 * 1024 * 1024;

export const logFile = () => path.join(logsDir(), "app.log");

export function log(level, message, detail) {
  const line = `${new Date().toISOString()} ${level.toUpperCase()} ${message}${detail ? " " + detail : ""}\n`;
  try {
    const file = logFile();
    if (fs.existsSync(file) && fs.statSync(file).size > MAX_BYTES) {
      fs.renameSync(file, file + ".1");
    }
    fs.appendFileSync(file, line);
  } catch {
    /* a log that cannot be written must never take the app down */
  }
  if (process.env.LOCALAIDE_DEBUG) process.stderr.write(line);
}

/** The last N lines of a log file ("app" or "engine"). */
export function tail(source, lines = 200) {
  const file = source === "engine" ? path.join(logsDir(), "engine.log") : logFile();
  if (!fs.existsSync(file)) return [];
  const text = fs.readFileSync(file, "utf8");
  return text.trimEnd().split("\n").slice(-Math.max(1, lines));
}

