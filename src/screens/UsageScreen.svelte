<script lang="ts">
  import { onMount } from "svelte";
  import Button from "../components/Button.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import ErrorBanner from "../components/ErrorBanner.svelte";
  import Icon from "../components/Icon.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import type { SystemProbeResult, UsageSummary } from "../lib/api/bindings";
  import { answerStreak, compactTokens, formatAnswerMs, windowDays } from "../lib/usage";

  let days = $state(30);
  let summary = $state<UsageSummary | null>(null);
  let loading = $state(true);
  let errorMsg = $state<string | null>(null);
  let probe = $state<SystemProbeResult | null>(null);

  const load = async () => {
    loading = true;
    errorMsg = null;
    try {
      summary = await api.usageSummary(days);
    } catch (e: unknown) {
      errorMsg = errorMessage(e);
      summary = null;
    } finally {
      loading = false;
    }
  };

  const setRange = (value: number) => {
    if (days === value) return;
    days = value;
    void load();
  };

  const loadProbe = async () => {
    try {
      probe = await api.systemProbe();
    } catch {
      probe = null;
    }
  };

  onMount(() => {
    void load();
    void loadProbe();
  });

  const series = $derived(summary ? windowDays(summary) : []);
  const peak = $derived(Math.max(1, ...series.map((day) => day.output)));
  const streak = $derived(answerStreak(series));
  const speed = $derived.by(() => {
    if (!summary || summary.ms <= 0 || summary.output_tokens <= 0) return null;
    return summary.output_tokens / (summary.ms / 1000);
  });
  const maxModel = $derived(
    Math.max(1, ...(summary?.per_model ?? []).map((m) => m.output)),
  );
  const localSpeed = $derived.by(() => {
    const rows = summary?.per_model.filter((m) => m.provider === "This computer") ?? [];
    const totalMs = rows.reduce((sum, m) => sum + m.avg_ms * m.answers, 0);
    const out = rows.reduce((sum, m) => sum + m.output, 0);
    if (out <= 0 || totalMs <= 0) return null;
    return out / (totalMs / 1000);
  });
  const ramPct = $derived.by(() => {
    if (!probe || probe.total_ram_gb <= 0) return 0;
    return Math.min(100, ((probe.total_ram_gb - probe.available_ram_gb) / probe.total_ram_gb) * 100);
  });

  const barHeight = (day: { answers: number; output: number }) => {
    if (day.answers === 0) return 0;
    return Math.max(5, (day.output / peak) * 100);
  };
</script>

