// Usage: one JSONL line per finished answer (~/.aiassistant/usage.jsonl,
// the shape the Tauri build wrote), aggregated for the Usage screen, plus
// the usage ledger the caller can record flat-cost estimates into.

import fs from "node:fs";
import { appDir, usageLogPath } from "../core/paths.mjs";
import { run, get, all } from "../core/db.mjs";

/** UTC calendar date for a Unix timestamp, no calendar crate. */
function dateFromTs(secs) {
  const z = Math.floor(secs / 86400) + 719_468;
  const era = Math.floor(z / 146_097);
  const doe = z - era * 146_097;
  const yoe = Math.floor((doe - Math.floor(doe / 1460) + Math.floor(doe / 36524) - Math.floor(doe / 146096)) / 365);
  const y = yoe + era * 400;
  const doy = doe - (365 * yoe + Math.floor(yoe / 4) - Math.floor(yoe / 100));
  const mp = Math.floor((5 * doy + 2) / 153);
  const d = doy - Math.floor((153 * mp + 2) / 5) + 1;
  const m = mp < 10 ? mp + 3 : mp - 9;
  const year = m <= 2 ? y + 1 : y;
  return `${String(year).padStart(4, "0")}-${String(m).padStart(2, "0")}-${String(d).padStart(2, "0")}`;
}

/** Days since the epoch for a "YYYY-MM-DD" date; null when not a date. */
function dayFromDate(date) {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(date);
  if (!m) return null;
  let [y, mo, d] = [Number(m[1]), Number(m[2]), Number(m[3])];
  if (mo < 1 || mo > 12 || d < 1 || d > 31) return null;
  if (mo <= 2) y -= 1;
  const era = Math.floor(y / 400);
  const yoe = y - era * 400;
  const mp = (mo + 9) % 12;
  const doy = Math.floor((153 * mp + 2) / 5) + d - 1;
  const doe = yoe * 365 + Math.floor(yoe / 4) - Math.floor(yoe / 100) + doy;
  return era * 146_097 + doe - 719_468;
}

/**
 * Appends one finished answer. Never throws: usage tracking must not be
 * able to break a chat. Fields mirror the Rust UsageRecord.
 */
export function recordUsage(rec) {
  try {
    fs.mkdirSync(appDir(), { recursive: true });
    const line = JSON.stringify({
      ts: rec.ts ?? Math.floor(Date.now() / 1000),
      model: String(rec.model ?? ""),
      provider: String(rec.provider ?? "This computer"),
      local: rec.local !== false,
      input_tokens: Math.max(0, Math.round(rec.input_tokens ?? 0)),
      output_tokens: Math.max(0, Math.round(rec.output_tokens ?? 0)),
      ms: Math.max(0, Math.round(rec.ms ?? 0)),
      ttft_ms: rec.ttft_ms ?? null,
      tok_s: rec.tok_s ?? null,
      cancelled: rec.cancelled === true,
      tool_calls: Math.max(0, Math.round(rec.tool_calls ?? 0)),
      source: rec.source ?? "chat",
      folder: rec.folder ?? null,
      session: rec.session ?? rec.threadId ?? null,
      cache_tokens: rec.cache_tokens ?? null,
    });
    fs.appendFileSync(usageLogPath(), line + "\n");
  } catch {
    /* the answer itself must never fail because of the log */
  }
}

