// Developer → Request log: the last backend calls with their timings.
// Records only while the "Request log" flag is on; nothing leaves the app.

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

export const requestLog = $state<{ enabled: boolean; entries: RequestEntry[] }>({
  enabled: false,
  entries: [],
});

export function recordRequest(command: string, ms: number, ok: boolean, error?: unknown): void {
  if (!requestLog.enabled) return;
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
