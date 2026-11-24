<script lang="ts">
  // The permission mode an approval was asked in, as a small chip. Same
  // label and icon as the permission menu (chatModes.ts).
  import Icon from "./Icon.svelte";
  import { permissionInfo } from "../lib/chatModes";
  import type { ApprovalMode } from "../lib/api/bindings";

  let { mode, class: className = "" }: { mode: ApprovalMode; class?: string } = $props();
  const info = $derived(permissionInfo(mode));
</script>

<span class={`mode-chip mode-chip-${info.id} ${className}`} title={`Asked in ${info.label}: ${info.hint}`}>
  <Icon name={info.icon} class="h-3.5 w-3.5 shrink-0" />
  <span><span class="sr-only">Permission: </span>{info.label}</span>
</span>

<style>
  .mode-chip {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    border-radius: 9999px;
    border: 1px solid var(--color-ink-700);
    padding: 0.05rem 0.5rem;
    font-size: 0.8125rem;
    font-weight: 500;
    line-height: 1.35rem;
    color: var(--color-ink-200);
    white-space: nowrap;
  }
  /* Plan uses the same sky as its menu icon; light theme darkens it. */
  .mode-chip-plan {
    color: #7dd3fc;
    border-color: color-mix(in oklab, #7dd3fc 35%, transparent);
  }
  :global([data-theme="light"]) .mode-chip-plan {
    color: #0369a1;
    border-color: color-mix(in oklab, #0369a1 35%, transparent);
  }
  .mode-chip-auto {
    color: var(--color-ember-300);
  }
</style>
