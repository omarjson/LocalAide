<script lang="ts">
  import { onMount } from "svelte";
  import Button from "../components/Button.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import ErrorBanner from "../components/ErrorBanner.svelte";
  import Icon from "../components/Icon.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import { describeError } from "../lib/api/errors";
  import { folderName } from "../lib/chats.svelte";
  import type { ModelPrice, SystemProbeResult, UsageSummary } from "../lib/api/bindings";
  import { answerStreak, compactTokens, formatAnswerMs, windowCoverage, windowDays } from "../lib/usage";

  // days 1 = today, 0 = all time, anything else the rolling window.
  const RANGES: { label: string; days: number }[] = [
    { label: "Today", days: 1 },
    { label: "7 days", days: 7 },
    { label: "30 days", days: 30 },
    { label: "90 days", days: 90 },
    { label: "All time", days: 0 },
  ];
  let days = $state(30);
  let folder = $state<string | null>(null);
  let summary = $state<UsageSummary | null>(null);
  let prices = $state<Record<string, ModelPrice>>({});
  let loading = $state(true);
  let errorMsg = $state<string | null>(null);
  let errorFix = $state<string | null>(null);
  let probe = $state<SystemProbeResult | null>(null);
  let updatedAt = $state<Date | null>(null);
  let showToolCalls = $state(false);
  let modelView = $state<"tokens" | "cost" | "steps">("tokens");

  const load = async () => {
    loading = true;
    errorMsg = null;
    errorFix = null;
    try {
      const [next, nextPrices] = await Promise.all([
        api.usageSummary(days, folder),
        api.openrouterModelPrices().catch(() => ({} as Record<string, ModelPrice>)),
      ]);
      summary = next;
      prices = nextPrices;
      updatedAt = new Date();
    } catch (e: unknown) {
      const described = describeError(e);
      errorMsg = described.message;
      errorFix = described.fix;
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

  const setFolder = (value: string | null) => {
    if (folder === value) return;
    folder = value;
    void load();
  };

  const rangeLabel = $derived(RANGES.find((r) => r.days === days)?.label ?? `${days} days`);
  const updatedLabel = $derived(
    updatedAt
      ? `Updated ${updatedAt.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}`
      : "",
  );

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
  // What window the totals actually cover: when retained history is
  // shorter than the requested window, the totals say so (N7).
  const coverage = $derived(summary ? windowCoverage(summary) : null);
  // Models with no price are costed at $0 — a total that silently omits
  // rows is a caveat, so the Cost card names them (N7).
  const unpricedCount = $derived(summary ? summary.per_model.filter((m) => !prices[m.model]).length : 0);
  const peak = $derived(Math.max(1, ...series.map((day) => day.output)));
  const streak = $derived(answerStreak(series));
  const speed = $derived.by(() => {
    if (!summary || summary.ms <= 0 || summary.output_tokens <= 0) return null;
    return summary.output_tokens / (summary.ms / 1000);
  });

  // --- Cost: per-model from OpenRouter's prices, USD per 1M tokens.
  // Models OpenRouter does not list (or a list that failed to load) price
  // at $0 rather than an invented number. Cache reads ride the input count
  // in provider reports, so they are split out and charged at their own
  // price when one exists.
  const modelCost = (m: { model: string; input: number; output: number; cache: number }): number => {
    const price = prices[m.model];
    if (!price) return 0;
    const cached = Math.min(m.cache, m.input);
    const freshIn = m.input - cached;
    const cachePrice = price.cache_read ?? price.input;
    return (freshIn * price.input + cached * cachePrice + m.output * price.output) / 1_000_000;
  };
  const totalCost = $derived.by(() => {
    if (!summary) return 0;
    return summary.per_model.reduce((sum, m) => sum + modelCost(m), 0);
  });
  const costLabel = (usd: number) => (usd > 0 ? `$${usd < 0.01 ? usd.toFixed(4) : usd.toFixed(2)}` : "$0.00");

  const totalTokens = $derived.by(() => {
    if (!summary) return 0;
    return summary.input_tokens + summary.output_tokens + summary.cache_tokens;
  });
  const share = (part: number) => (totalTokens > 0 ? (part / totalTokens) * 100 : 0);
  // Cache reads share the prompt with fresh input on the bar; draws show it.
  const compInput = $derived.by(() => {
    if (!summary) return 0;
    return Math.max(0, summary.input_tokens - summary.cache_tokens);
  });
  const cacheRate = $derived.by(() => {
    if (!summary || summary.input_tokens <= 0 || summary.cache_tokens <= 0) return null;
    return (summary.cache_tokens / summary.input_tokens) * 100;
  });

  const sessions = $derived(Math.max(1, summary?.sessions ?? 1));
  const tokensPerSession = $derived.by(() => {
    if (!summary) return 0;
    return Math.round(totalTokens / sessions);
  });
  const costPerSession = $derived.by(() => {
    if (!summary || summary.sessions === 0) return null;
    return totalCost / summary.sessions;
  });
  const costPer1M = $derived.by(() => {
    if (!summary || totalTokens <= 0) return null;
    return (totalCost / totalTokens) * 1_000_000;
  });
  const stepsPerSession = $derived.by(() => {
    if (!summary) return 0;
    return Math.round(summary.tool_calls / sessions);
  });

  const modelMetric = (m: UsageSummary["per_model"][number]): number => {
    if (modelView === "tokens") return m.input + m.output + m.cache;
    if (modelView === "cost") return modelCost(m);
    return m.steps;
  };
  const maxMetric = $derived(Math.max(1e-9, ...(summary?.per_model ?? []).map(modelMetric)));
  const metricLabel = (m: UsageSummary["per_model"][number]): string => {
    if (modelView === "tokens") return `${compactTokens(modelMetric(m))} · ${costLabel(modelCost(m))}`;
    if (modelView === "cost") return costLabel(modelCost(m));
    return `${m.steps} steps`;
  };

  const maxProject = $derived(
    Math.max(1, ...(summary?.per_folder ?? []).map((p) => p.output)),
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
      <div class="flex flex-wrap items-center justify-end gap-2">
        {#if updatedLabel}
          <span class="flex items-center gap-1.5 text-sm text-ink-400" title="Reload the numbers">
            {updatedLabel}
            <button type="button" aria-label="Refresh usage" class="grid h-7 w-7 place-items-center rounded-md text-ink-400 transition-colors hover:bg-veil/5 hover:text-ink-100" onclick={() => void load()}>
              <Icon name="refresh" class="h-3.5 w-3.5" />
            </button>
          </span>
        {/if}
        <div class="flex items-center gap-1 rounded-lg border border-ink-700 bg-ink-900 p-1">
          {#each RANGES as range (range.days)}
            <button
              type="button"
              onclick={() => setRange(range.days)}
              aria-pressed={days === range.days}
              class={`rounded-md px-3 py-1.5 text-sm transition-colors ${days === range.days ? "bg-ink-100 font-semibold text-on-ember" : "text-ink-400 hover:text-ink-50"}`}
            >{range.label}</button>
          {/each}
        </div>
        {#if summary && summary.per_folder.length > 0}
          <label class="flex items-center gap-2 rounded-lg border border-ink-700 bg-ink-900 px-2 py-1.5 text-sm text-ink-400">
            <Icon name="folder-open" class="h-4 w-4 shrink-0" />
            <span class="sr-only">Project</span>
            <select
              class="cursor-pointer bg-transparent text-sm text-ink-100 outline-none"
              onchange={(e) => setFolder((e.currentTarget as HTMLSelectElement).value || null)}
              aria-label="Filter usage by project"
            >
              <option value="" class="bg-ink-900">All projects</option>
              {#each summary.per_folder as project (project.folder)}
                <option value={project.folder} selected={folder === project.folder} class="bg-ink-900">{folderName(project.folder)}</option>
              {/each}
            </select>
          </label>
        {/if}
      </div>
    {/snippet}
  </PageHeader>

  {#if errorMsg}
    <div class="mb-4"><ErrorBanner message={errorMsg} fix={errorFix ?? undefined} onRetry={() => void load()} onDismiss={() => { errorMsg = null; errorFix = null; }} /></div>
  {/if}

  {#if loading}
    <div class="rounded-lg border border-ink-700 bg-ink-900 px-4 py-12 text-center text-sm text-ink-400">Counting answers…</div>
  {:else if summary && summary.answers === 0}
    <EmptyState
      icon="chart"
      title="No answers yet"
      description={`Every answer LocalAide gives lands here — totals, speed and which models you use. ${folder ? "Nothing from this project in the window." : `Try the next ${rangeLabel.toLowerCase()}.`}`}
    />
  {:else if summary}
    <div class="space-y-5">
      {#if coverage?.short}
        <p class="rounded-lg border border-amber-500/30 bg-amber-500/10 px-4 py-2 text-sm text-amber-200" role="note">Totals cover {coverage.from} – {coverage.to}. Nothing is recorded before {coverage.from}.</p>
      {/if}
      <!-- Headline numbers -->
      <div class="grid grid-cols-2 gap-3 sm:grid-cols-3">
        <div class="rounded-lg border border-ink-700 bg-ink-900 p-4">
          <p class="text-sm text-ink-400">Sessions</p>
          <p class="mt-1 text-3xl font-semibold text-ink-50">{summary.sessions}</p>
          <p class="mt-1 text-sm text-ink-400">{summary.cancelled > 0 ? `${summary.cancelled} stopped` : `${summary.answers} answers`}</p>
        </div>
        <div class="rounded-lg border border-ink-700 bg-ink-900 p-4">
          <p class="text-sm text-ink-400">Prompts</p>
          <p class="mt-1 text-3xl font-semibold text-ink-50">{summary.answers}</p>
          <p class="mt-1 text-sm text-ink-400">{formatAnswerMs(summary.ms)} answering</p>
        </div>
        <div class="rounded-lg border border-ink-700 bg-ink-900 p-4">
          <p class="text-sm text-ink-400">Tokens</p>
          <p class="mt-1 text-3xl font-semibold text-ink-50">{compactTokens(totalTokens)}</p>
          <p class="mt-1 text-sm text-ink-400">In {compactTokens(summary.input_tokens)} · Out {compactTokens(summary.output_tokens)}{summary.cache_tokens > 0 ? ` · Cache ${compactTokens(summary.cache_tokens)}` : ""}</p>
        </div>
        <div class="rounded-lg border border-ink-700 bg-ink-900 p-4">
          <p class="text-sm text-ink-400">Cost</p>
          <p class="mt-1 text-3xl font-semibold text-ink-50">{costLabel(totalCost)}</p>
          <p class="mt-1 text-sm text-ink-400">{Object.keys(prices).length > 0 ? "From your price table" : "Add prices in Settings to track"}</p>
          {#if unpricedCount > 0}<p class="mt-1 text-sm text-amber-300">Cost leaves out {unpricedCount} {unpricedCount === 1 ? "model" : "models"} with no price.</p>{/if}
        </div>
        <div class="rounded-lg border border-ink-700 bg-ink-900 p-4">
          <p class="text-sm text-ink-400">Active days</p>
          <p class="mt-1 text-3xl font-semibold text-ink-50">{summary.active_days}</p>
          <p class="mt-1 text-sm text-ink-400">of {days === 0 ? "all time" : days === 1 ? "today" : `${days} days`}</p>
        </div>
        <div class="rounded-lg border border-ink-700 bg-ink-900 p-4">
          <p class="text-sm text-ink-400">Longest streak (days)</p>
          <p class="mt-1 text-3xl font-semibold text-ink-50">{summary.longest_streak}</p>
          {#if streak >= 2}
            <p class="mt-1 flex items-center gap-1 text-sm text-ink-300"><Icon name="flame" class="h-3.5 w-3.5" />{streak} now</p>
          {/if}
        </div>
      </div>

      <!-- Daily activity -->
      <div class="rounded-lg border border-ink-700 bg-ink-900 p-5">
        <div class="flex items-center justify-between">
          <h2 class="font-semibold text-ink-50">Daily activity</h2>
          <span class="text-sm text-ink-500">Answers per day</span>
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

      <!-- Token composition -->
      <div class="rounded-lg border border-ink-700 bg-ink-900 p-5">
        <h2 class="font-semibold text-ink-50">Token composition</h2>
        <div class="mt-4 flex h-2.5 overflow-hidden rounded-full bg-ink-800">
          {#if compInput > 0}
            <div class="h-full bg-ember-600" style={`width:${share(compInput)}%`} title={`Input ${compactTokens(compInput)} · ${share(compInput).toFixed(1)}%`}></div>
          {/if}
          {#if summary.output_tokens > 0}
            <div class="h-full bg-ember-400" style={`width:${share(summary.output_tokens)}%`} title={`Output ${compactTokens(summary.output_tokens)} · ${share(summary.output_tokens).toFixed(1)}%`}></div>
          {/if}
          {#if summary.cache_tokens > 0}
            <div class="h-full bg-ink-500" style={`width:${share(summary.cache_tokens)}%`} title={`Cache reads ${compactTokens(summary.cache_tokens)} · ${share(summary.cache_tokens).toFixed(1)}%`}></div>
          {/if}
        </div>
        <div class="mt-3 grid grid-cols-1 gap-x-6 gap-y-1 text-sm sm:grid-cols-2">
          <div class="flex items-center justify-between gap-3">
            <span class="flex items-center gap-2 text-ink-400"><span class="h-2 w-2 shrink-0 rounded-full bg-ember-600"></span>Input</span>
            <span class="tabular-nums text-ink-200">{compactTokens(compInput)} · {share(compInput).toFixed(1)}%</span>
          </div>
          <div class="flex items-center justify-between gap-3">
            <span class="flex items-center gap-2 text-ink-400"><span class="h-2 w-2 shrink-0 rounded-full bg-ember-400"></span>Output + reasoning</span>
            <span class="tabular-nums text-ink-200">{compactTokens(summary.output_tokens)} · {share(summary.output_tokens).toFixed(1)}%</span>
          </div>
          <div class="flex items-center justify-between gap-3">
            <span class="flex items-center gap-2 text-ink-400"><span class="h-2 w-2 shrink-0 rounded-full bg-ink-500"></span>Cache reads</span>
            <span class="tabular-nums text-ink-200">{compactTokens(summary.cache_tokens)} · {share(summary.cache_tokens).toFixed(1)}%</span>
          </div>
          <div class="flex items-center justify-between gap-3">
            <span class="flex items-center gap-2 text-ink-400"><span class="h-2 w-2 shrink-0 rounded-full border border-ink-500"></span>Cache writes</span>
            <span class="tabular-nums text-ink-500">not reported</span>
          </div>
        </div>
      </div>

      <!-- Efficiency -->
      <div class="rounded-lg border border-ink-700 bg-ink-900 p-5">
        <h2 class="font-semibold text-ink-50">Efficiency</h2>
        <div class="mt-4 grid grid-cols-2 gap-3 sm:grid-cols-3">
          <div class="rounded-lg border border-ink-800 bg-ink-950 p-3">
            <p class="text-sm text-ink-400">Cache hit rate</p>
            <p class="mt-1 text-2xl font-semibold text-ink-50">{cacheRate !== null ? `${cacheRate.toFixed(1)}%` : "—"}</p>
          </div>
          <div class="rounded-lg border border-ink-800 bg-ink-950 p-3">
            <p class="text-sm text-ink-400">Cost / session</p>
            <p class="mt-1 text-2xl font-semibold text-ink-50">{costPerSession !== null ? costLabel(costPerSession) : "—"}</p>
          </div>
          <div class="rounded-lg border border-ink-800 bg-ink-950 p-3">
            <p class="text-sm text-ink-400">Cost per 1M tokens</p>
            <p class="mt-1 text-2xl font-semibold text-ink-50">{costPer1M !== null ? costLabel(costPer1M) : "—"}</p>
          </div>
          <div class="rounded-lg border border-ink-800 bg-ink-950 p-3">
            <p class="text-sm text-ink-400">Tokens / session</p>
            <p class="mt-1 text-2xl font-semibold text-ink-50">{compactTokens(tokensPerSession)}</p>
          </div>
          <div class="rounded-lg border border-ink-800 bg-ink-950 p-3">
            <p class="text-sm text-ink-400">Steps / session</p>
            <p class="mt-1 text-2xl font-semibold text-ink-50">{stepsPerSession}</p>
          </div>
          <div class="rounded-lg border border-ink-800 bg-ink-950 p-3">
            <p class="text-sm text-ink-400">Avg speed</p>
            <p class="mt-1 text-2xl font-semibold text-ink-50">{speed ? speed.toFixed(1) : "—"}</p>
            <p class="text-xs text-ink-500">tokens/s</p>
          </div>
        </div>
      </div>

      <!-- Per-model breakdown with Tokens / Cost / Steps views -->
      <div class="rounded-lg border border-ink-700 bg-ink-900 p-5">
        <div class="flex items-center justify-between">
          <h2 class="font-semibold text-ink-50">Models</h2>
          <div class="flex items-center gap-1 rounded-lg border border-ink-700 bg-ink-950 p-1">
            {#each ["tokens", "cost", "steps"] as view (view)}
              <button
                type="button"
                onclick={() => (modelView = view as typeof modelView)}
                aria-pressed={modelView === view}
                class={`rounded-md px-2.5 py-1 text-sm capitalize transition-colors ${modelView === view ? "bg-ink-800 font-semibold text-ink-50" : "text-ink-400 hover:text-ink-50"}`}
              >{view}</button>
            {/each}
          </div>
        </div>
        <div class="mt-4 space-y-4">
          {#each summary.per_model as m (m.model + m.provider)}
            <div>
              <div class="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
                <p class="truncate font-medium text-ink-100">{m.model}</p>
                <p class="shrink-0 text-sm text-ink-400">{metricLabel(m)}</p>
              </div>
              <div class="mt-1.5 flex items-center gap-2">
                <div class="h-2 min-w-0 flex-1 overflow-hidden rounded-full bg-ink-800">
                  <div
                    class="h-full rounded-full bg-gradient-to-r from-ember-600 to-ember-400 transition-[width] duration-500"
                    style={`width:${Math.max(2, (modelMetric(m) / maxMetric) * 100)}%`}
                  ></div>
                </div>
                <span class="shrink-0 rounded-full border border-ink-700 px-2 py-0.5 text-xs text-ink-400">{m.provider}</span>
              </div>
            </div>
          {/each}
        </div>
      </div>

      <!-- Projects -->
      {#if summary.per_folder.length > 0}
        <div class="rounded-lg border border-ink-700 bg-ink-900 p-5">
          <div class="flex items-center justify-between">
            <h2 class="font-semibold text-ink-50">Projects</h2>
            {#if folder}
              <button type="button" class="rounded-md border border-ink-700 px-2 py-1 text-sm text-ink-400 transition-colors hover:bg-veil/5 hover:text-ink-100" onclick={() => setFolder(null)}>Showing {folderName(folder)} — show all</button>
            {/if}
          </div>
          <div class="mt-4 space-y-4">
            {#each summary.per_folder as project (project.folder)}
              <button
                type="button"
                class="block w-full text-left"
                onclick={() => setFolder(folder === project.folder ? null : project.folder)}
                aria-pressed={folder === project.folder}
                title="Show only this project"
              >
                <div class="flex flex-wrap items-baseline justify-between gap-x-3 gap-y-1">
                  <p class="truncate font-medium text-ink-100">{folderName(project.folder)}</p>
                  <p class="shrink-0 text-sm text-ink-400">
                    {project.answers} {project.answers === 1 ? "answer" : "answers"} · {compactTokens(project.output)} out
                    <span class="text-ink-500"> · {formatAnswerMs(project.ms)}</span>
                  </p>
                </div>
                <div class="mt-1.5 flex items-center gap-2">
                  <div class="h-2 min-w-0 flex-1 overflow-hidden rounded-full bg-ink-800">
                    <div
                      class="h-full rounded-full bg-gradient-to-r from-ember-600 to-ember-400 transition-[width] duration-500"
                      style={`width:${Math.max(2, (project.output / maxProject) * 100)}%`}
                    ></div>
                  </div>
                  <span class={`shrink-0 rounded-full border px-2 py-0.5 text-xs transition-colors ${folder === project.folder ? "border-ink-600 bg-veil/8 text-ink-100" : "border-ink-700 text-ink-400"}`}>{project.answers} answers</span>
                </div>
              </button>
            {/each}
          </div>
        </div>
      {/if}

      <!-- Tool calls, loaded on request like the reference -->
      {#if summary.tool_calls > 0}
        <div class="rounded-lg border border-ink-700 bg-ink-900 p-5">
          <div class="flex items-center justify-between">
            <h2 class="font-semibold text-ink-50">Tool calls</h2>
            <span class="text-sm text-ink-500">{summary.tool_calls} in the window</span>
          </div>
          {#if showToolCalls}
            <div class="mt-4 space-y-2">
              {#each summary.per_model.filter((m) => m.steps > 0) as m (m.model + m.provider)}
                <div class="flex items-center justify-between text-sm">
                  <span class="truncate text-ink-200">{m.model}</span>
                  <span class="shrink-0 tabular-nums text-ink-400">{m.steps} {m.steps === 1 ? "run" : "runs"}</span>
                </div>
              {/each}
            </div>
          {:else}
            <div class="mt-3 flex items-center gap-3">
              <Button variant="subtle" onclick={() => (showToolCalls = true)}>Show tool calls</Button>
              <span class="text-xs text-ink-500">Counted from the usage log, grouped by model.</span>
            </div>
          {/if}
        </div>
      {/if}

      <!-- This machine -->
      {#if probe}
        <div class="rounded-lg border border-ink-700 bg-ink-900 p-5">
          <div class="flex items-center justify-between">
            <h2 class="font-semibold text-ink-50">This computer</h2>
            {#if localSpeed !== null}
              <span class="rounded-full border border-ink-600 bg-veil/8 px-2.5 py-1 text-sm text-ink-300">{localSpeed.toFixed(1)} tok/s on this machine</span>
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
