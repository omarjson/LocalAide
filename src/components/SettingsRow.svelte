<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon, { type IconName } from "./Icon.svelte";

  let {
    icon = "info",
    title,
    description,
    control,
    class: className = "",
  }: {
    icon?: IconName;
    title: string;
    description: string;
    control?: Snippet;
    class?: string;
  } = $props();
</script>

<div class={`flex flex-col gap-4 p-4 sm:min-h-16 sm:flex-row sm:items-center sm:gap-4 sm:px-[18px] ${className}`}>
  <div class="grid h-9 w-9 shrink-0 place-items-center rounded-md bg-ink-800 text-ink-200">
    <Icon name={icon} class="h-[18px] w-[18px]" />
  </div>
  <div class="min-w-0 flex-1">
    <div class="font-semibold text-ink-50">{title}</div>
    <div class="text-sm text-ink-400">{description}</div>
  </div>
  {#if control}
    <div class="flex shrink-0 items-center gap-2 sm:ml-auto">
      {@render control()}
    </div>
  {/if}
</div>
