import assert from "node:assert/strict";
import { test } from "node:test";
import type { UsageSummary } from "../../src/lib/api/bindings.ts";
import { answerStreak, compactTokens, formatAnswerMs, windowDays } from "../../src/lib/usage.ts";

const day = (date: string, answers: number) => ({ date, answers, input: answers * 10, output: answers * 100 });

const summaryWith = (days: number, per_day: { date: string; answers: number }[]): UsageSummary => ({
  days,
  answers: per_day.length,
  cancelled: 0,
  input_tokens: 0,
  output_tokens: 0,
  ms: 0,
  tool_calls: 0,
  per_day: per_day.map((d) => day(d.date, d.answers)),
  per_model: [],
});

test("windowDays fills the whole window, oldest first, zeroing empty days", () => {
  // Wednesday, 2026-02-04 (a fixed date so the test never ages).
  const today = new Date(2026, 1, 4);
  const s = summaryWith(7, [{ date: "2026-02-01", answers: 3 }, { date: "2026-02-04", answers: 1 }]);
  const window = windowDays(s, today);
  assert.equal(window.length, 7);
  assert.equal(window[0].date, "2026-01-29");
  assert.equal(window[0].answers, 0);
  assert.equal(window.find((d) => d.date === "2026-02-01")?.answers, 3);
  assert.equal(window[6].date, "2026-02-04");
});

test("answerStreak counts back from today and forgives an empty today", () => {
  const s = summaryWith(7, [
    { date: "2026-01-30", answers: 1 },
    { date: "2026-01-31", answers: 1 },
    { date: "2026-02-01", answers: 1 },
    { date: "2026-02-02", answers: 0 },
  ]);
  const today = new Date(2026, 1, 4);
  const window = windowDays(s, today);
  // Today (02-04) is empty — that is fine, the streak ran to yesterday.
  assert.equal(answerStreak(window), 0);
  const withYesterday = summaryWith(7, [
    { date: "2026-02-02", answers: 2 },
    { date: "2026-02-03", answers: 2 },
    { date: "2026-02-04", answers: 0 },
  ]);
  assert.equal(answerStreak(windowDays(withYesterday, today)), 2);
});

test("compactTokens picks one unit", () => {
  assert.equal(compactTokens(0), "0");
  assert.equal(compactTokens(940), "940");
  assert.equal(compactTokens(1234), "1.2k");
  assert.equal(compactTokens(48211), "48k");
  assert.equal(compactTokens(2_500_000), "2.5M");
});

test("formatAnswerMs stays seconds under a minute, minutes above", () => {
  assert.equal(formatAnswerMs(0), "0 s");
  assert.equal(formatAnswerMs(6200), "6.2 s");
  assert.equal(formatAnswerMs(204_000), "3 m 24 s");
});
