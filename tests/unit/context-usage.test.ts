import assert from "node:assert/strict";
import { test } from "node:test";
import type { ChatMessage } from "../../src/lib/api/bindings.ts";
import { compactionStart, contextUsage, fits, PROMPT_OVERHEAD_TOKENS } from "../../src/lib/contextUsage.ts";

const msg = (role: string, chars: number, extra: Partial<ChatMessage> = {}): ChatMessage => ({ role, content: "x".repeat(chars), ...extra });

test("usage counts what is sent: text, attached text, draft, overhead and reserve", () => {
  const messages = [
    msg("user", 400, { attachments: [{ name: "a.md", kind: "text", text: "y".repeat(800), chars: 800, truncated: false }] }),
    msg("assistant", 400),
    msg("assistant", 4000, { error: true }),
  ];
  const u = contextUsage(messages, 10_000, { draft: "z".repeat(40), reserve: 1000 });
  assert.equal(u.output, 104, "the failed answer is not counted");
  assert.equal(u.input, 104 + 200 + 104 + 10);
  assert.equal(u.overhead, PROMPT_OVERHEAD_TOKENS);
  assert.equal(u.used, u.input + u.overhead);
  assert.equal(u.total, u.used + 1000);
  assert.equal(u.percent, (u.used / 10_000) * 100, "the fill is what the chat uses, not the reserve");
  assert.equal(contextUsage(messages, null).percent, 0, "an unknown limit shows no fill");
});

test("compaction cuts the oldest turns until the rest fits, and keeps the last two", () => {
  const messages = Array.from({ length: 10 }, (_, i) => msg(i % 2 ? "assistant" : "user", 4000));
  const start = compactionStart(messages, 8000, 80);
  assert.ok(start > 0);
  assert.ok(fits(contextUsage(messages, 8000, { from: start }), 80));
  assert.ok(start <= 8);
  assert.equal(compactionStart(messages.slice(0, 2), 100_000, 80), 0, "a short chat is left alone");
  // Even when nothing fits, the newest two messages stay.
  assert.equal(compactionStart(messages, 1000, 50), 8);
});

test("an empty chat on a small model is not full, but a reserve that cannot fit is", () => {
  const empty = contextUsage([], 8192, { reserve: 2048 + 4096 });
  assert.ok(empty.percent < 15, "an empty chat shows little fill");
  assert.ok(!empty.overflow);
  const tight = contextUsage([msg("user", 8000)], 8192, { reserve: 6144 });
  assert.ok(tight.percent < 50 && tight.overflow, "half full, but the answer would not fit");
  assert.equal(compactionStart([msg("user", 8000), msg("assistant", 400), msg("user", 40)], 8192, 80, { reserve: 6144 }), 1);
});
