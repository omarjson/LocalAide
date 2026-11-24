import assert from "node:assert";
import test from "node:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const root = new URL("../..", import.meta.url).pathname;
const read = (p: string) => readFileSync(join(root, p), "utf8");

const stream = read("src-tauri/src/services/inference/openai_stream.rs");
const agent = read("src-tauri/src/services/agent/mod.rs");

// N1 — provider failover. The classification lives in the stream layer
// (openai_stream.rs); the chain and the switch live in the agent run
// (agent/mod.rs). The chain's ordering is covered by Rust unit tests in
// agent/mod.rs; these tests pin the wiring the Rust tests cannot see.

test("a retryable error is only 429 / 408 / 5xx, never a 4xx or a stop", () => {
  const m = stream.match(
    /if matches!\(status, ([^)]+)\) \{\s*err = err\.retryable\(\);/,
  );
  assert.ok(m, "a retryable classification exists");
  const codes = m[1].split("|").map((s) => s.trim());
  assert.deepEqual(codes.sort(), ["408", "429", "500", "502", "503", "504"]);
});

test("a network error and a stalled stream are retryable timeouts", () => {
  assert.match(
    stream,
    /Could not reach \{\}: \{e\}\. Check the address and your connection\."[\s\S]*?\.retryable\(\)/,
    "could-not-connect is retryable",
  );
  assert.match(
    stream,
    /\{\} stopped responding\. Try again\."[\s\S]*?\.retryable\(\)/,
    "a stalled remote stream is retryable",
  );
});

test("same-provider retries are capped small so failover can happen in time", () => {
  assert.match(
    stream,
    /const SAME_PROVIDER_RETRIES: u32 = 3;/,
    "three same-provider tries, not ten",
  );
  assert.match(
    stream,
    /tries < SAME_PROVIDER_RETRIES/,
    "the retry loop uses the cap",
  );
  assert.doesNotMatch(stream, /try \{tries\} of 10/, "no stale 'of 10' copy");
});

test("failover only fires on a retryable error before any token landed", () => {
  assert.match(
    agent,
    /Err\(e\) if e\.retryable && round_delivered == 0 =>/,
    "the failover arm requires a retryable error and zero delivered tokens",
  );
  assert.match(
    agent,
    /let mut round_delivered = 0usize;/,
    "the per-round delivery counter exists",
  );
  assert.match(
    agent,
    /round_delivered \+= 1;/,
    "every streamed delta counts as delivered",
  );
});

test("failover walks the user's provider order and ends on the local model", () => {
  assert.match(
    agent,
    /fn failover_chain\(/,
    "the chain is a pure, unit-tested function",
  );
  assert.match(
    agent,
    /chain\.push\(FailoverCandidate::Local\);/,
    "the local model is the last resort",
  );
  assert.match(
    agent,
    /\.filter\(\|id\| Some\(id\.as_str\(\)\) != current_id && !tried\.contains\(\*id\)\)/,
    "the current and already-tried providers are skipped",
  );
  assert.match(
    agent,
    /fn failover_chain_skips_current_and_tried_and_ends_local/,
    "Rust unit tests cover the chain",
  );
});

test("failover tells the user in one line and retries the same round", () => {
  assert.match(
    agent,
    /status\(sink, format!\("\{from\} was busy — continued on \{to\}"\)\);/,
    "the chat gets one plain line naming the switch",
  );
  assert.match(
    agent,
    /\/\/ Same round again, on the new provider\.\s*continue;/,
    "failover re-runs the same round, it does not start a new one",
  );
});

test("failover is fail-open and never sends a local chat to the cloud", () => {
  assert.match(
    agent,
    /let ChatTarget::Remote\(remote\) = current else \{\s*return None;/,
    "a local target never fails over",
  );
  assert.match(
    agent,
    /let Ok\(cfg\) = crate::commands::cloud::load_config\(state\) else \{\s*return None;/,
    "an unreadable config means no failover, not a crash",
  );
  assert.match(
    agent,
    /\/\/ Chain exhausted \(or failover itself broke\): the\s*\/\/ original error stands, unchanged\.\s*None => return Err\(e\),/,
    "when the chain is exhausted the original error stands",
  );
});

test("failover rebuilds the flavor-dependent options for the new provider", () => {
  assert.match(
    agent,
    /opts = initial_options\(\s*endpoint\.flavor,/,
    "options are rebuilt from the new endpoint's flavor",
  );
  assert.match(agent, /opts\.tools\.clone\(\),/, "the tools state is kept");
  assert.match(
    agent,
    /opts\.forbid_tools = forbid;/,
    "the last-round tool ban survives the rebuild",
  );
  assert.match(
    agent,
    /key = format!\("\{\}\|\{\}", endpoint\.url, endpoint\.model\);/,
    "the no-tools tracking key follows the new endpoint",
  );
});

// N2 — never stall-timeout local models. The stall budget exists, but it
// must not apply to the local server: a local model's silent thinking
// phase is healthy.

test("the stall budget never applies to the local server", () => {
  assert.match(
    stream,
    /endpoint\.flavor != Flavor::LlamaServer\s*&& last_activity\.elapsed\(\) > STREAM_STALL_LIMIT/,
    "the stall check exempts the local flavor",
  );
  assert.match(
    stream,
    /Never applied to the local server: a local model's silent thinking/,
    "the rule is written at the constant, not just at the check",
  );
  assert.match(stream, /`0` = disabled/, "0 means disabled, never a tiny budget");
});

// N3 — `__omit__` for reasoning_effort. No code change: the stream layer
// already drops the field when a provider refuses it, and the retry test
// proves the second request goes out without it.

test("reasoning_effort is already dropped when a provider refuses it (N3 needs no change)", () => {
  assert.match(
    stream,
    /if !matches!\(opts\.effort, Effort::Auto\) && mentions\(&\["reasoning", "effort", "thinking"\]\) \{\s*opts\.effort = Effort::Auto;\s*return Some\(Adjustment::DroppedEffort\);/,
    "adjust_for_error drops the effort when the provider names it",
  );
  assert.match(
    stream,
    /assert!\(second\.get\("reasoning_effort"\)\.is_none\(\)\);/,
    "the existing test proves the retry goes out without the field",
  );
});
