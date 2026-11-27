// Automations: saved prompts that run on a schedule or on demand, with a
// history of runs. All state lives in the library database's automations
// and automation_runs tables. Ported from services/automations.rs.

import crypto from "node:crypto";
import { AppError } from "../core/errors.mjs";
import { get, all, run } from "../core/db.mjs";
import { emitEvent } from "../core/channels.mjs";
import { quickAnswer } from "./chat.mjs";

const rowToAutomation = (row) => ({
  id: row.id,
  name: row.name,
  prompt: row.prompt,
  kind: row.kind,
  hour: row.hour ?? null,
  minute: row.minute ?? null,
  interval_minutes: row.interval_minutes ?? null,
  enabled: row.enabled === 1,
  created_at: row.created_at,
  last_run_at: row.last_run_at ?? null,
});

const rowToRun = (row) => ({
  automation_id: row.automation_id,
  name: row.name,
  at: row.at,
  ok: row.ok === 1,
  answer: row.answer,
  ms: row.ms,
});

function validName(name) {
  const clean = String(name ?? "").trim();
  if (!clean) throw AppError.invalidInput("Give the automation a name.");
  if (clean.length > 80) throw AppError.invalidInput("Keep the name under 80 characters.");
  return clean;
}

/** Validates kind/hour/minute/interval, as the Rust save did. */
function validateShape(a) {
  if (a.kind === "daily") {
    if (!Number.isInteger(a.hour) || a.hour < 0 || a.hour > 23) {
      throw AppError.invalidInput("A daily automation needs an hour of the day (0–23).");
    }
    if (!Number.isInteger(a.minute) || a.minute < 0 || a.minute > 59) {
      throw AppError.invalidInput("A daily automation needs a minute (0–59).");
    }
  } else if (a.kind === "hourly") {
    if (!Number.isInteger(a.minute) || a.minute < 0 || a.minute > 59) {
      throw AppError.invalidInput("An hourly automation needs a minute (0–59).");
    }
  } else if (a.kind === "every") {
    if (!Number.isInteger(a.interval_minutes) || a.interval_minutes < 5 || a.interval_minutes > 1440) {
      throw AppError.invalidInput("The interval must be between 5 minutes and a day.");
    }
  } else {
    throw AppError.invalidInput("Automations are daily, hourly or every-N-minutes.");
  }
}

function state() {
  const automations = all(`SELECT * FROM automations ORDER BY created_at ASC`).map(rowToAutomation);
  const runs = all(`SELECT * FROM automation_runs ORDER BY at DESC LIMIT 50`).map(rowToRun);
  return { automations, runs };
}

export function automationsList() {
  return state();
}

/** Add or update one automation, then report the new state. */
export function automationsSave(automation) {
  const a = automation ?? {};
  const name = validName(a.name);
  const kind = String(a.kind ?? "");
  const shape = {
    kind,
    hour: a.hour ?? null,
    minute: a.minute ?? null,
    interval_minutes: a.interval_minutes ?? null,
  };
  validateShape(shape);
  const prompt = String(a.prompt ?? "").trim();
  if (!prompt) throw AppError.invalidInput("Write the prompt the automation sends.");
  const now = Math.floor(Date.now() / 1000);
  const id = a.id && typeof a.id === "string" ? a.id : `auto_${crypto.randomUUID().slice(0, 8)}`;
  run(
    `INSERT INTO automations (id, name, prompt, kind, hour, minute, interval_minutes, enabled, created_at, last_run_at)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
     ON CONFLICT(id) DO UPDATE SET
       name = excluded.name, prompt = excluded.prompt, kind = excluded.kind,
       hour = excluded.hour, minute = excluded.minute, interval_minutes = excluded.interval_minutes,
       enabled = excluded.enabled`,
    id, name, prompt, kind, shape.hour, shape.minute, shape.interval_minutes, a.enabled === false ? 0 : 1, now, null,
  );
  emitEvent("automations-changed", null);
  return state();
}

export function automationsDelete(id) {
  run(`DELETE FROM automations WHERE id = ?`, String(id));
  run(`DELETE FROM automation_runs WHERE automation_id = ?`, String(id));
  emitEvent("automations-changed", null);
  return state();
}

/** Run one automation now, through whatever model chat would use. */
export async function automationsRunNow(id) {
  const row = get(`SELECT * FROM automations WHERE id = ?`, String(id));
  if (!row) throw AppError.invalidInput("That automation was deleted.");
  const started = Date.now();
  let ok = true;
  let answer = "";
  try {
    answer = await quickAnswer(row.prompt, { maxTokens: 1200 });
    if (!answer) {
      ok = false;
      answer = "The model gave no answer back.";
    }
  } catch (e) {
    ok = false;
    answer = e?.message ?? String(e);
  }
  run(
    `INSERT INTO automation_runs (automation_id, name, at, ok, answer, ms) VALUES (?, ?, ?, ?, ?, ?)`,
    row.id, row.name, Math.floor(started / 1000), ok ? 1 : 0, answer.slice(0, 4000), Date.now() - started,
  );
  run(`UPDATE automations SET last_run_at = ? WHERE id = ?`, Math.floor(started / 1000), row.id);
  emitEvent("automations-changed", null);
  return ok;
}
