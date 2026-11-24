// Frontend helpers for the Usage screen: window building and number
// formatting. The aggregation itself happens in Rust (services/usage.rs).

import type { UsageDay, UsageSummary } from "./api/bindings";

/**
 * Every day of the summary window, oldest first, with zero-filled gaps.
 * Rust buckets by UTC day while `today` is local, so an answer logged
 * near midnight can sit one column off — fine for a dashboard.
 */
export function windowDays(summary: UsageSummary, today = new Date()): UsageDay[] {
  const byDate = new Map(summary.per_day.map((day) => [day.date, day]));
  const out: UsageDay[] = [];
  // All time (days 0) still needs a chart: the last 30 calendar days of it.
  const span = summary.days === 0 ? 30 : summary.days;
  for (let i = span - 1; i >= 0; i--) {
    const d = new Date(today.getFullYear(), today.getMonth(), today.getDate() - i);
    const date = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
    out.push(byDate.get(date) ?? { date, answers: 0, input: 0, output: 0 });
  }
  return out;
}

/** Consecutive days ending today (or yesterday) that had answers. */
export function answerStreak(days: UsageDay[]): number {
  let streak = 0;
  for (let i = days.length - 1; i >= 0; i--) {
    if (days[i].answers > 0) streak++;
    else if (i === days.length - 1) continue; // today may still be empty
    else break;
  }
  return streak;
}

/** 1234567 tokens reads better as "1.2M" on a card. */
export function compactTokens(n: number): string {
  if (!Number.isFinite(n) || n <= 0) return "0";
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 10_000) return `${Math.round(n / 1000)}k`;
  if (n >= 1_000) return `${(n / 1000).toFixed(1)}k`;
  return String(Math.round(n));
}

/** Answer wall time: "6.2 s" below a minute, "3 m 24 s" above. */
export function formatAnswerMs(ms: number): string {
  if (!Number.isFinite(ms) || ms <= 0) return "0 s";
  const s = ms / 1000;
  if (s < 60) return `${s.toFixed(1)} s`;
  const m = Math.floor(s / 60);
  return `${m} m ${Math.round(s - m * 60)} s`;
}

export interface WindowCoverage {
  /** First day the totals actually cover, "Mar 3" style. */
  from: string;
  /** Last day the totals cover, "Mar 3" style. */
  to: string;
  /** True when retained history starts after the requested window. */
  short: boolean;
}

const dayKey = (d: Date) =>
  `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
const dayLabel = (key: string) => {
  const [y, m, d] = key.split("-").map(Number);
  return new Date(y, m - 1, d).toLocaleDateString(undefined, { month: "short", day: "numeric" });
};

/**
 * What window the totals actually cover. The ledger only keeps days that
 * had answers, so when retained history is shorter than the requested
 * window the earliest kept day is later than the window start — and the
 * totals must say so instead of wearing the requested label. "All time"
 * claims no window, so it has no coverage to check. A total that silently
 * omits rows is a caveat, not a status update.
 */
export function windowCoverage(summary: UsageSummary, today = new Date()): WindowCoverage | null {
  if (summary.days === 0) return null;
  const start = new Date(today.getFullYear(), today.getMonth(), today.getDate() - (summary.days - 1));
  const startKey = dayKey(start);
  const earliest = summary.per_day.length > 0 ? [...summary.per_day.map((d) => d.date)].sort()[0] : null;
  if (!earliest || earliest <= startKey) return { from: dayLabel(startKey), to: dayLabel(dayKey(today)), short: false };
  return { from: dayLabel(earliest), to: dayLabel(dayKey(today)), short: true };
}
