<script lang="ts">
  import Icon from "./Icon.svelte";
  import { contextLabel } from "../lib/chatParts";
  import type { ChatMessage } from "../lib/api/bindings";

  interface Props {
    messages: ChatMessage[];
    context: number | null | undefined;
    streaming?: boolean;
  }

  let { messages, context = null, streaming = false }: Props = $props();
  let open = $state(false);

  const estimate = (text: string) => Math.ceil(text.length / 4);
  const inputTokens = $derived(messages.filter((m) => m.role === "user").reduce((sum, m) => sum + estimate(m.content), 0));
  const outputTokens = $derived(messages.filter((m) => m.role === "assistant").reduce((sum, m) => sum + estimate(m.content), 0));
  const used = $derived(Math.max(0, inputTokens + outputTokens));
  const limit = $derived(context ?? 0);
  const percent = $derived(limit > 0 ? Math.min(100, (used / limit) * 100) : 0);
  const color = $derived(percent >= 90 ? "#c2410c" : percent >= 70 ? "#d97706" : "var(--color-ember-500)");

  const formatTokens = (value: number) => (value >= 1000 ? `${(value / 1000).toFixed(value >= 10000 ? 0 : 1)}K` : String(value));
</script>

<div class="relative" title="Context window usage">
  <button
    type="button"
    class="meter-button"
    aria-label={`Context usage: ${formatTokens(used)} of ${contextLabel(limit) || "unknown"} tokens`}
    aria-expanded={open}
    onclick={() => (open = !open)}
  >
    <span class="meter" style={`--usage:${percent}%;--meter-color:${color}`} aria-hidden="true"><span></span></span>
  </button>
  {#if open}
    <div class="meter-popover" role="dialog" aria-label="Context usage">
      <div class="flex items-center justify-between gap-3">
        <span class="font-semibold text-ink-50">Context window</span>
        <span class="tabular-nums text-ink-300">{formatTokens(used)} / {contextLabel(limit) || "—"}</span>
      </div>
      <div class="mt-2 h-1.5 overflow-hidden rounded-full bg-ink-800"><div class="h-full rounded-full bg-ember-500 transition-[width]" style={`width:${percent}%`}></div></div>
      <dl class="mt-3 grid grid-cols-2 gap-x-5 gap-y-1 text-xs">
        <div><dt class="text-ink-500">Input</dt><dd class="tabular-nums text-ink-200">~{formatTokens(inputTokens)} tokens</dd></div>
        <div><dt class="text-ink-500">Output</dt><dd class="tabular-nums text-ink-200">~{formatTokens(outputTokens)} tokens</dd></div>
        <div><dt class="text-ink-500">Cached input</dt><dd class="text-ink-300">Engine not reporting</dd></div>
        <div><dt class="text-ink-500">Cached output</dt><dd class="text-ink-300">Not applicable</dd></div>
      </dl>
      <p class="mt-2 text-xs leading-5 text-ink-500">Usage is estimated from text length. {streaming ? "The current answer is still growing." : "Older turns are compacted when the window fills."}</p>
    </div>
  {/if}
</div>

<style>
  .meter-button { display: grid; height: 2rem; width: 2rem; place-items: center; border-radius: 9999px; color: var(--color-ink-300); transition: background-color 150ms ease-out; }
  .meter-button:hover, .meter-button[aria-expanded="true"] { background: color-mix(in oklab, var(--color-veil) 7%, transparent); }
  .meter { position: relative; display: grid; height: 1.35rem; width: 1.35rem; place-items: center; border-radius: 9999px; background: conic-gradient(var(--meter-color) var(--usage), var(--color-ink-700) 0); }
  .meter::before { content: ""; position: absolute; inset: 2px; border-radius: 9999px; background: var(--color-ink-950); }
  .meter span { position: relative; height: 0.25rem; width: 0.25rem; border-radius: 9999px; background: var(--meter-color); }
  .meter-popover { position: absolute; bottom: calc(100% + 0.5rem); right: 0; z-index: 45; width: 17rem; border: 1px solid var(--color-edge); border-radius: 0.75rem; background: var(--color-ink-950); padding: 0.75rem; box-shadow: 0 18px 40px -16px rgb(0 0 0 / .55); }
</style>
