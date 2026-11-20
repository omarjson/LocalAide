<script lang="ts">
  import { slide } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import { dur, motion } from "../lib/motion";
  import { thinkingLabel } from "../lib/chatParts";

  interface Props {
    text: string;
    /** How long the model thought; null while it still thinks. */
    ms?: number | null;
    /** The answer is still streaming. */
    live?: boolean;
  }

  let { text, ms = null, live = false }: Props = $props();
  let open = $state(false);

  const thinking = $derived(live && (ms === null || ms === undefined));
  // While the model thinks, its newest line shows under the header, so the
  // block feels alive without filling the screen.
  const lastLine = $derived.by(() => {
    const lines = text.trim().split(/\n+/);
    return (lines.at(-1) ?? "").slice(-160);
  });
</script>

<div class="thinking">
  <button
    type="button"
    class="thinking-head"
    aria-expanded={open}
    onclick={() => (open = !open)}
    title={open ? "Hide the thinking" : "Show the thinking"}
  >
    <Icon name="brain" class={`h-4 w-4 shrink-0 ${thinking ? "text-ember-300" : "text-ink-400"}`} />
    <span class={thinking ? "shimmer font-medium" : "font-medium text-ink-300"}>{thinkingLabel(thinking ? null : ms)}</span>
    <Icon name="chevron-down" class={`h-3.5 w-3.5 shrink-0 text-ink-500 transition-transform duration-200 ${open ? "rotate-180" : ""}`} />
  </button>
  {#if !open && thinking && lastLine}
    <p class="thinking-preview" aria-hidden="true">{lastLine}</p>
  {/if}
  {#if open}
    <div class="thinking-body" transition:slide={{ duration: dur(motion.base) }}>
      <p class="whitespace-pre-wrap break-words">{text.trim()}</p>
    </div>
  {/if}
</div>

<style>
  .thinking {
    margin-bottom: 0.5rem;
  }
  .thinking-head {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    border-radius: 0.5rem;
    padding: 0.25rem 0.5rem 0.25rem 0.25rem;
    margin-left: -0.25rem;
    font-size: 0.875rem;
    transition: background-color 150ms ease-out;
  }
  .thinking-head:hover {
    background: color-mix(in oklab, var(--color-veil) 6%, transparent);
  }
  .thinking-preview {
    margin: 0.125rem 0 0 1.6rem;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: 0.8125rem;
    color: var(--color-ink-500);
  }
  .thinking-body {
    margin: 0.35rem 0 0.25rem 0.45rem;
    max-height: 22rem;
    overflow-y: auto;
    border-left: 1px solid var(--color-ink-700);
    padding: 0.15rem 0 0.15rem 1rem;
    font-size: 0.875rem;
    line-height: 1.6rem;
    color: var(--color-ink-400);
  }
  /* A soft breathing label while the model thinks. */
  .shimmer {
    color: var(--color-ink-100);
    animation: breathe 1.6s ease-in-out infinite;
  }
  @keyframes breathe {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.5;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .shimmer {
      animation: none;
    }
  }
  :global(:root[data-motion="off"]) .shimmer {
    animation: none;
  }
</style>
