<script lang="ts">
  // A visible countdown on an approval prompt. It starts when it mounts, so
  // a parent stops it by unmounting it (the user chose, or the prompt went
  // away). `onExpire` runs once, at zero. Time spent paused (the parent
  // says so, or the window is hidden) does not count, so nothing runs
  // while the user cannot see it counting.
  import { onMount } from "svelte";
  import { formatCountdown } from "../lib/countdown";

  interface Props {
    seconds: number;
    /** Words before the time, e.g. "Runs in". */
    label: string;
    /** Words after the time, e.g. "unless you choose Don't run." */
    suffix?: string;
    paused?: boolean;
    /** Why it is paused, e.g. "Paused while you type." */
    pausedLabel?: string;
    onExpire: () => void;
  }

  let { seconds, label, suffix = "", paused = false, pausedLabel = "Paused.", onExpire }: Props = $props();

  let remaining = $state(0);
  let hidden = $state(false);
  let fired = false;
  // What screen readers hear: the start, ten seconds left, and pauses —
  // not every tick.
  let announcement = $state("");

  const isPaused = $derived(paused || hidden);
  const fraction = $derived(seconds > 0 ? Math.max(0, Math.min(1, remaining / seconds)) : 0);

  onMount(() => {
    remaining = seconds;
    announcement = `${label} ${formatCountdown(seconds)} ${suffix}`.trim();
    let left = seconds * 1000;
    let last = performance.now();
    let warned = false;
    const onVisibility = () => (hidden = document.visibilityState === "hidden");
    onVisibility();
    document.addEventListener("visibilitychange", onVisibility);
    const tick = () => {
      const now = performance.now();
      const step = now - last;
      last = now;
      if (fired || isPaused) return;
      left -= step;
      remaining = Math.max(0, left / 1000);
      if (!warned && left <= 10_000 && seconds > 10) {
        warned = true;
        announcement = `${label} ${formatCountdown(remaining)} ${suffix}`.trim();
      }
      if (left <= 0) {
        fired = true;
        onExpire();
      }
    };
    const timer = setInterval(tick, 250);
    return () => {
      clearInterval(timer);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  });

  $effect(() => {
    if (isPaused && !fired) announcement = pausedLabel;
  });
</script>

<div class="countdown" data-paused={isPaused ? "" : undefined}>
  <p class="text-sm text-ink-200">
    {#if isPaused}
      <span class="text-ink-400">{pausedLabel}</span>
      <span class="tabular-nums text-ink-400">({formatCountdown(remaining)} left)</span>
    {:else}
      {label} <strong class="font-semibold tabular-nums text-ink-50">{formatCountdown(remaining)}</strong>{#if suffix}{" "}{suffix}{/if}
    {/if}
  </p>
  <div class="countdown-track" aria-hidden="true">
    <div class="countdown-fill" style={`transform: scaleX(${fraction})`}></div>
  </div>
  <span class="sr-only" aria-live="polite">{announcement}</span>
</div>

<style>
  .countdown-track {
    margin-top: 0.35rem;
    height: 3px;
    overflow: hidden;
    border-radius: 9999px;
    background: var(--color-ink-800);
  }
  .countdown-fill {
    height: 100%;
    width: 100%;
    transform-origin: left center;
    background: var(--color-ember-500);
    transition: transform 250ms linear;
  }
  .countdown[data-paused] .countdown-fill {
    background: var(--color-ink-600);
  }
  @media (prefers-reduced-motion: reduce) {
    .countdown-fill {
      transition: none;
    }
  }
</style>
