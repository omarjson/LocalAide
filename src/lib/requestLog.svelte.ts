// Developer → Request log: the last backend calls with their timings.
// Always measured in memory (a tiny ring buffer), so a slowdown can be
// diagnosed after the fact; only the display lives behind a developer flag.
// Nothing leaves the app unless the user copies the diagnostics report.

export interface RequestEntry {
  id: number;
  command: string;
  ms: number;
  ok: boolean;
  error: string | null;
  at: number;
}

const LIMIT = 200;
let next = 1;

export const requestLog = $state<{ entries: RequestEntry[] }>({ entries: [] });

export function recordRequest(command: string, ms: number, ok: boolean, error?: unknown): void {
  const message = ok
    ? null
    : typeof error === "string"
      ? error
      : error && typeof error === "object" && "message" in error && typeof error.message === "string"
        ? error.message
        : "failed";
  requestLog.entries = [{ id: next++, command, ms, ok, error: message, at: Date.now() }, ...requestLog.entries].slice(0, LIMIT);
}

export function clearRequestLog(): void {
  requestLog.entries = [];
}
