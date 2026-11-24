<script lang="ts">
  // The permission mode, as one icon in the chat box: Plan mode, Ask
  // before changes, Autonomous (asks only when there is risk) and Full
  // access. Blocked commands never run in any mode.
  import { fly } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import { popover } from "../lib/actions/popover";
  import { dur, motion } from "../lib/motion";
  import { PERMISSION_MODES, permissionInfo } from "../lib/chatModes";
  import type { ApprovalMode } from "../lib/api/bindings";

  interface Props {
    mode: ApprovalMode;
    onChange: (mode: ApprovalMode) => void;
  }

  let { mode, onChange }: Props = $props();
  let open = $state(false);

  // The list is shared with the approval prompts and Settings (chatModes.ts).
  const current = $derived(permissionInfo(mode));
</script>

<div class="relative" use:popover={{ open, onClose: () => (open = false) }}>
  <button
    type="button"
    aria-haspopup="menu"
    aria-expanded={open}
    aria-label={`Permissions: ${current.label}`}
    title={`${current.label}: ${current.hint}`}
    onclick={() => (open = !open)}
    class={`perm-btn perm-${current.id}`}
  >
    <Icon name={current.icon} class="h-[18px] w-[18px]" />
  </button>
  {#if open}
    <div role="menu" aria-label="Permissions" class="perm-menu" in:fly={{ y: 6, duration: dur(motion.base), easing: motion.enter }}>
      {#each PERMISSION_MODES as m, i (m.id)}
        {#if i === 1}<div class="my-1 h-px bg-ink-800" role="separator"></div>{/if}
        <button
          type="button"
          role="menuitemradio"
          aria-checked={m.id === mode}
          data-popover-item
          onclick={() => { onChange(m.id); open = false; }}
          class={`perm-item ${m.id === mode ? "perm-item-on" : ""}`}
        >
          <Icon name={m.icon} class="mt-0.5 h-[18px] w-[18px] shrink-0 text-ink-300" />
          <span class="min-w-0 flex-1">
            <span class="block text-sm font-medium text-ink-50">{m.label}</span>
            <span class="block text-[13px] leading-5 text-ink-400">{m.hint}</span>
          </span>
          {#if m.id === mode}<Icon name="check" class="mt-1 h-4 w-4 shrink-0 text-ink-100" />{/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .perm-btn {
    display: grid;
    height: 2rem;
    width: 2rem;
    place-items: center;
    border-radius: 0.6rem;
    color: var(--color-ink-400);
    transition: background-color 150ms ease-out, color 150ms ease-out;
  }
  .perm-btn:hover,
  .perm-btn[aria-expanded="true"] {
    background: color-mix(in oklab, var(--color-veil) 6%, transparent);
    color: var(--color-ink-50);
  }
  .perm-plan {
    color: #7dd3fc;
  }
  /* Sky-300 is ~1.6:1 on the light page; sky-700 clears 4.5:1. */
  :global([data-theme="light"]) .perm-plan {
    color: #0369a1;
  }
  .perm-smart {
    color: var(--color-ink-200);
  }
  .perm-auto {
    color: var(--color-ember-300);
  }
  .perm-menu {
    position: absolute;
    bottom: calc(100% + 0.5rem);
    left: 0;
    z-index: 40;
    width: 16.5rem;
    border-radius: 0.8rem;
    border: 1px solid var(--color-edge);
    background: var(--color-ink-950);
    padding: 0.35rem;
    box-shadow: 0 20px 44px -18px rgb(0 0 0 / 0.6);
    transform-origin: bottom left;
  }
  .perm-item {
    display: flex;
    width: 100%;
    align-items: flex-start;
    gap: 0.7rem;
    border-radius: 0.55rem;
    padding: 0.5rem 0.6rem;
    text-align: left;
    transition: background-color 120ms ease-out;
  }
  .perm-item:hover,
  .perm-item:focus {
    background: color-mix(in oklab, var(--color-veil) 6%, transparent);
    outline: none;
  }
  .perm-item-on {
    background: color-mix(in oklab, var(--color-veil) 8%, transparent);
  }
</style>
