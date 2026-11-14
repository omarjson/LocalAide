<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    title,
    description,
    actions,
  }: {
    title: string;
    description: string;
    actions?: Snippet;
  } = $props();
</script>

<header class="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between mb-6">
  <div class="min-w-0">
    <h1 class="text-[28px] leading-tight font-semibold tracking-tight text-ink-50">{title}</h1>
    <p class="mt-1 text-sm text-ink-400 max-w-2xl">{description}</p>
  </div>
  {#if actions}
    <div class="flex items-center gap-2 shrink-0 sm:pt-1">
      {@render actions()}
    </div>
  {/if}
</header>
