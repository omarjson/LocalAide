<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon, { type IconName } from "./Icon.svelte";

  let {
    icon = "info",
    title,
    description,
    action,
  }: {
    icon?: IconName;
    title: string;
    description: string;
    action?: Snippet;
  } = $props();
</script>

<div class="rounded-lg border border-ink-700 bg-ink-900 px-6 py-14 text-center">
  <div class="mx-auto grid h-10 w-10 place-items-center rounded-md bg-ink-800 text-ink-300">
    <Icon name={icon} class="h-5 w-5" />
  </div>
  <h2 class="mt-4 font-semibold text-ink-50">{title}</h2>
  <p class="mx-auto mt-1 max-w-md text-sm leading-6 text-ink-400">{description}</p>
  {#if action}<div class="mt-4 flex justify-center">{@render action()}</div>{/if}
</div>
