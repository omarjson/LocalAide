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

<!-- The row lays out side by side only when its own column is wide
     enough to hold the text and the control: at a narrow window the
     settings column is ~440px and the slider ate the label down to one
     word a line. Container queries (not the viewport) decide, so the
     breakpoint follows the column wherever this row is used. -->
<div class={`flex flex-col gap-4 p-4 @md:min-h-16 @md:flex-row @md:items-center @md:gap-4 sm:px-[18px] ${className}`}>
  <div class="grid h-9 w-9 shrink-0 place-items-center rounded-md bg-ink-800 text-ink-200">
    <Icon name={icon} class="h-[18px] w-[18px]" />
  </div>
  <div class="min-w-0 flex-1">
    <div class="font-semibold text-ink-50">{title}</div>
    <div class="text-sm text-ink-400">{description}</div>
  </div>
  {#if control}
    <div class="flex shrink-0 items-center gap-2 @md:ml-auto">
      {@render control()}
    </div>
  {/if}
</div>
