<script lang="ts">
  import { fly } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import { popover } from "../lib/actions/popover";
  import { dur, motion } from "../lib/motion";
  import { effortLabel } from "../lib/chatParts";

  interface Props {
    /** The model's levels, lowest first. */
    efforts: string[];
    value: string;
    onChange: (effort: string) => void;
  }

  let { efforts, value, onChange }: Props = $props();
  let open = $state(false);

  const options = $derived(["auto", ...efforts.filter((e) => e !== "auto")]);
  const hints: Record<string, string> = {
    auto: "The model's own default.",
    off: "Answer right away, without thinking first.",
    on: "Think before answering.",
    minimal: "A quick look before answering.",
    low: "Think briefly. Faster answers.",
    medium: "A balance of speed and care.",
    high: "Think longer for hard questions. Slower.",
  };
</script>

<div class="relative" use:popover={{ open, onClose: () => (open = false) }}>
  <button
    type="button"
    aria-haspopup="menu"
    aria-expanded={open}
    onclick={() => (open = !open)}
    class={`effort-chip ${value !== "auto" && value !== "off" ? "effort-on" : ""}`}
    title="How hard the model thinks"
  >
    <Icon name="brain" class="h-3.5 w-3.5" />
    <span class="hidden sm:inline">{value === "auto" ? "Thinking" : effortLabel(value)}</span>
  </button>
  {#if open}
    <div
      role="menu"
      aria-label="Thinking effort"
      class="absolute bottom-full right-0 z-40 mb-2 w-64 overflow-hidden rounded-xl border border-edge bg-ink-950 p-1.5 shadow-2xl"
      in:fly={{ y: 6, duration: dur(motion.base), easing: motion.enter }}
    >
      <div class="px-2.5 pb-1 pt-1.5 text-xs font-semibold text-ink-500">Thinking</div>
      {#each options as option (option)}
        <button
          type="button"
          role="menuitemradio"
          aria-checked={option === value}
          data-popover-item
          onclick={() => { onChange(option); open = false; }}
          class={`flex w-full items-start gap-2.5 rounded-lg px-2.5 py-2 text-left transition-colors hover:bg-veil/5 focus:bg-veil/5 focus:outline-none ${option === value ? "bg-ember-500/10" : ""}`}
        >
          <span class="min-w-0 flex-1">
            <span class="block text-sm font-semibold text-ink-50">{effortLabel(option)}</span>
            {#if hints[option]}<span class="block text-xs leading-5 text-ink-400">{hints[option]}</span>{/if}
          </span>
          {#if option === value}<Icon name="check" class="mt-1 h-4 w-4 shrink-0 text-ember-300" />{/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .effort-chip {
    display: inline-flex;
    height: 1.75rem;
    align-items: center;
    gap: 0.375rem;
    border-radius: 9999px;
    border: 1px solid var(--color-ink-700);
    padding: 0 0.55rem;
    font-size: 0.75rem;
    color: var(--color-ink-300);
    transition: background-color 150ms ease-out, color 150ms ease-out, border-color 150ms ease-out;
  }
  .effort-chip:hover,
  .effort-chip[aria-expanded="true"] {
    border-color: var(--color-edge);
    color: var(--color-ink-50);
  }
  .effort-on {
    border-color: color-mix(in oklab, var(--color-ember-500) 45%, transparent);
    background: color-mix(in oklab, var(--color-ember-500) 12%, transparent);
    color: var(--color-ember-200);
  }
</style>
