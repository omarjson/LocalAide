<script lang="ts">
  // Developer tools for the local goal store (commands/goal.rs), plus an
  // optional model-backed runner. The planner in lib/goalRunner.svelte.ts is
  // pure; the runner uses the existing chat stream and invents no events.
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import StatusBadge from "./StatusBadge.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import type { Goal } from "../lib/api/bindings";
  import { advanceGoal, decideNext, type StopReason } from "../lib/goalRunner.svelte";

  let objective = $state("");
  let threadId = $state("");
  let budget = $state(5);
  let lookupId = $state("");
  let stepText = $state("");
  let goal = $state<Goal | null>(null);
  let notice = $state<{ ok: boolean; text: string } | null>(null);
  let busy = $state(false);

  // Model-backed runner state. Progress recorded by this runner is local: the
  // goal DTO has no steps field, so the prompt is built from what we just did.
  const MAX_RUN_TURNS = 12;
  let blocked = $state(false);
  let runnerBusy = $state(false);
  let runnerStatus = $state("");
  let runnerPrompt = $state("");
  let runnerSteps = $state<string[]>([]);
  let stopRequested = $state(false);
  let stopController: AbortController | null = null;

  const stopLabels: Record<StopReason, string> = {
    budget_exhausted: "Step budget used up.",
    blocked: "Goal is marked blocked.",
    completed: "Goal is already completed.",
    abandoned: "Goal was abandoned.",
  };

  const settled = (g: Goal, text: string) => {
    goal = g;
    lookupId = g.id;
    notice = { ok: true, text };
    runnerSteps = [];
    runnerPrompt = "";
  };

  const start = async () => {
    if (!objective.trim() || !threadId.trim()) {
      notice = { ok: false, text: "Enter an objective and a thread id first." };
      return;
    }
    busy = true;
    notice = null;
    try {
      const g = await api.goalStart(
        threadId.trim(),
        objective.trim(),
        Math.max(0, Math.round(budget)),
      );
      settled(g, `Goal ${g.id} started with ${g.budget} steps.`);
      objective = "";
      stepText = "";
    } catch (e: unknown) {
      notice = { ok: false, text: errorMessage(e) };
    } finally {
      busy = false;
    }
  };

  const lookup = async () => {
    if (!lookupId.trim()) {
      notice = { ok: false, text: "Enter a goal id to look up." };
      return;
    }
    busy = true;
    notice = null;
    try {
      const found = await api.goalStatus(lookupId.trim());
      if (found) {
        goal = found;
        runnerSteps = [];
        runnerPrompt = "";
        notice = { ok: true, text: `Loaded ${found.id}.` };
      } else {
        goal = null;
        notice = { ok: false, text: "There is no goal with that id on this computer." };
      }
    } catch (e: unknown) {
      notice = { ok: false, text: errorMessage(e) };
    } finally {
      busy = false;
    }
  };

  const step = async () => {
    if (!goal || !stepText.trim()) {
      notice = { ok: false, text: "Write what happened in this step first." };
      return;
    }
    busy = true;
    notice = null;
    try {
      const g = await api.goalStep(goal.id, stepText.trim());
      settled(g, `Step recorded. ${g.budget} ${g.budget === 1 ? "step" : "steps"} left.`);
      stepText = "";
    } catch (e: unknown) {
      notice = { ok: false, text: errorMessage(e) };
    } finally {
      busy = false;
    }
  };

  const complete = async () => {
    if (!goal) return;
    busy = true;
    notice = null;
    try {
      const g = await api.goalComplete(goal.id);
      settled(g, `Goal ${g.id} is completed.`);
    } catch (e: unknown) {
      notice = { ok: false, text: errorMessage(e) };
    } finally {
      busy = false;
    }
  };

  // One model turn. Returns true only when a step was actually recorded, so
  // a stop on blocked / completion / error / cancellation needs no extra flag.
  const runTurn = async (controller: AbortController): Promise<boolean> => {
    if (!goal) return false;
    const decision = decideNext(goal, runnerSteps, blocked);
    if (decision.kind === "stop") {
      runnerPrompt = "";
      runnerStatus = stopLabels[decision.reason];
      return false;
    }
    runnerPrompt = decision.prompt;
    runnerStatus = "Running the next step with the active chat model…";
    const result = await advanceGoal({
      goal,
      steps: runnerSteps,
      blocked,
      onStatus: (label) => {
        runnerStatus = label;
      },
      signal: controller.signal,
    });
    if (result.recorded) {
      runnerSteps = [...runnerSteps, result.summary];
      goal = result.goal;
      return true;
    }
    runnerStatus = stopRequested || controller.signal.aborted
      ? "Stopped by you."
      : "Stopped: the answer had no progress to record.";
    return false;
  };

  // "Run next step": exactly one model turn.
  const runNext = async () => {
    if (!goal || runnerBusy) return;
    runnerBusy = true;
    stopRequested = false;
    const controller = new AbortController();
    stopController = controller;
    try {
      const recorded = await runTurn(controller);
      if (recorded) {
        const current = goal;
        if (current && current.status === "completed") {
          runnerStatus = "Goal completed.";
        } else if (current) {
          const next = decideNext(current, runnerSteps, blocked);
          runnerStatus = next.kind === "stop"
            ? `Step recorded. ${stopLabels[next.reason]}`
            : "Step recorded. Ready for the next step.";
        }
      }
    } catch (e: unknown) {
      runnerStatus = `Stopped: ${errorMessage(e)}`;
    } finally {
      runnerBusy = false;
      stopController = null;
    }
  };

  // "Run until done": keep going while budget remains and the goal is active,
  // with a hard cap. Stops on blocked, completion, error, cancel or no budget.
  const runAll = async () => {
    if (!goal || runnerBusy) return;
    runnerBusy = true;
    stopRequested = false;
    const controller = new AbortController();
    stopController = controller;
    let turns = 0;
    try {
      while (turns < MAX_RUN_TURNS && !stopRequested && goal && goal.status === "active") {
        const recorded = await runTurn(controller);
        if (!recorded) return;
        turns += 1;
      }
      if (!stopRequested) {
        if (goal?.status === "completed") runnerStatus = "Goal completed.";
        else runnerStatus = turns >= MAX_RUN_TURNS
          ? `Stopped: reached the ${MAX_RUN_TURNS}-turn safety cap.`
          : "Stopped.";
      }
    } catch (e: unknown) {
      runnerStatus = `Stopped: ${errorMessage(e)}`;
    } finally {
      runnerBusy = false;
      stopController = null;
    }
  };

  const stopRun = () => {
    stopRequested = true;
    runnerStatus = "Stopping after this answer…";
    stopController?.abort();
  };
