import assert from "node:assert/strict";
import { describe, test } from "node:test";
import {
  buildDiagnosticReport,
  formatDiagBytes,
  redactSecrets,
  requestStats,
} from "../../src/lib/diagnostics.ts";
import type { RequestEntry } from "../../src/lib/requestLog.svelte.ts";

import type { SystemHealthResponse } from "../../src/lib/api/bindings.ts";

const entry = (id: number, command: string, ms: number, ok = true): RequestEntry => ({
  id, command, ms, ok, error: ok ? null : "boom", at: 1_700_000_000_000 + id,
});

const health: SystemHealthResponse = {
  app_version: "0.2.1",
  catalog_version: 5,
  engine: { loaded: false, active_model: null, context_size: 0, sidecar_binary_found: true },
  embedding: { ready: false, model_id: null, dimension: null, total_chunks: 0, embedded_chunks: 0 },
  rerank: { ready: false, model_id: null },
  library: { doc_count: 0 },
  performance: { uptime_secs: 120, rss_bytes: 2 * 1024 * 1024, logs_bytes: 4096, library_db_bytes: 0 },
  config_ok: true,
};

describe("request statistics", () => {
  test("empty input is honest", () => {
    assert.deepEqual(requestStats([]), {
      count: 0, failed: 0, avgMs: 0, p95Ms: 0, maxMs: 0, slowest: null,
    });
  });

  test("p95 ignores the single worst call in a large sample", () => {
    const entries = Array.from({ length: 19 }, (_, i) => entry(i + 1, "normal", 10 + i));
    entries.push(entry(20, "bad", 5000, false));
    const stats = requestStats(entries);
    assert.equal(stats.count, 20);
    assert.equal(stats.failed, 1);
    assert.equal(stats.maxMs, 5000);
    assert.ok(stats.p95Ms < 1000, "one slow call must not dominate p95");
    assert.equal(stats.slowest?.command, "bad");
  });
});

test("byte formatting stays readable", () => {
  assert.equal(formatDiagBytes(0), "0 B");
  assert.equal(formatDiagBytes(512), "512 B");
  assert.equal(formatDiagBytes(2 * 1024 * 1024), "2.0 MB");
});

test("secrets are redacted before the report is copied", () => {
  const redacted = redactSecrets([
    "Authorization: Bearer sk-abcdefghijklmnopqrstuvwxyz",
    'api_key: "sk-test-0123456789abcdef"',
    "password=hunter2; next",
  ].join("\n"));
  assert.doesNotMatch(redacted, /abcdefghijklmnop/);
  assert.doesNotMatch(redacted, /hunter2/);
  assert.match(redacted, /\[REDACTED\]/);
});

test("the report includes health, timings and both log tails", () => {
  const report = buildDiagnosticReport({
    health,
    platform: "mac",
    entries: [entry(1, "chat_completion", 900)],
    appLog: ["ready", "token abcdefghijklmnopqrstuvwxyz"],
    engineLog: ["model loaded"],
  });
  assert.match(report, /LocalAide diagnostics/);
  assert.match(report, /0\.2\.1/);
  assert.match(report, /chat_completion/);
  assert.match(report, /2\.0 MB/);
  assert.match(report, /model loaded/);
  assert.match(report, /never uploads it/);
});
