<script lang="ts">
  // Tappable answers for a question or an approval. Number keys pick an
  // option while the chat box is empty, so typing is never hijacked.
  import Icon from "./Icon.svelte";
  import { orderOptionsRecommended } from "../lib/questions";

  export interface Option {
    label: string;
    /** "primary" for the main choice, "quiet" for the decline. */
    tone?: "primary" | "default" | "quiet";
    /** The model's pick when the user is unsure: shown first, marked Recommended. */
    recommended?: boolean;
  }

  interface Props {
    options: Option[];
    multiple?: boolean;
    /** Adds "Other…", which hands focus to the chat box. */
    allowOther?: boolean;
    /** Listen for number keys and focus the first option. */
    active?: boolean;
    /** The answer already given, shown as chosen. */
    chosen?: string[] | null;
    onPick: (labels: string[]) => void;
    onOther?: () => void;
    ariaLabel: string;
  }

  let { options, multiple = false, allowOther = false, active = true, chosen = null, onPick, onOther, ariaLabel }: Props = $props();

  let picked = $state<string[]>([]);
  let els = $state<HTMLButtonElement[]>([]);
  const done = $derived(chosen !== null);
  // A recommended option stands first and is marked; the rest keep the
  // model's order. Number keys follow the order on screen.
  const shown = $derived(orderOptionsRecommended(options));

  $effect(() => {
    if (active && !done) {
      els[0]?.scrollIntoView({ block: "nearest" });
    }
  });

  const choose = (label: string) => {
    if (done) return;
    if (multiple) {
      picked = picked.includes(label) ? picked.filter((p) => p !== label) : [...picked, label];
    } else {
      onPick([label]);
    }
  };

  const onKey = (e: KeyboardEvent) => {
    if (!active || done || e.ctrlKey || e.metaKey || e.altKey) return;
    const target = e.target as HTMLElement | null;
    const typing = target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable);
    if (typing && (target as HTMLInputElement).value !== "") return;
    if (typing && target.tagName === "INPUT") return;
    const n = Number(e.key);
    if (Number.isInteger(n) && n >= 1 && n <= shown.length) {
      e.preventDefault();
      choose(shown[n - 1].label);
    } else if (multiple && e.key === "Enter" && picked.length > 0 && !typing) {
      e.preventDefault();
      onPick(picked);
    }
  };
</script>

<svelte:window onkeydown={onKey} />

<div role="group" aria-label={ariaLabel} class="flex flex-wrap gap-2">
  {#each shown as option, i (option.label)}
    {@const isOn = done ? chosen!.includes(option.label) : picked.includes(option.label)}
    <button
      bind:this={els[i]}
      type="button"
      disabled={done && !isOn}
      aria-pressed={multiple || done ? isOn : undefined}
      onclick={() => choose(option.label)}
      class={`opt opt-${option.tone ?? "default"} ${isOn ? "opt-on" : ""}`}
    ><span class="opt-key" aria-hidden="true">{i + 1}</span><span>{option.label}</span>{#if option.recommended}<span class="opt-rec">Recommended</span>{/if}{#if isOn && (multiple || done)}<Icon name="check" class="h-3.5 w-3.5" />{/if}</button>
  {/each}
  {#if allowOther && !done}
    <button type="button" onclick={() => onOther?.()} class="opt opt-quiet"><Icon name="pencil" class="h-3.5 w-3.5" /><span>Other…</span></button>
  {/if}
  {#if multiple && !done}
    <button type="button" disabled={picked.length === 0} onclick={() => onPick(picked)} class="opt opt-primary">Send {picked.length || ""}</button>
  {/if}
</div>
{#if active && !done}
  <p class="mt-2 text-xs text-ink-500">Press {shown.length === 1 ? "1" : `1–${shown.length}`} to choose{multiple ? ", Enter to send" : ""}{allowOther ? ", or type your own answer with Other…" : ""}.</p>
{/if}

<style>
  .opt {
    display: inline-flex;
    min-height: 2.25rem;
    align-items: center;
    gap: 0.5rem;
    border-radius: 0.6rem;
    border: 1px solid var(--color-ink-700);
    background: var(--color-ink-900);
    padding: 0.35rem 0.75rem 0.35rem 0.4rem;
    font-size: 0.875rem;
    line-height: 1.25rem;
    color: var(--color-ink-100);
    text-align: left;
    transition: background-color 120ms ease-out, border-color 120ms ease-out, transform 120ms ease-out;
  }
  .opt:not(:disabled):hover {
    border-color: var(--color-edge);
    background: var(--color-ink-800);
  }
  .opt:not(:disabled):active {
    transform: scale(0.98);
  }
  .opt:disabled {
    cursor: default;
    opacity: 0.45;
  }
  .opt:focus-visible {
    outline: 2px solid var(--color-ember-500);
    outline-offset: 2px;
  }
  .opt-key {
    display: grid;
    height: 1.4rem;
    min-width: 1.4rem;
    place-items: center;
    border-radius: 0.4rem;
    background: var(--color-ink-800);
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 0.75rem;
    color: var(--color-ink-300);
  }
  .opt-primary {
    border-color: transparent;
    background: var(--color-ink-100);
    color: var(--color-on-ember);
    font-weight: 600;
  }
  .opt-primary:not(:disabled):hover {
    background: var(--color-ink-50);
    border-color: transparent;
  }
  .opt-primary .opt-key {
    background: color-mix(in oklab, var(--color-on-ember) 18%, transparent);
    color: var(--color-on-ember);
  }
  .opt-quiet {
    background: transparent;
    color: var(--color-ink-300);
  }
  .opt-on {
    border-color: var(--color-ink-600);
    background: var(--color-ink-700);
    color: var(--color-ink-50);
  }
  .opt-on .opt-key {
    background: var(--color-ink-800);
    color: var(--color-ink-200);
  }
  .opt-rec {
    border-radius: 9999px;
    background: var(--color-ink-700);
    padding: 0.05rem 0.45rem;
    font-size: 0.6875rem;
    font-weight: 600;
    color: var(--color-ink-200);
  }
</style>
