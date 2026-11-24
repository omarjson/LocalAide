<script lang="ts">
  // A dialog with a dimmed backdrop. Escape and a click outside close it;
  // focus stays inside while it is open.
  import type { Snippet } from "svelte";
  import { fade, scale } from "svelte/transition";
  import FocusTrap from "./FocusTrap.svelte";
  import Icon from "./Icon.svelte";
  import { dur, enterDialog, leaveDialog, motion } from "../lib/motion";

  let {
    title,
    onClose,
    size = "md",
    children,
    footer,
    motion: animate = true,
  }: {
    title: string;
    onClose: () => void;
    size?: "md" | "lg";
    children: Snippet;
    footer?: Snippet;
    /** False renders the dialog instantly: chat search opens often enough
        that the scale-in reads as extra motion. */
    motion?: boolean;
  } = $props();

  const width = $derived(size === "lg" ? "max-w-3xl" : "max-w-md");
</script>

<div
  role="presentation"
  in:fade={{ duration: animate ? dur(motion.fast) : 0 }}
  out:fade={{ duration: animate ? dur(motion.fast) : 0 }}
  class="fixed inset-0 z-[70] grid place-items-center bg-black/60 p-4 backdrop-blur-sm"
  onclick={onClose}
  onkeydown={(e) => { if (e.key === "Escape") onClose(); }}
>
  <div
    role="dialog"
    tabindex="-1"
    aria-modal="true"
    aria-label={title}
    in:scale={animate ? enterDialog() : { start: 1, duration: 0 }}
    out:scale={animate ? leaveDialog() : { start: 1, duration: 0 }}
    class={`flex max-h-[85vh] w-full ${width} flex-col overflow-hidden rounded-xl border border-edge bg-ink-900 shadow-2xl`}
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => { if (e.key === "Escape") onClose(); e.stopPropagation(); }}
  >
    <FocusTrap>
      <header class="flex shrink-0 items-center justify-between gap-3 border-b border-ink-800 px-5 py-3">
        <h2 class="font-semibold text-ink-50">{title}</h2>
        <button type="button" aria-label="Close" onclick={onClose} class="grid h-8 w-8 place-items-center rounded-md text-ink-400 transition-colors hover:bg-veil/5 hover:text-ink-50">
          <Icon name="close" class="h-4 w-4" />
        </button>
      </header>
      <div class="min-h-0 flex-1 overflow-y-auto px-5 py-4">
        {@render children()}
      </div>
      {#if footer}
        <footer class="flex shrink-0 justify-end gap-2 border-t border-ink-800 px-5 py-3">
          {@render footer()}
        </footer>
      {/if}
    </FocusTrap>
  </div>
</div>
