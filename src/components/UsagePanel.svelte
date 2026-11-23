<script lang="ts">
  // Developer tools for the local usage ledger (commands/usage_ledger.rs).
  // Costs are estimates from the flat rate the caller supplies; no price
  // table is looked up or invented here.
  import { onMount } from "svelte";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import type { UsageLedgerEntry } from "../lib/api/bindings";

  let provider = $state("");
  let model = $state("");
  let inputTokens = $state(0);
  let outputTokens = $state(0);
  let threadId = $state("");
  let rate = $state("");
  let entries = $state<UsageLedgerEntry[]>([]);
  let filterThread = $state("");
  let notice = $state<{ ok: boolean; text: string } | null>(null);
  let busy = $state(false);
  let loading = $state(false);

  const load = async () => {
    loading = true;
    try {
      entries = await api.usageList(filterThread.trim() || null, 200);
    } catch (e: unknown) {
      notice = { ok: false, text: errorMessage(e) };
    } finally {
      loading = false;
    }
  };

  const record = async () => {
    if (!provider.trim() || !model.trim() || !threadId.trim()) {
      notice = { ok: false, text: "Provider, model and thread id are required." };
      return;
    }
    busy = true;
    notice = null;
    try {
      const parsedRate = rate.trim() === "" ? null : Number(rate);
      const entry = await api.usageRecord({
        provider: provider.trim(),
        model: model.trim(),
        inputTokens: Math.max(0, Math.round(inputTokens)),
        outputTokens: Math.max(0, Math.round(outputTokens)),
        threadId: threadId.trim(),
        ratePerMillionUsd: parsedRate !== null && Number.isFinite(parsedRate) && parsedRate >= 0 ? parsedRate : null,
      });
      notice = { ok: true, text: `Recorded ${entry.model} · estimated $${entry.estimated_cost_usd.toFixed(6)}.` };
      await load();
    } catch (e: unknown) {
      notice = { ok: false, text: errorMessage(e) };
    } finally {
      busy = false;
    }
  };

  onMount(() => { void load(); });
</script>

<section class="mt-9" aria-labelledby="usage-panel-title">
  <h3 id="usage-panel-title" class="mb-2 flex items-center gap-2 text-[15px] font-semibold text-ink-50">
    <Icon name="usage" class="h-4 w-4 text-ember-300" />Usage ledger
  </h3>
  <p class="mb-3 text-sm text-ink-400">
    Record one model call and read the local ledger. This is separate from the aggregated Usage screen;
    it keeps individual calls, with a flat cost estimate you supply.
  </p>

  {#if notice}
    <p class={`mb-3 flex items-center gap-2 text-sm ${notice.ok ? "text-emerald-300" : "text-rose-300"}`} role="status">
      <Icon name={notice.ok ? "check" : "alert"} class="h-4 w-4 shrink-0" />{notice.text}
    </p>
  {/if}

  <div class="rounded-lg border border-ink-700 bg-ink-900 p-4">
    <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
      <label class="text-sm text-ink-400">Provider
        <input bind:value={provider} placeholder="local" class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 text-sm text-ink-100 outline-none placeholder:text-ink-500 focus:border-edge" />
      </label>
      <label class="text-sm text-ink-400">Model
        <input bind:value={model} placeholder="model id" class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 font-mono text-sm text-ink-100 outline-none placeholder:text-ink-500 focus:border-edge" />
      </label>
      <label class="text-sm text-ink-400">Thread id
        <input bind:value={threadId} placeholder="chat id" class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 font-mono text-sm text-ink-100 outline-none placeholder:text-ink-500 focus:border-edge" />
      </label>
      <label class="text-sm text-ink-400">Input tokens
        <input type="number" min="0" bind:value={inputTokens} class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 text-sm tabular-nums text-ink-100 outline-none focus:border-edge" />
      </label>
      <label class="text-sm text-ink-400">Output tokens
        <input type="number" min="0" bind:value={outputTokens} class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 text-sm tabular-nums text-ink-100 outline-none focus:border-edge" />
      </label>
      <label class="text-sm text-ink-400">USD per 1M tokens (optional)
        <input type="number" min="0" step="0.01" bind:value={rate} placeholder="default" class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 text-sm tabular-nums text-ink-100 outline-none placeholder:text-ink-500 focus:border-edge" />
      </label>
    </div>
    <div class="mt-3 flex justify-end"><Button variant="primary" disabled={busy} onclick={() => void record()}>Record call</Button></div>
  </div>

  <div class="mt-3 rounded-lg border border-ink-700 bg-ink-900 p-4">
    <div class="flex flex-col gap-2 sm:flex-row sm:items-end sm:justify-between">
      <label class="text-sm text-ink-400">Filter by thread id
        <input bind:value={filterThread} placeholder="all threads" class="mt-1 h-9 w-full rounded-md border border-ink-700 bg-ink-950 px-3 font-mono text-sm text-ink-100 outline-none placeholder:text-ink-500 focus:border-edge sm:w-64" />
      </label>
      <Button variant="secondary" disabled={loading} onclick={() => void load()}>{loading ? "Refreshing…" : "Refresh list"}</Button>
    </div>
    {#if entries.length === 0}
      <p class="mt-4 rounded-md border border-dashed border-ink-700 p-5 text-center text-sm text-ink-400">
        {loading ? "Loading the ledger…" : "No calls recorded yet. Record one above."}
      </p>
    {:else}
      <div class="mt-3 max-h-80 overflow-auto rounded-md border border-ink-800">
        <table class="w-full text-left text-sm">
          <thead class="sticky top-0 border-b border-ink-700 bg-ink-950 text-xs text-ink-400">
            <tr><th class="p-2">When</th><th class="p-2">Provider / model</th><th class="p-2 text-right">In</th><th class="p-2 text-right">Out</th><th class="p-2 text-right">Estimate</th></tr>
          </thead>
          <tbody>
            {#each entries as entry (entry.id)}
              <tr class="border-b border-ink-800 last:border-0 hover:bg-veil/5">
                <td class="whitespace-nowrap p-2 text-ink-400">{new Date(entry.at * 1000).toLocaleString()}</td>
                <td class="p-2"><span class="text-ink-200">{entry.provider}</span><span class="text-ink-500"> / </span><span class="font-mono text-ink-300">{entry.model}</span></td>
                <td class="p-2 text-right tabular-nums text-ink-300">{entry.input_tokens.toLocaleString()}</td>
                <td class="p-2 text-right tabular-nums text-ink-300">{entry.output_tokens.toLocaleString()}</td>
                <td class="p-2 text-right tabular-nums text-ink-200">${entry.estimated_cost_usd.toFixed(6)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>
</section>
