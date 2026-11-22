/** Pure helpers for the Settings → About diagnostics panel and its export. */

import type { SystemHealthResponse } from "./api/bindings";
import type { RequestEntry } from "./requestLog.svelte";

export interface RequestStats {
  count: number;
  failed: number;
  avgMs: number;
  p95Ms: number;
  maxMs: number;
  slowest: RequestEntry | null;
}

export function requestStats(entries: readonly RequestEntry[]): RequestStats {
  if (entries.length === 0) {
    return { count: 0, failed: 0, avgMs: 0, p95Ms: 0, maxMs: 0, slowest: null };
  }
  const sorted = entries.map((e) => e.ms).sort((a, b) => a - b);
  const slowest = entries.reduce((a, b) => (b.ms > a.ms ? b : a));
  return {
    count: entries.length,
    failed: entries.filter((e) => !e.ok).length,
    avgMs: sorted.reduce((sum, ms) => sum + ms, 0) / sorted.length,
    p95Ms: sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * 0.95) - 1)],
    maxMs: sorted[sorted.length - 1],
    slowest,
  };
}

/** Byte counts in the report, independent of the UI formatter. */
export function formatDiagBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
  const value = bytes / 1024 ** i;
  return `${value >= 10 || i === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[i]}`;
}

/**
 * Best-effort scrub before a report is pasted into a forum or issue. Replaces
 * common key shapes and bearer tokens; the user still reviews the result.
 */
export function redactSecrets(text: string): string {
  return text
    .replace(/\b(sk|pk)-[A-Za-z0-9_-]{12,}/g, "$1-[REDACTED]")
    .replace(/\b(Bearer\s+)[A-Za-z0-9._~+/=-]{8,}/gi, "$1[REDACTED]")
    .replace(
      /\b((?:api[_-]?key|access[_-]?token|refresh[_-]?token|secret|password|authorization)\s*[:=]\s*["']?)[^\s"',;}\]]+/gi,
      "$1[REDACTED]",
    );
}

export interface DiagnosticInput {
  health: SystemHealthResponse | null;
  platform: string;
  entries: readonly RequestEntry[];
  appLog: readonly string[];
  engineLog: readonly string[];
}

/** A markdown report the user can copy and review before sharing. */
export function buildDiagnosticReport(input: DiagnosticInput): string {
  const now = new Date().toISOString();
  const stats = requestStats(input.entries);
  const perf = input.health?.performance;
  const lines: string[] = [
    "# LocalAide diagnostics",
    "",
    `Generated: ${now}`,
    `App: ${input.health?.app_version ?? "unknown"}`,
    `Platform: ${input.platform}`,
    "",
    "## Performance",
    `- Session uptime: ${perf ? `${perf.uptime_secs}s` : "unavailable"}`,
    `- LocalAide resident memory: ${perf ? formatDiagBytes(perf.rss_bytes) : "unavailable"}`,
    `- Logs on disk: ${perf ? formatDiagBytes(perf.logs_bytes) : "unavailable"}`,
    `- Library database: ${perf ? formatDiagBytes(perf.library_db_bytes) : "unavailable"}`,
    `- Backend calls recorded: ${stats.count} (${stats.failed} failed)`,
    `- Mean / p95 / max: ${stats.avgMs.toFixed(0)} / ${stats.p95Ms.toFixed(0)} / ${stats.maxMs.toFixed(0)} ms`,
    stats.slowest ? `- Slowest call: \`${stats.slowest.command}\` — ${stats.slowest.ms.toFixed(0)} ms` : "- Slowest call: none",
    "",
    "## System health",
    "```json",
    redactSecrets(JSON.stringify(input.health ?? { error: "unavailable" }, null, 2)),
    "```",
    "",
    "## Slowest backend calls (up to 20)",
    "| Time | Call | ms | OK |",
    "| --- | --- | ---: | :---: |",
    ...[...input.entries]
      .sort((a, b) => b.ms - a.ms)
      .slice(0, 20)
      .map((e) => `| ${new Date(e.at).toISOString()} | \`${e.command}\` | ${e.ms.toFixed(0)} | ${e.ok ? "yes" : "no"} |`),
    "",
    "## Local log tails (redacted, 120 lines each)",
    "### App log",
    "```text",
    redactSecrets(input.appLog.join("\n")) || "(empty)",
    "```",
    "### Engine log",
    "```text",
    redactSecrets(input.engineLog.join("\n")) || "(empty)",
    "```",
    "",
    "Review this report before sharing. It is generated locally; LocalAide never uploads it.",
  ];
  return lines.join("\n");
}
