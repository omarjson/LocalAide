<script lang="ts">
  // What left this computer: every call to an online service, newest
  // first. Previews are short and have keys removed before they are saved.
  import { onMount } from "svelte";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import type { CloudCall } from "../lib/api/bindings";

  let calls = $state<CloudCall[] | null>(null);
  let error = $state<string | null>(null);
  let kind = $state("all");
  let busy = $state(false);

  const load = async () => {
    try {
      calls = await api.cloudAuditList(300);
      error = null;
    } catch (e: unknown) {
      error = errorMessage(e);
      calls = [];
    }
  };
  onMount(() => {
    void load();
  });

  const clear = async () => {
    busy = true;
    try {
      await api.cloudAuditClear();
      await load();
    } catch (e: unknown) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  };

  const kinds = $derived(["all", ...new Set((calls ?? []).map((c) => c.kind))]);
  const shown = $derived((calls ?? []).filter((c) => kind === "all" || c.kind === kind));
  const when = (secs: number) => {
    const d = new Date(secs * 1000);
    const today = new Date().toDateString() === d.toDateString();
    return today ? d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" }) : d.toLocaleString([], { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
  };
  const size = (b: number) => (b <= 0 ? "" : b < 1024 ? `${b} B` : `${(b / 1024).toFixed(b < 10240 ? 1 : 0)} KB`);
</script>

<section aria-label="What left this computer">
  <div class="mb-2 flex flex-wrap items-center gap-2">
    <h3 class="text-[15px] font-semibold text-ink-50">What left this computer</h3>
    <span class="text-sm text-ink-500">{calls ? `${calls.length.toLocaleString()} ${calls.length === 1 ? "call" : "calls"}` : ""}</span>
    <span class="flex-1"></span>
    {#if kinds.length > 2}
      <select class="h-8 rounded-md border border-ink-700 bg-ink-950 px-2 text-sm text-ink-100" bind:value={kind} aria-label="Show calls of this kind">
        {#each kinds as k (k)}<option value={k}>{k === "all" ? "All kinds" : k}</option>{/each}
      </select>
    {/if}
    <Button variant="subtle" onclick={() => void load()}><Icon name="refresh" class="h-4 w-4" />Refresh</Button>
    <Button variant="subtle" disabled={busy || !calls?.length} onclick={() => void clear()}>Clear</Button>
  </div>
  <p class="mb-3 text-sm text-ink-400">Every request to an online service: chat with an online model, voice transcription, web search, page reads and model lists. Keys are never shown or stored here.</p>

  <div class="log">
    {#if calls === null}
      <div class="skeleton m-3 h-16"></div>
    {:else if shown.length === 0}
      <p class="px-4 py-8 text-center text-sm text-ink-400">Nothing has left this computer{kind === "all" ? "" : " for this kind"}.</p>
    {:else}
      <table class="w-full text-left text-[13px]">
        <thead>
          <tr class="text-xs text-ink-500">
            <th class="px-3 py-2 font-medium">When</th>
            <th class="px-3 py-2 font-medium">Sent to</th>
            <th class="px-3 py-2 font-medium">What</th>
            <th class="hidden px-3 py-2 font-medium md:table-cell">Preview</th>
            <th class="px-3 py-2 text-right font-medium">Result</th>
          </tr>
        </thead>
        <tbody>
          {#each shown as c (c.id)}
            <tr class="border-t border-ink-800 align-top">
              <td class="whitespace-nowrap px-3 py-2 tabular-nums text-ink-400">{when(c.timestamp)}</td>
              <td class="px-3 py-2">
                <span class="block text-ink-100">{c.provider}</span>
                <span class="block text-xs text-ink-500">{c.host}</span>
              </td>
              <td class="px-3 py-2">
                <span class="block text-ink-200">{c.kind}</span>
                {#if c.model}<span class="block max-w-[12rem] truncate font-mono text-xs text-ink-500" title={c.model}>{c.model}</span>{/if}
              </td>
              <td class="hidden max-w-[22rem] px-3 py-2 md:table-cell">
                <span class="line-clamp-2 break-words text-ink-300" title={c.preview}>{c.preview || "—"}</span>
                {#if c.bytes > 0}<span class="text-xs tabular-nums text-ink-500">{size(c.bytes)} sent</span>{/if}
              </td>
              <td class="whitespace-nowrap px-3 py-2 text-right">
                <span class={`inline-flex items-center gap-1 text-xs font-medium ${c.status === "ok" ? "text-emerald-400" : c.status === "failed" ? "text-rose-300" : "text-ink-400"}`} title={c.detail ?? ""}>
                  <Icon name={c.status === "ok" ? "check" : c.status === "failed" ? "alert" : "clock"} class="h-3.5 w-3.5" />{c.status === "ok" ? "Sent" : c.status === "failed" ? "Failed" : "Sending"}
                </span>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
  {#if error}<p class="mt-2 text-sm text-rose-300" role="alert">{error}</p>{/if}
</section>

<style>
  .log {
    max-height: 26rem;
    overflow: auto;
    border-radius: 12px;
    border: 1px solid var(--color-ink-800);
    background: var(--color-ink-950);
  }
  thead {
    position: sticky;
    top: 0;
    background: var(--color-ink-950);
  }
</style>