<div class="mx-auto max-w-5xl pb-10">
  <PageHeader title="Usage" description="What LocalAide has answered, and how fast.">
    {#snippet actions()}
      <div class="flex items-center gap-1 rounded-lg border border-ink-700 bg-ink-900 p-1">
        {#each [7, 30, 90] as range (range)}
          <button
            type="button"
            onclick={() => setRange(range)}
            aria-pressed={days === range}
            class={`rounded-md px-3 py-1.5 text-sm transition-colors ${days === range ? "bg-ember-600 font-semibold text-on-ember" : "text-ink-400 hover:text-ink-50"}`}
          >{range}d</button>
        {/each}
      </div>
    {/snippet}
  </PageHeader>

  {#if errorMsg}
    <div class="mb-4"><ErrorBanner message={errorMsg} onRetry={() => void load()} onDismiss={() => errorMsg = null} /></div>
  {/if}

  {#if loading}
    <div class="rounded-lg border border-ink-700 bg-ink-900 px-4 py-12 text-center text-sm text-ink-400">Counting answers…</div>
  {:else if summary && summary.answers === 0}
    <EmptyState
      icon="chart"
      title="No answers yet"
      description={`Every answer LocalAide gives lands here — totals, speed and which models you use. Try the next ${days} days.`}
    />
  {:else if summary}
    <div class="space-y-5">
      <!-- Headline numbers -->
      <div class="grid grid-cols-2 gap-3 sm:grid-cols-4">
        <div class="rounded-lg border border-ink-700 bg-ink-900 p-4">
          <p class="text-sm text-ink-400">Answers</p>
          <p class="mt-1 text-3xl font-semibold text-ink-50">{summary.answers}</p>
          {#if summary.cancelled > 0}
            <p class="mt-1 text-sm text-ink-400">{summary.cancelled} stopped</p>
          {/if}
        </div>
        <div class="rounded-lg border border-ink-700 bg-ink-900 p-4">
          <p class="text-sm text-ink-400">Tokens out</p>
          <p class="mt-1 text-3xl font-semibold text-ink-50">{compactTokens(summary.output_tokens)}</p>
          <p class="mt-1 text-sm text-ink-400">{compactTokens(summary.input_tokens)} in</p>
        </div>
        <div class="rounded-lg border border-ink-700 bg-ink-900 p-4">
          <p class="text-sm text-ink-400">Time answering</p>
          <p class="mt-1 text-3xl font-semibold text-ink-50">{formatAnswerMs(summary.ms)}</p>
          {#if summary.tool_calls > 0}
            <p class="mt-1 text-sm text-ink-400">{summary.tool_calls} tool runs</p>
          {/if}
        </div>
        <div class="rounded-lg border border-ink-700 bg-ink-900 p-4">
          <p class="text-sm text-ink-400">Avg speed</p>
          <p class="mt-1 text-3xl font-semibold text-ink-50">{speed ? speed.toFixed(1) : "—"}</p>
          <p class="mt-1 text-sm text-ink-400">tokens/s, tool work included</p>
        </div>
      </div>

      <!-- Daily activity -->
      <div class="rounded-lg border border-ink-700 bg-ink-900 p-5">
        <div class="flex items-center justify-between">
          <h2 class="font-semibold text-ink-50">Activity</h2>
          {#if streak >= 2}
            <span class="flex items-center gap-1.5 rounded-full border border-ember-500/40 bg-ember-500/10 px-2.5 py-1 text-sm text-ember-200">
              <Icon name="flame" class="h-3.5 w-3.5" />{streak}-day streak
            </span>
          {/if}
        </div>
        <div class="mt-4 flex h-36 items-end gap-[3px]">
          {#each series as day (day.date)}
            <div
              class="group relative flex h-full min-w-0 flex-1 flex-col justify-end"
              title={`${day.date} — ${day.answers} ${day.answers === 1 ? "answer" : "answers"}, ${compactTokens(day.output)} out`}
            >
              <div
                class={`w-full rounded-t-sm transition-[height,background-color] duration-500 ${day.answers > 0 ? "bg-gradient-to-t from-ember-600 to-ember-400 group-hover:from-ember-500 group-hover:to-ember-300" : "bg-ink-800"}`}
                style={`height:${barHeight(day)}%`}
              ></div>
            </div>
          {/each}
        </div>
        <div class="mt-2 flex justify-between text-xs text-ink-500">
          <span>{series[0]?.date ?? ""}</span>
          <span>{series[Math.floor(series.length / 2)]?.date ?? ""}</span>
          <span>Today</span>
        </div>
      </div>

      <!-- Per-model breakdown -->
      <div class="rounded-lg border border-ink-700 bg-ink-900 p-5">
        <h2 class="font-semibold text-ink-50">Models</h2>
        <div class="mt-4 space-y-4">
          {#each summary.per_model as m (m.model + m.provider)}
            <div>
              <div class="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
                <p class="truncate font-medium text-ink-100">{m.model}</p>
                <p class="shrink-0 text-sm text-ink-400">
                  {m.answers} {m.answers === 1 ? "answer" : "answers"} · {compactTokens(m.output)} out
                  {#if m.avg_tok_s !== null}
                    <span class="text-ink-500"> · {m.avg_tok_s.toFixed(1)} tok/s</span>
                  {/if}
                  {#if m.avg_ttft_ms !== null}
                    <span class="text-ink-500"> · first token {(m.avg_ttft_ms / 1000).toFixed(1)} s</span>
                  {/if}
                </p>
              </div>
              <div class="mt-1.5 flex items-center gap-2">
                <div class="h-2 min-w-0 flex-1 overflow-hidden rounded-full bg-ink-800">
                  <div
                    class="h-full rounded-full bg-gradient-to-r from-ember-600 to-ember-400 transition-[width] duration-500"
                    style={`width:${Math.max(2, (m.output / maxModel) * 100)}%`}
                  ></div>
                </div>
                <span class="shrink-0 rounded-full border border-ink-700 px-2 py-0.5 text-xs text-ink-400">{m.provider}</span>
              </div>
            </div>
          {/each}
        </div>
      </div>

      <!-- This machine -->
      {#if probe}
        <div class="rounded-lg border border-ink-700 bg-ink-900 p-5">
          <div class="flex items-center justify-between">
            <h2 class="font-semibold text-ink-50">This computer</h2>
            {#if localSpeed !== null}
              <span class="rounded-full border border-ember-500/40 bg-ember-500/10 px-2.5 py-1 text-sm text-ember-200">{localSpeed.toFixed(1)} tok/s on this machine</span>
            {/if}
          </div>
          <div class="mt-4">
            <div class="flex justify-between text-sm text-ink-400">
              <span>Memory in use</span>
              <span>{(probe.total_ram_gb - probe.available_ram_gb).toFixed(1)} / {probe.total_ram_gb.toFixed(1)} GB · model budget {probe.budget_gb.toFixed(1)} GB</span>
            </div>
            <div class="mt-2 h-2 overflow-hidden rounded-full bg-ink-800">
              <div class="h-full rounded-full bg-ink-600 transition-[width] duration-500" style={`width:${Math.max(2, ramPct)}%`}></div>
            </div>
          </div>
        </div>
      {/if}

      {#if summary.per_model.length > 0}
        <p class="text-center text-sm text-ink-500">Counts every answer since 0.2.0. Deleting chats does not delete the counts.</p>
      {/if}
    </div>
  {/if}
</div>
