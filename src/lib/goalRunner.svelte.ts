// Bounded, local goal planner + one-turn runner.
//
// The planning half is pure and mirrors services/goal.rs (step_prompt,
// decide_next, drive) using only values already on the Goal DTO: objective,
// budget, status and thread id. It never calls a model. The running half uses
// the existing chat stream API (api.streamChat) and waits for its `done`/`error`
// ChatStreamEvent — there are no new Tauri events and no fabricated answers.
//
// This module is deliberately free of top-level runtime imports so the planner
// can be exercised by the plain Node unit tests (which have no Tauri window).
// The API client is loaded lazily, only when a turn is actually run.

import type {
  ChatStreamEvent,
  ChatStreamRequest,
  ChatTarget,
  Goal,
} from "./api/bindings";

/** Why a bounded goal loop stopped (matches services/goal.rs StopReason). */
export type StopReason = "budget_exhausted" | "blocked" | "paused" | "completed" | "abandoned";

/** The next action for a goal (matches services/goal.rs StepDecision). */
export type StepDecision =
  | { readonly kind: "continue"; readonly prompt: string; readonly budgetLeft: number }
  | { readonly kind: "stop"; readonly reason: StopReason };

/** Result of a pure bounded drive over queued progress lines. */
export interface DriveResult {
  goal: Goal;
  applied: number;
  decision: StepDecision;
}

/**
 * Build one step's prompt from the objective, progress so far and budget.
 * Pure mirror of `step_prompt`: no database, model or clock.
 */
export function stepPrompt(
  goal: Pick<Goal, "objective" | "budget">,
  steps: readonly string[],
): string {
  let out = `Goal: ${goal.objective}\n`;
  if (steps.length === 0) {
    out += "No progress recorded yet.\n";
  } else {
    out += "Progress so far:\n";
    for (const step of steps) out += `- ${step}\n`;
  }
  out += `Steps left: ${goal.budget}. Take the next step, or say the goal is done.`;
  return out;
}

/**
 * Decide the next action from a goal's state. A blocked report or a finished
 * goal stops the loop even when budget remains; completion is never inferred
 * from budget alone. Pure mirror of `decide_next`.
 */
export function decideNext(
  goal: Goal,
  steps: readonly string[],
  blocked: boolean,
): StepDecision {
  if (goal.status === "completed") return { kind: "stop", reason: "completed" };
  if (goal.status === "abandoned") return { kind: "stop", reason: "abandoned" };
  // A paused or blocked goal only moves again after an explicit resume.
  if (goal.status === "paused") return { kind: "stop", reason: "paused" };
  if (goal.status === "blocked") return { kind: "stop", reason: "blocked" };
  if (blocked) return { kind: "stop", reason: "blocked" };
  if (goal.budget <= 0) return { kind: "stop", reason: "budget_exhausted" };
  return {
    kind: "continue",
    prompt: stepPrompt(goal, steps),
    budgetLeft: goal.budget,
  };
}

/** Planner entry point: Goal + progress + blocked flag -> decision and prompt. */
export const plan = decideNext;

/**
 * Pure bounded loop mirroring `drive`: consume at most one queued progress line
 * per remaining budget unit, stopping at once when blocked or finished. No
 * model and no database are involved; the caller persists each step itself.
 */
export function drive(
  goal: Goal,
  queued: readonly string[],
  blocked: boolean,
): DriveResult {
  const current: Goal = { ...goal };
  let applied = 0;
  const steps: string[] = [];
  if (!blocked) {
    for (const progress of queued) {
      // Unknown statuses are treated as active, matching GoalStatus::parse.
      // Paused and blocked goals take no steps until resumed.
      if (
        current.status === "completed" ||
        current.status === "abandoned" ||
        current.status === "paused" ||
        current.status === "blocked" ||
        current.budget <= 0
      ) {
        break;
      }
      current.budget -= 1;
      applied += 1;
      steps.push(progress);
    }
  }
  const decision: StepDecision = blocked
    ? { kind: "stop", reason: "blocked" }
    : decideNext(current, steps, false);
  // The state machine moves on its own when progress stops: a blocked
  // turn waits as Blocked, a used-up budget waits as Paused. The caller
  // persists the returned goal.
  if (current.status !== "completed" && current.status !== "abandoned" && current.status !== "paused" && current.status !== "blocked") {
    if (blocked) current.status = "blocked";
    else if (decision.kind === "stop" && decision.reason === "budget_exhausted") current.status = "paused";
  }
  return { goal: current, applied, decision };
}