/** Aggregates the log over a window and, when set, one project folder. */
export function summary(days, folder) {
  const want = Math.min(36500, Math.max(0, Math.round(Number(days) || 0) || 30));
  const nowSecs = Math.floor(Date.now() / 1000);
  const cutoff =
    want === 0 ? 0 : want === 1 ? nowSecs - (nowSecs % 86400) : nowSecs - want * 86400;

  let answers = 0;
  let cancelled = 0;
  let input = 0;
  let output = 0;
  let cache = 0;
  let ms = 0;
  let toolCalls = 0;
  const perDay = new Map();
  const perModel = new Map();
  const perFolder = new Map();
  const sessions = new Set();
  let sessionless = 0;

  let text = "";
  try {
    text = fs.readFileSync(usageLogPath(), "utf8");
  } catch {
    text = "";
  }
  for (const line of text.split("\n")) {
    let rec;
    try {
      rec = JSON.parse(line);
    } catch {
      continue;
    }
    if (!rec || typeof rec.ts !== "number" || rec.ts < cutoff) continue;
    // Projects count in their own rollup even while the summary is filtered
    // to another project, so the picker stays complete.
    if (rec.folder) {
      const proj = perFolder.get(rec.folder) ?? { folder: rec.folder, answers: 0, input: 0, output: 0, ms: 0 };
      proj.answers += 1;
      proj.input += rec.input_tokens ?? 0;
      proj.output += rec.output_tokens ?? 0;
      proj.ms += rec.ms ?? 0;
      perFolder.set(rec.folder, proj);
    }
    if (folder && rec.folder !== folder) continue;
    answers += 1;
    if (rec.cancelled) cancelled += 1;
    input += rec.input_tokens ?? 0;
    output += rec.output_tokens ?? 0;
    cache += rec.cache_tokens ?? 0;
    ms += rec.ms ?? 0;
    toolCalls += rec.tool_calls ?? 0;
    if (rec.session) sessions.add(rec.session);
    else sessionless += 1;
    const date = dateFromTs(rec.ts);
    const day = perDay.get(date) ?? { date, answers: 0, input: 0, output: 0 };
    day.answers += 1;
    day.input += rec.input_tokens ?? 0;
    day.output += rec.output_tokens ?? 0;
    perDay.set(date, day);
    const key = `${rec.model ?? ""}|${rec.provider ?? ""}`;
    const acc = perModel.get(key) ?? {
      model: rec.model ?? "",
      provider: rec.provider ?? "",
      answers: 0, input: 0, output: 0, cache: 0, steps: 0, ms: 0, msN: 0, tokS: 0, tokSN: 0, ttft: 0, ttftN: 0,
    };
    acc.answers += 1;
    acc.input += rec.input_tokens ?? 0;
    acc.output += rec.output_tokens ?? 0;
    acc.cache += rec.cache_tokens ?? 0;
    acc.steps += rec.tool_calls ?? 0;
    acc.ms += rec.ms ?? 0;
    acc.msN += 1;
    if (typeof rec.tok_s === "number") { acc.tokS += rec.tok_s; acc.tokSN += 1; }
    if (typeof rec.ttft_ms === "number") { acc.ttft += rec.ttft_ms; acc.ttftN += 1; }
    perModel.set(key, acc);
  }

  // Longest run of consecutive active days, ending anywhere in the window.
  let longest = 0;
  let run = 0;
  let prev = null;
  for (const date of [...perDay.keys()].sort()) {
    const stamp = dayFromDate(date);
    if (stamp === null) continue;
    run = prev !== null && stamp === prev + 1 ? run + 1 : 1;
    longest = Math.max(longest, run);
    prev = stamp;
  }

  const perModelOut = [...perModel.values()]
    .map((a) => ({
      model: a.model,
      provider: a.provider,
      answers: a.answers,
      input: a.input,
      output: a.output,
      cache: a.cache,
      steps: a.steps,
      avg_ms: a.msN > 0 ? Math.round(a.ms / a.msN) : 0,
      avg_tok_s: a.tokSN > 0 ? a.tokS / a.tokSN : null,
      avg_ttft_ms: a.ttftN > 0 ? Math.round(a.ttft / a.ttftN) : null,
    }))
    .sort((a, b) => b.output - a.output || b.answers - a.answers);
  const perFolderOut = [...perFolder.values()].sort((a, b) => b.output - a.output || b.answers - a.answers);

  return {
    days: want,
    answers,
    cancelled,
    input_tokens: input,
    output_tokens: output,
    cache_tokens: cache,
    ms,
    tool_calls: toolCalls,
    sessions: sessions.size + sessionless,
    active_days: perDay.size,
    longest_streak: longest,
    per_day: [...perDay.values()],
    per_model: perModelOut,
    per_folder: perFolderOut,
  };
}

// ---- The usage ledger (usage_record / usage_list) --------------------

/** The flat estimate when the caller does not supply a rate, USD per 1M. */
const DEFAULT_FLAT_RATE_PER_MILLION_USD = 1.0;

const nowSecs = () => Math.floor(Date.now() / 1000);

const rowToEntry = (row) => ({
  id: row.id,
  provider: row.provider,
  model: row.model,
  input_tokens: row.input_tokens,
  output_tokens: row.output_tokens,
  estimated_cost_usd: row.estimated_cost_usd,
  at: row.at,
  thread_id: row.thread_id,
});

/** Record one call. The rate is caller-supplied; no price table is built in. */
export function ledgerAdd({ provider, model, inputTokens, outputTokens, threadId, ratePerMillionUsd }) {
  const rate = Number.isFinite(Number(ratePerMillionUsd)) && ratePerMillionUsd !== null
    ? Number(ratePerMillionUsd)
    : DEFAULT_FLAT_RATE_PER_MILLION_USD;
  const input = Math.max(0, Math.round(Number(inputTokens) || 0));
  const output = Math.max(0, Math.round(Number(outputTokens) || 0));
  const cost = ((input + output) / 1_000_000) * rate;
  const at = nowSecs();
  const info = run(
    `INSERT INTO usage_ledger (at, provider, model, input_tokens, output_tokens, estimated_cost_usd, thread_id)
     VALUES (?, ?, ?, ?, ?, ?, ?)`,
    at, String(provider ?? ""), String(model ?? ""), input, output, cost, String(threadId ?? ""),
  );
  return {
    id: Number(info.lastInsertRowid),
    provider: String(provider ?? ""),
    model: String(model ?? ""),
    input_tokens: input,
    output_tokens: output,
    estimated_cost_usd: cost,
    at,
    thread_id: String(threadId ?? ""),
  };
}

/** The ledger, newest first, optionally one chat's rows only. */
export function ledgerList(threadId, limit) {
  const keep = Math.max(1, Math.min(1000, Math.round(Number(limit) || 200)));
  if (threadId) {
    return all(
      `SELECT * FROM usage_ledger WHERE thread_id = ? ORDER BY at DESC, id DESC LIMIT ?`,
      String(threadId), keep,
    ).map(rowToEntry);
  }
  return all(`SELECT * FROM usage_ledger ORDER BY at DESC, id DESC LIMIT ?`, keep).map(rowToEntry);
}

/** One row by id, to confirm a write landed. */
export const ledgerGet = (id) => {
  const row = get(`SELECT * FROM usage_ledger WHERE id = ?`, Number(id));
  return row ? rowToEntry(row) : null;
};
