<script lang="ts">
  // The in-chat search bar. The search itself (matches, jumping, the
  // virtualizer) stays in ChatScreen; this is its markup.
  import { fly } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import { dur, motion } from "../lib/motion";

  interface Props {
    open: boolean;
    query: string;
    input?: HTMLInputElement;
    index: number;
    matchCount: number;
    onKeydown: (e: KeyboardEvent) => void;
    onPrevious: () => void;
    onNext: () => void;
    onClose: () => void;
  }

  let { open, query = $bindable(), input = $bindable(), index, matchCount, onKeydown, onPrevious, onNext, onClose }: Props = $props();
</script>

{#if open}
  <div
    class="absolute left-1/2 top-3 z-30 flex w-[min(30rem,90%)] -translate-x-1/2 items-center gap-1 rounded-xl border border-ink-700 bg-ink-900 px-2 py-1.5 shadow-[var(--shadow-elev)]"
    role="search"
    in:fly={{ y: -6, duration: dur(motion.base), easing: motion.enter }}
  >
    <Icon name="search" class="h-4 w-4 shrink-0 text-ink-400" />
    <input
      bind:this={input}
      bind:value={query}
      onkeydown={onKeydown}
      aria-label="Search in this chat"
      placeholder="Search in this chat"
      class="field-bare h-7 min-w-0 flex-1 bg-transparent text-sm text-ink-50 placeholder:text-ink-500"
    />
    {#if query.trim()}
      <span class="shrink-0 text-xs tabular-nums text-ink-500" aria-live="polite">
        {matchCount === 0 ? "No matches" : `${index + 1} of ${matchCount}`}
      </span>
    {/if}
    <button type="button" aria-label="Previous match" disabled={matchCount === 0} onclick={onPrevious} class="chat-icon-btn h-7 w-7 disabled:opacity-40"><Icon name="chevron-down" class="h-4 w-4 rotate-180" /></button>
    <button type="button" aria-label="Next match" disabled={matchCount === 0} onclick={onNext} class="chat-icon-btn h-7 w-7 disabled:opacity-40"><Icon name="chevron-down" class="h-4 w-4" /></button>
    <button type="button" aria-label="Close search" onclick={onClose} class="chat-icon-btn h-7 w-7"><Icon name="close" class="h-4 w-4" /></button>
  </div>
{/if}

<style>
  .chat-icon-btn {
    display: grid;
    height: 2.25rem;
    width: 2.25rem;
    place-items: center;
    border-radius: 0.6rem;
    color: var(--color-ink-400);
    transition: background-color 150ms ease-out, color 150ms ease-out;
  }
  .chat-icon-btn:hover {
    background: color-mix(in oklab, var(--color-veil) 6%, transparent);
    color: var(--color-ink-50);
  }
</style>
