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
  for (let i = summary.days - 1; i >= 0; i--) {
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
