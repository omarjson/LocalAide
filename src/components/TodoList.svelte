<script lang="ts">
  // The model's to-do list for a longer task (update_todos).
  import Icon from "./Icon.svelte";
  import type { TodoItem } from "../lib/api/bindings";

  let { items, compact = false }: { items: TodoItem[]; compact?: boolean } = $props();
  const done = $derived(items.filter((t) => t.status === "done").length);
</script>

<div class={compact ? "" : "rounded-xl border border-ink-800 bg-ink-950/50 px-3 py-2.5"}>
  {#if !compact}
    <div class="mb-1.5 flex items-center gap-2 text-sm">
      <Icon name="list-check" class="h-4 w-4 text-ink-400" />
      <span class="font-medium text-ink-100">Plan</span>
      <span class="text-xs tabular-nums text-ink-500">{done} of {items.length} done</span>
    </div>
  {/if}
  <ol class="space-y-1">
    {#each items as item, i (i)}
      <li class="flex items-start gap-2 text-[13px] leading-5">
        {#if item.status === "done"}
          <span class="mt-0.5 grid h-4 w-4 shrink-0 place-items-center rounded-full bg-emerald-500/15 text-emerald-400"><Icon name="check" class="h-3 w-3" /></span>
          <span class="text-ink-500 line-through decoration-ink-600">{item.text}</span>
        {:else if item.status === "in_progress"}
          <span class="mt-0.5 h-4 w-4 shrink-0 animate-spin rounded-full border-2 border-ink-700 border-t-ember-500" aria-label="In progress"></span>
          <span class="font-medium text-ink-100">{item.text}</span>
        {:else}
          <span class="mt-0.5 h-4 w-4 shrink-0 rounded-full border border-ink-600"></span>
          <span class="text-ink-300">{item.text}</span>
        {/if}
      </li>
    {/each}
  </ol>
</div>
