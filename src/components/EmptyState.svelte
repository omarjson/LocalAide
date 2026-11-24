<script lang="ts">
  import type { Snippet } from "svelte";
  import { fly } from "svelte/transition";
  import Icon, { type IconName } from "./Icon.svelte";
  import { dur, motion } from "../lib/motion";

  let {
    icon = "info",
    title,
    description,
    action,
    /** One short time-of-day greeting line, shown above the title. */
    greeting,
  }: {
    icon?: IconName;
    title: string;
    description: string;
    action?: Snippet;
    greeting?: string;
  } = $props();
</script>

<div class="ember-glow rounded-lg border border-ink-700 bg-ink-900 px-6 py-14 text-center" in:fly={{ y: 6, duration: dur(motion.slow), easing: motion.enter }}>
  <div class="mx-auto grid h-12 w-12 place-items-center rounded-xl border border-ink-700 bg-ink-800 text-ember-300 shadow-sm">
    <Icon name={icon} class="h-6 w-6" />
  </div>
  {#if greeting}<p class="mt-4 text-sm text-ink-400">{greeting}</p>{/if}
  <h2 class="mt-4 font-semibold text-ink-50">{title}</h2>
  <p class="mx-auto mt-1 max-w-md text-sm leading-6 text-ink-400">{description}</p>
  {#if action}<div class="mt-4 flex justify-center">{@render action()}</div>{/if}
</div>