/** A short, single-line summary derived only from text the model actually sent. */
export function summarizeAnswer(answer: string, maxChars = 240): string {
  const clean = (answer ?? "").replace(/\s+/g, " ").trim();
  if (!clean) return "";
  return clean.length <= maxChars ? clean : `${clean.slice(0, maxChars - 1).trimEnd()}…`;
}

/** The slice of the API client this runner needs (injectable, and lazy by default). */
export interface GoalChatClient {
  streamChat(
    req: ChatStreamRequest,
    onEvent: (event: ChatStreamEvent) => void,
  ): Promise<string>;
  goalStep(id: string, progress: string): Promise<Goal>;
  cancelChat?(jobId: string): Promise<void>;
}

export interface AdvanceGoalOptions {
  goal: Goal;
  /** Progress lines recorded so far, used to build the step prompt. */
  steps?: readonly string[];
  blocked?: boolean;
  /** Model label for the request; empty means the app's active model. */
  model?: string;
  target?: ChatTarget | null;
  /** Defaults to the real API client; tests can inject a client. */
  client?: GoalChatClient;
  onStatus?: (label: string) => void;
  /** Stop button: cancels the in-flight chat job once its id is known. */
  signal?: AbortSignal;
}

export interface AdvanceResult {
  /** Latest known goal; unchanged when no step was recorded. */
  goal: Goal;
  /** A model turn was started. */
  ran: boolean;
  /** A goal step was recorded from the answer. */
  recorded: boolean;
  /** The summary passed to goalStep, or "" when none was recorded. */
  summary: string;
  /** The answer's full text, so the caller can tell a refused permission
   * ("The user declined this action") from an empty answer. */
  answer: string;
  /** Set when the planner stopped before running any turn. */
  reason?: StopReason;
}

/**
 * Run exactly one model turn for a goal: send the step prompt through the
 * existing chat stream, wait for its existing `done`/`error` event, then record
 * a short summary of the real answer with goalStep. Completion notification is
 * left to the existing api.goalComplete path — no new notification code.
 */
export async function advanceGoal(
  options: AdvanceGoalOptions,
): Promise<AdvanceResult> {
  const { goal, steps = [], blocked = false } = options;
  const decision = decideNext(goal, steps, blocked);
  if (decision.kind === "stop") {
    return { goal, ran: false, recorded: false, summary: "", answer: "", reason: decision.reason };
  }

  const client: GoalChatClient = options.client ?? (await import("./api/client")).api;

  let answer = "";
  let finishReason: string | null = null;
  let settled = false;
  let resolveDone!: (value: string) => void;
  let rejectDone!: (error: Error) => void;
  const completed = new Promise<string>((resolve, reject) => {
    resolveDone = resolve;
    rejectDone = reject;
  });
  const onEvent = (event: ChatStreamEvent): void => {
    if (event.type === "token") {
      answer += event.payload.token;
    } else if (event.type === "status") {
      options.onStatus?.(event.payload.label);
    } else if (event.type === "done") {
      finishReason = event.payload.finish_reason;
      if (!settled) {
        settled = true;
        resolveDone(answer);
      }
    } else if (event.type === "error") {
      if (!settled) {
        settled = true;
        rejectDone(new Error(event.payload.error));
      }
    }
  };

  const req: ChatStreamRequest = {
    message: decision.prompt,
    model: options.model ?? "",
    thread_id: goal.thread_id,
    target: options.target ?? undefined,
  };

  const jobId = await client.streamChat(req, onEvent);
  const onAbort = (): void => {
    if (client.cancelChat) void client.cancelChat(jobId).catch(() => undefined);
  };
  if (options.signal) {
    if (options.signal.aborted) onAbort();
    else options.signal.addEventListener("abort", onAbort, { once: true });
  }

  let finalAnswer: string;
  try {
    finalAnswer = await completed;
  } finally {
    options.signal?.removeEventListener("abort", onAbort);
  }

  if (finishReason === "cancelled" || options.signal?.aborted === true) {
    return { goal, ran: true, recorded: false, summary: "", answer: finalAnswer };
  }

  const summary = summarizeAnswer(finalAnswer);
  if (!summary) return { goal, ran: true, recorded: false, summary: "", answer: finalAnswer };

  const updated = await client.goalStep(goal.id, summary);
  return { goal: updated, ran: true, recorded: true, summary, answer: finalAnswer };
}
