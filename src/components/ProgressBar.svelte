<script lang="ts">
  /*
   * One progress bar for downloads and installs. It tells a screen reader
   * the value (the plain divs it replaces said nothing), and a paused bar
   * turns quiet grey so "stopped" reads differently from "moving".
   */
  let {
    value,
    label,
    paused = false,
    min = 0,
    class: className = "h-1.5",
  }: {
    /** 0 to 100. */
    value: number;
    /** What is progressing, e.g. "Downloading Qwen 3 4B". */
    label: string;
    paused?: boolean;
    /** Smallest visible fill, so a just-started bar still shows a sliver. */
    min?: number;
    class?: string;
  } = $props();

  const pct = $derived(Math.max(0, Math.min(100, Math.round(value || 0))));
</script>

<div
  class={`overflow-hidden rounded-full bg-ink-800 ${className}`}
  role="progressbar"
  aria-label={label}
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={pct}
  aria-valuetext={paused ? `${pct}%, paused` : `${pct}%`}
>
  <div
    class={`h-full rounded-full transition-[width,background-color] duration-200 ease-out ${paused ? "bg-ink-600" : "bg-ember-500"}`}
    style={`width:${Math.max(min, pct)}%`}
  ></div>
</div>
