// Pure goal-planner tests. No Tauri window, no Svelte compiler, no model:
// these exercise the bounded planner that mirrors services/goal.rs using only
// values on the Goal DTO. Progress lines here are recorded step text, never
// fabricated model answers.
import assert from "node:assert/strict";
import { test } from "node:test";
import type { Goal } from "../../src/lib/api/bindings.ts";
import {
  decideNext,
  drive,
  plan,
  stepPrompt,
  summarizeAnswer,
  type StopReason,
} from "../../src/lib/goalRunner.svelte.ts";

const goal = (over: Partial<Goal> = {}): Goal => ({
  id: "g1",
  thread_id: "t1",
  objective: "ship it",
  budget: 3,
  status: "active",
  created_at: 1,
  ...over,
});

// --- prompt formatting ------------------------------------------------------

test("step_prompt with no progress names the objective and the budget", () => {
  assert.equal(
    stepPrompt(goal(), []),
    "Goal: ship it\nNo progress recorded yet.\n" +
      "Steps left: 3. Take the next step, or say the goal is done.",
  );
});

test("step_prompt lists recorded progress oldest first", () => {
  assert.equal(
    stepPrompt(goal(), ["drafted the outline", "filled in the numbers"]),
    "Goal: ship it\nProgress so far:\n" +
      "- drafted the outline\n" +
      "- filled in the numbers\n" +
      "Steps left: 3. Take the next step, or say the goal is done.",
  );
});

test("step_prompt reflects the current budget, not a stored one", () => {
  const prompt = stepPrompt(goal({ budget: 1 }), ["one step left"]);
  assert.match(prompt, /Steps left: 1\./);
  assert.doesNotMatch(prompt, /Steps left: 3\./);
});

// --- decide_next: budget, blocked, completion -------------------------------

test("decide_next offers the prompt while budget remains and goal is active", () => {
  const decision = decideNext(goal({ budget: 2 }), ["drafted outline"], false);
  assert.equal(decision.kind, "continue");
  if (decision.kind === "continue") {
    assert.equal(decision.budgetLeft, 2);
    assert.match(decision.prompt, /Goal: ship it/);
    assert.match(decision.prompt, /- drafted outline/);
  }
});

test("decide_next stops on an empty budget even with no blocker", () => {
  assert.deepEqual(decideNext(goal({ budget: 0 }), [], false), {
    kind: "stop",
    reason: "budget_exhausted",
  });
  assert.deepEqual(decideNext(goal({ budget: -1 }), [], false), {
    kind: "stop",
    reason: "budget_exhausted",
  });
});

test("decide_next stops on blocked before considering budget", () => {
  assert.deepEqual(decideNext(goal({ budget: 5 }), [], true), {
    kind: "stop",
    reason: "blocked",
  });
});

test("decide_next stops on completion and abandonment regardless of budget", () => {
  assert.deepEqual(decideNext(goal({ status: "completed", budget: 5 }), [], false), {
    kind: "stop",
    reason: "completed",
  });
  assert.deepEqual(decideNext(goal({ status: "abandoned", budget: 5 }), [], false), {
    kind: "stop",
    reason: "abandoned",
  });
});

test("completion takes priority over a blocked flag", () => {
  assert.deepEqual(decideNext(goal({ status: "completed" }), [], true), {
    kind: "stop",
    reason: "completed",
  });
});

test("plan is the decision entry point", () => {
  assert.deepEqual(plan(goal(), [], false), decideNext(goal(), [], false));
});

// --- drive: bounded planner loop --------------------------------------------

test("drive applies at most one queued line per budget unit", () => {
  const result = drive(goal({ budget: 2 }), ["a", "b", "c", "d"], false);
  assert.equal(result.applied, 2);
  assert.equal(result.goal.budget, 0);
  assert.deepEqual(result.decision, { kind: "stop", reason: "budget_exhausted" });
});

test("drive stops immediately when blocked, spending no budget", () => {
  const result = drive(goal({ budget: 3 }), ["a", "b"], true);
  assert.equal(result.applied, 0);
  assert.equal(result.goal.budget, 3);
  assert.deepEqual(result.decision, { kind: "stop", reason: "blocked" });
});

test("drive stops before applying anything to a completed goal", () => {
  const result = drive(goal({ status: "completed", budget: 3 }), ["a"], false);
  assert.equal(result.applied, 0);
  assert.equal(result.goal.budget, 3);
  assert.deepEqual(result.decision, { kind: "stop", reason: "completed" });
});

// --- the loop itself: repeated decisions converge on a stop ------------------

test("looping decide_next runs exactly budget turns then stops", () => {
  let current = goal({ budget: 3 });
  const prompts: string[] = [];
  let reason: StopReason | null = null;
  for (let turn = 0; turn < 10; turn++) {
    const decision = decideNext(current, [], false);
    if (decision.kind === "stop") {
      reason = decision.reason;
      break;
    }
    prompts.push(decision.prompt);
    // A recorded step consumes one unit of budget (what goalStep does).
    current = { ...current, budget: current.budget - 1 };
  }
  assert.equal(prompts.length, 3);
  assert.equal(reason, "budget_exhausted");
});

test("looping decide_next stops early when a blocker is reported", () => {
  let current = goal({ budget: 5 });
  let prompts = 0;
  let reason: StopReason | null = null;
  for (let turn = 0; turn < 10; turn++) {
    const decision = decideNext(current, [], turn === 2);
    if (decision.kind === "stop") {
      reason = decision.reason;
      break;
    }
    prompts++;
    current = { ...current, budget: current.budget - 1 };
  }
  assert.equal(prompts, 2);
  assert.equal(reason, "blocked");
  assert.equal(current.budget, 3, "a blocked check spends no budget");
});

test("looping decide_next stops as soon as the goal is completed", () => {
  let current = goal({ budget: 5 });
  let prompts = 0;
  let reason: StopReason | null = null;
  for (let turn = 0; turn < 10; turn++) {
    const decision = decideNext(current, [], false);
    if (decision.kind === "stop") {
      reason = decision.reason;
      break;
    }
    prompts++;
    current = { ...current, status: "completed" };
  }
  assert.equal(prompts, 1);
  assert.equal(reason, "completed");
  assert.equal(current.budget, 5, "completion is not inferred from budget");
});

// --- summary derived only from received text --------------------------------

test("summarizeAnswer collapses whitespace and truncates with an ellipsis", () => {
  assert.equal(summarizeAnswer("  drafted   the outline\nnext line  "), "drafted the outline next line");
  assert.equal(summarizeAnswer(""), "");
  const long = summarizeAnswer("x".repeat(300), 10);
  assert.equal(long.length, 10);
  assert.ok(long.endsWith("…"));
});