</script>

<section class="mt-9" aria-labelledby="goal-panel-title">
  <h3 id="goal-panel-title" class="mb-2 flex items-center gap-2 text-[15px] font-semibold text-ink-50">
    <Icon name="target" class="h-4 w-4 text-ember-300" />Goals
  </h3>
  <p class="mb-3 text-sm text-ink-400">
    Track a multi-step objective in the local goals database. The runner can use the active chat model to
    take the next step for you; manual steps still record progress text you write.
  </p>

  {#if notice}
    <p class={`mb-3 flex items-center gap-2 text-sm ${notice.ok ? "text-emerald-300" : "text-rose-300"}`} role="status">
      <Icon name={notice.ok ? "check" : "alert"} class="h-4 w-4 shrink-0" />{notice.text}
    </p>
  {/if}

  <div class="rounded-lg border border-ink-700 bg-ink-900 p-4">
    <div class="grid gap-3 sm:grid-cols-[1fr_12rem_7rem]">
      <label class="text-sm text-ink-400">Objective
        <input bind:value={objective} placeholder="What are you working towards?" class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 text-sm text-ink-100 outline-none placeholder:text-ink-500 focus:border-edge" />
      </label>
      <label class="text-sm text-ink-400">Thread id
        <input bind:value={threadId} placeholder="chat id" class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 font-mono text-sm text-ink-100 outline-none placeholder:text-ink-500 focus:border-edge" />
      </label>
      <label class="text-sm text-ink-400">Steps
        <input type="number" min="0" max="10000" bind:value={budget} class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 text-sm tabular-nums text-ink-100 outline-none focus:border-edge" />
      </label>
    </div>
    <div class="mt-3 flex justify-end"><Button variant="primary" disabled={busy} onclick={() => void start()}>Start goal</Button></div>
  </div>

  <div class="mt-3 rounded-lg border border-ink-700 bg-ink-900 p-4">
    <label class="text-sm text-ink-400">Look up a goal by id
      <div class="mt-1 flex gap-2">
        <input bind:value={lookupId} placeholder="goal-…" class="h-10 min-w-0 flex-1 rounded-md border border-ink-700 bg-ink-950 px-3 font-mono text-sm text-ink-100 outline-none placeholder:text-ink-500 focus:border-edge" />
        <Button variant="secondary" disabled={busy} onclick={() => void lookup()}>Look up</Button>
      </div>
    </label>
  </div>

  {#if goal}
    <div class="mt-3 rounded-lg border border-ink-700 bg-ink-900 p-4">
      <div class="flex flex-wrap items-center justify-between gap-2">
        <div class="min-w-0">
          <p class="truncate font-semibold text-ink-50">{goal.objective}</p>
          <p class="font-mono text-xs text-ink-500">{goal.id} · thread {goal.thread_id}</p>
        </div>
        <div class="flex items-center gap-2">
          <StatusBadge label={goal.status} tone={goal.status === "active" ? "ok" : "idle"} />
          <span class="text-sm tabular-nums text-ink-300">{goal.budget} left</span>
        </div>
      </div>
      <div class="mt-3 flex flex-col gap-2 sm:flex-row">
        <input bind:value={stepText} placeholder="What happened in this step?" class="h-10 min-w-0 flex-1 rounded-md border border-ink-700 bg-ink-950 px-3 text-sm text-ink-100 outline-none placeholder:text-ink-500 focus:border-edge" />
        <Button variant="secondary" disabled={busy || goal.status !== "active"} onclick={() => void step()}>Record step</Button>
        <Button variant="primary" disabled={busy || goal.status !== "active"} onclick={() => void complete()}>Complete</Button>
      </div>
      <div class="mt-4 border-t border-ink-700 pt-3">
        <div class="flex flex-wrap items-center justify-between gap-2">
          <p class="text-sm font-semibold text-ink-200">Autonomous runner</p>
          <span class="text-xs text-ink-500">At most {MAX_RUN_TURNS} turns per run</span>
        </div>
        <label class="mt-2 flex items-center gap-2 text-sm text-ink-300">
          <input type="checkbox" bind:checked={blocked} class="h-4 w-4 rounded border-ink-700 bg-ink-950" />
          Marked blocked — the runner stops before the next turn
        </label>
        <div class="mt-2 flex flex-col gap-2 sm:flex-row">
          <Button variant="secondary" disabled={runnerBusy || busy || goal.status !== "active" || goal.budget <= 0} onclick={() => void runNext()}>Run next step</Button>
          <Button variant="primary" disabled={runnerBusy || busy || goal.status !== "active" || goal.budget <= 0} onclick={() => void runAll()}>Run until done</Button>
          <Button variant="secondary" disabled={!runnerBusy} onclick={stopRun}>Stop</Button>
        </div>
        {#if runnerStatus}
          <p class="mt-2 text-sm text-ink-300" role="status">{runnerStatus}</p>
        {/if}
        {#if runnerPrompt}
          <pre class="mt-2 max-h-40 overflow-auto whitespace-pre-wrap rounded-md border border-ink-700 bg-ink-950 p-2 text-xs text-ink-400">{runnerPrompt}</pre>
        {/if}
      </div>
    </div>
  {/if}
</section>
