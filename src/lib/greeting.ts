// The empty chat's one-line greeting. Time of day first, and "nice work
// today" only in the evening and only when a chat actually had a turn
// today — the line must never claim work that did not happen.

/** Morning / afternoon / evening, with the late-night line after 23:00. */
export function greetingAt(now: Date, workedToday: boolean): string {
  const hour = now.getHours();
  if (hour < 5) return "Working late?";
  if (hour < 12) return "Good morning.";
  if (hour < 18) return "Good afternoon.";
  if (hour < 23) return workedToday ? "Good evening. Nice work today." : "Good evening.";
  return "It's late — take care of yourself.";
}

/** True when any saved chat was updated today (local calendar day). */
export function workedToday(threads: { updated_at: number }[], now = new Date()): boolean {
  const startOfDay = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime() / 1000;
  return threads.some((t) => t.updated_at >= startOfDay);
}
