<script lang="ts">
  // Developer tools for the local goal store (commands/goal.rs). Nothing here
  // runs a model: the caller supplies each step's progress text.
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import StatusBadge from "./StatusBadge.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import type { Goal } from "../lib/api/bindings";

  let objective = $state("");
  let threadId = $state("");
  let budget = $state(5);
  let lookupId = $state("");
  let stepText = $state("");
  let goal = $state<Goal | null>(null);
  let notice = $state<{ ok: boolean; text: string } | null>(null);
  let busy = $state(false);

  const settled = (g: Goal, text: string) => {
    goal = g;
    lookupId = g.id;
    notice = { ok: true, text };
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
</script>

<section class="mt-9" aria-labelledby="goal-panel-title">
  <h3 id="goal-panel-title" class="mb-2 flex items-center gap-2 text-[15px] font-semibold text-ink-50">
    <Icon name="target" class="h-4 w-4 text-ember-300" />Goals
  </h3>
  <p class="mb-3 text-sm text-ink-400">
    Track a multi-step objective in the local goals database. Steps only record progress text you write;
    no model is called.
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
    </div>
  {/if}
</section>
