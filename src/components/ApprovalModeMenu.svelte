<script lang="ts">
  // The chat header's approval switch: how often LocalAide asks before a
  // tool changes something. Blocked commands never run in any mode.
  import { fly } from "svelte/transition";
  import Icon, { type IconName } from "./Icon.svelte";
  import { popover } from "../lib/actions/popover";
  import { dur, motion } from "../lib/motion";
  import type { ApprovalMode } from "../lib/api/bindings";

  interface Props {
    mode: ApprovalMode;
    onChange: (mode: ApprovalMode) => void;
  }
  let { mode, onChange }: Props = $props();
  let open = $state(false);

  const MODES: { id: ApprovalMode; label: string; short: string; hint: string; icon: IconName }[] = [
    { id: "ask", label: "Ask every time", short: "Ask", hint: "Commands and changes wait for your OK.", icon: "shield" },
    { id: "chat", label: "Allow for this chat", short: "This chat", hint: "No questions until you start another chat.", icon: "unlock" },
    { id: "auto", label: "Auto-approve all", short: "Auto", hint: "No questions in any chat. Destructive commands still never run, and everything is logged.", icon: "bolt" },
  ];
  const current = $derived(MODES.find((m) => m.id === mode) ?? MODES[0]);
</script>

<div class="relative" use:popover={{ open, onClose: () => (open = false) }}>
  <button
    type="button"
    aria-haspopup="menu"
    aria-expanded={open}
    aria-label={`Approvals: ${current.label}`}
    title={`Approvals: ${current.label}`}
    onclick={() => (open = !open)}
    class={`mode-btn ${mode === "auto" ? "mode-auto" : mode === "chat" ? "mode-chat" : ""}`}
  >
    <Icon name={current.icon} class="h-3.5 w-3.5 shrink-0" />
    <span class="hidden sm:inline">{current.short}</span>
    <Icon name="chevron-down" class={`h-3 w-3 shrink-0 transition-transform duration-200 ${open ? "rotate-180" : ""}`} />
  </button>
  {#if open}
    <div role="menu" aria-label="Approvals" class="absolute right-0 top-10 z-30 w-80 rounded-xl border border-edge bg-ink-950 p-1.5 shadow-lg" in:fly={{ y: -4, duration: dur(motion.base), easing: motion.enter }}>
      <div class="px-2.5 pb-1 pt-1.5 text-xs font-semibold text-ink-500">Before a tool changes something</div>
      {#each MODES as m (m.id)}
        <button
          type="button"
          role="menuitemradio"
          aria-checked={m.id === mode}
          data-popover-item
          onclick={() => { onChange(m.id); open = false; }}
          class={`flex w-full items-start gap-3 rounded-lg px-2.5 py-2 text-left transition-colors hover:bg-veil/5 ${m.id === mode ? "bg-ember-500/10" : ""}`}
        >
          <Icon name={m.icon} class={`mt-0.5 h-4 w-4 shrink-0 ${m.id === mode ? "text-ember-300" : "text-ink-400"}`} />
          <span class="min-w-0 flex-1">
            <span class="block text-sm font-semibold text-ink-50">{m.label}</span>
            <span class="block text-xs leading-5 text-ink-400">{m.hint}</span>
          </span>
          {#if m.id === mode}<Icon name="check" class="mt-1 h-4 w-4 shrink-0 text-ember-300" />{/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .mode-btn {
    display: inline-flex;
    height: 1.9rem;
    align-items: center;
    gap: 0.35rem;
    border-radius: 9999px;
    border: 1px solid var(--color-ink-700);
    padding: 0 0.6rem;
    font-size: 0.75rem;
    font-weight: 500;
    color: var(--color-ink-300);
    transition: background-color 150ms ease-out, border-color 150ms ease-out, color 150ms ease-out;
  }
  .mode-btn:hover,
  .mode-btn[aria-expanded="true"] {
    border-color: var(--color-edge);
    color: var(--color-ink-50);
  }
  .mode-chat {
    border-color: color-mix(in oklab, var(--color-ember-500) 35%, transparent);
    color: var(--color-ember-200);
  }
  .mode-auto {
    border-color: color-mix(in oklab, var(--color-ember-500) 60%, transparent);
    background: color-mix(in oklab, var(--color-ember-500) 14%, transparent);
    color: var(--color-ember-200);
  }
</style>
