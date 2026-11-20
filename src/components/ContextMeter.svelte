<script lang="ts">
  import { fly } from "svelte/transition";
  import { popover } from "../lib/actions/popover";
  import { dur, motion } from "../lib/motion";
  import { contextLabel } from "../lib/chatParts";
  import type { ContextUsage } from "../lib/contextUsage";

  interface Props {
    usage: ContextUsage;
    streaming?: boolean;
    autoCompactAt: number;
    /** Older messages are already left out. */
    compacted?: boolean;
    onCompact?: () => void;
  }

  let { usage, streaming = false, autoCompactAt, compacted = false, onCompact }: Props = $props();
  let open = $state(false);

  const known = $derived(usage.limit > 0);
  // Ember until close to the auto-compact line, then a deeper burnt orange,
  // so the meter warns without leaving the palette.
  const warn = $derived(known && (usage.overflow || usage.percent >= autoCompactAt));
  const tone = $derived(!known ? "var(--color-ink-600)" : warn ? "var(--color-ember-700)" : "var(--color-ember-500)");
  const fmt = (n: number) => (n >= 1000 ? `${(n / 1000).toFixed(n >= 10_000 ? 0 : 1)}K` : String(n));
</script>

<div class="relative" use:popover={{ open, onClose: () => (open = false) }}>
  <button
    type="button"
    class="meter-button"
    aria-haspopup="dialog"
    aria-expanded={open}
    aria-label={known ? `Context: about ${Math.round(usage.percent)}% used` : "Context: size unknown"}
    title={known ? `About ${Math.round(usage.percent)}% of the context window used` : "Context window"}
    onclick={() => (open = !open)}
  >
    <span class="meter" style={`--usage:${known ? usage.percent : 0}%;--tone:${tone}`} aria-hidden="true"></span>
  </button>
  {#if open}
    <div class="meter-popover" role="dialog" aria-label="Context window" in:fly={{ y: 6, duration: dur(motion.base), easing: motion.enter }}>
      <div class="flex items-baseline justify-between gap-3">
        <span class="font-semibold text-ink-50">Context window</span>
        <span class="tabular-nums text-ink-300">{known ? `~${fmt(usage.used)} / ${contextLabel(usage.limit)}` : "Size unknown"}</span>
      </div>
      {#if known}
        <div class="mt-2 h-1.5 overflow-hidden rounded-full bg-ink-800">
          <div class="h-full rounded-full transition-[width] duration-300" style={`width:${usage.percent}%;background:${tone}`}></div>
        </div>
      {/if}
      <dl class="mt-3 space-y-1 text-xs">
        <div class="flex justify-between"><dt class="text-ink-400">Chat so far</dt><dd class="tabular-nums text-ink-200">~{fmt(usage.input - usage.output)}</dd></div>
        <div class="flex justify-between"><dt class="text-ink-400">Answers so far</dt><dd class="tabular-nums text-ink-200">~{fmt(usage.output)}</dd></div>
        <div class="flex justify-between"><dt class="text-ink-400">Instructions</dt><dd class="tabular-nums text-ink-200">~{fmt(usage.overhead)}</dd></div>
        <div class="flex justify-between"><dt class="text-ink-400">Kept free for the answer</dt><dd class="tabular-nums text-ink-200">{fmt(usage.reserve)}</dd></div>
        {#if known}
          <div class="flex justify-between border-t border-ink-800 pt-1"><dt class="text-ink-400">Room left</dt><dd class={`tabular-nums ${usage.overflow ? "font-semibold text-ember-300" : "text-ink-200"}`}>{usage.overflow ? "Not enough for the answer" : `~${fmt(usage.limit - usage.total)}`}</dd></div>
        {/if}
      </dl>
      <p class="mt-3 text-xs leading-5 text-ink-500">
        Estimated from text length; the model's own count can differ. Cached tokens are not reported by this engine.
        {streaming ? "The answer is still growing." : known ? `At ${autoCompactAt}%, older messages stop being sent.` : ""}
      </p>
      {#if onCompact}
        <button type="button" onclick={() => { open = false; onCompact?.(); }} class="mt-2 w-full rounded-lg border border-edge px-3 py-1.5 text-sm text-ink-100 transition-colors hover:bg-veil/5">
          {compacted ? "Compact further" : "Compact now"}
        </button>
      {/if}
    </div>
  {/if}
</div>

<style>
  .meter-button { display: grid; height: 1.75rem; width: 1.75rem; place-items: center; border-radius: 9999px; transition: background-color 150ms ease-out; }
  .meter-button:hover, .meter-button[aria-expanded="true"] { background: color-mix(in oklab, var(--color-veil) 7%, transparent); }
  .meter { height: 1.15rem; width: 1.15rem; border-radius: 9999px; background: conic-gradient(var(--tone) var(--usage), var(--color-ink-700) 0); -webkit-mask: radial-gradient(farthest-side, transparent calc(100% - 3px), #000 calc(100% - 3px)); mask: radial-gradient(farthest-side, transparent calc(100% - 3px), #000 calc(100% - 3px)); transition: background 300ms ease-out; }
  .meter-popover { position: absolute; bottom: calc(100% + 0.5rem); right: 0; z-index: 45; width: 17.5rem; border: 1px solid var(--color-edge); border-radius: 0.75rem; background: var(--color-ink-950); padding: 0.75rem; box-shadow: 0 18px 40px -16px rgb(0 0 0 / 0.55); }
</style>
