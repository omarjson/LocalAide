<script lang="ts">
  // Check my setup: Tiny model present (downloaded if missing), loaded, and
  // one reply. Steps run in order; each shows pass or fail with its time.
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import ProgressBar from "./ProgressBar.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import type { SelfTestReport, SelfTestStep } from "../lib/api/bindings";

  const PLAN: { step: SelfTestStep["step"]; label: string; hint: string }[] = [
    { step: "model", label: "Tiny model on this computer", hint: "Downloads it once if it is missing (about 0.7 GB), with its checksum checked." },
    { step: "engine", label: "Model engine", hint: "Downloads the llama.cpp build for this processor once, if it is missing." },
    { step: "load", label: "Load the model", hint: "Starts it in the local engine." },
    { step: "reply", label: "First reply", hint: "Asks it to say hello, on this computer only." },
  ];

  let steps = $state<Record<string, SelfTestStep>>({});
  let report = $state<SelfTestReport | null>(null);
  let running = $state(false);
  let error = $state<string | null>(null);

  const run = async () => {
    running = true;
    report = null;
    error = null;
    steps = {};
    try {
      report = await api.selfTest((s) => (steps = { ...steps, [s.step]: s }));
    } catch (e: unknown) {
      error = errorMessage(e);
    } finally {
      running = false;
    }
  };
  const time = (ms: number) => (ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(1)} s`);
</script>

<div class="rounded-xl border border-ink-700 bg-ink-900">
  <div class="flex flex-wrap items-center gap-3 p-4">
    <div class="min-w-0 flex-1">
      <h4 class="text-[15px] font-semibold text-ink-50">Check my setup</h4>
      <p class="mt-0.5 text-sm text-ink-400">Downloads the Tiny model if needed, loads it and gets one reply. You can run it again any time.</p>
    </div>
    {#if report}
      <span class={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-sm font-semibold ${report.passed ? "bg-emerald-500/10 text-emerald-400" : "bg-rose-500/10 text-rose-300"}`}>
        <Icon name={report.passed ? "check" : "alert"} class="h-4 w-4" />{report.passed ? `Passed in ${time(report.ms)}` : "Something failed"}
      </span>
    {/if}
    <Button variant={report ? "secondary" : "primary"} disabled={running} onclick={() => void run()}>
      {#if running}<span class="h-4 w-4 animate-spin rounded-full border-2 border-ink-700 border-t-ember-500" aria-hidden="true"></span>Checking…{:else}{report ? "Run again" : "Check my setup"}{/if}
    </Button>
  </div>

  {#if running || report || Object.keys(steps).length > 0}
    <ol class="border-t border-ink-800" aria-live="polite">
      {#each PLAN as item, i (item.step)}
        {@const s = steps[item.step]}
        <li class="flex gap-3 border-b border-ink-800 px-4 py-3 last:border-b-0">
          <span class={`step ${s?.status ?? "wait"}`} aria-hidden="true">
            {#if s?.status === "pass"}<Icon name="check" class="h-3.5 w-3.5" />
            {:else if s?.status === "fail"}<Icon name="close" class="h-3.5 w-3.5" />
            {:else if s?.status === "running"}<span class="h-3 w-3 animate-spin rounded-full border-2 border-current border-t-transparent"></span>
            {:else}{i + 1}{/if}
          </span>
          <div class="min-w-0 flex-1">
            <div class="flex flex-wrap items-baseline gap-x-3">
              <span class={`text-sm font-medium ${s ? "text-ink-50" : "text-ink-400"}`}>{item.label}</span>
              {#if s && s.status !== "running" && s.status !== "skip"}<span class="text-xs tabular-nums text-ink-500">{time(s.ms)}</span>{/if}
              <span class="sr-only">{s?.status ?? "waiting"}</span>
            </div>
            <p class={`mt-0.5 text-[13px] leading-5 ${s?.status === "fail" ? "text-rose-300" : "text-ink-400"}`}>{s?.detail || item.hint}</p>
            {#if s?.status === "running" && s.percent !== null}
              <ProgressBar value={s.percent} label={item.label} class="mt-2 h-1.5 max-w-sm" />
              <span class="mt-1 block text-xs tabular-nums text-ink-500">{Math.round(s.percent)}%</span>
            {/if}
          </div>
        </li>
      {/each}
    </ol>
  {/if}
  {#if error}<p class="border-t border-ink-800 px-4 py-3 text-sm text-rose-300" role="alert">{error}</p>{/if}
</div>

<style>
  .step {
    display: grid;
    height: 1.5rem;
    width: 1.5rem;
    flex-shrink: 0;
    place-items: center;
    border-radius: 9999px;
    font-size: 0.75rem;
    font-weight: 600;
    background: var(--color-ink-800);
    color: var(--color-ink-400);
  }
  .step.pass {
    background: color-mix(in oklab, #10b981 18%, transparent);
    color: #34d399;
  }
  .step.fail {
    background: color-mix(in oklab, #f43f5e 18%, transparent);
    color: #fda4af;
  }
  .step.running {
    background: color-mix(in oklab, var(--color-ember-500) 18%, transparent);
    color: var(--color-ember-300);
  }
</style>
