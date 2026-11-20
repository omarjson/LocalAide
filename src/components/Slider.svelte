<script lang="ts">
  // The app's slider. The look lives in main.css (input[type="range"]); this
  // adds the filled part of the track so the value reads at a glance.
  let {
    value = $bindable(),
    min,
    max,
    step = 1,
    label,
    disabled = false,
    oninput,
    onchange,
    class: className = "",
  }: {
    value: number;
    min: number;
    max: number;
    step?: number;
    /** Read by screen readers. */
    label: string;
    disabled?: boolean;
    oninput?: () => void;
    onchange?: () => void;
    class?: string;
  } = $props();

  const fill = $derived.by(() => {
    const pct = max > min ? ((Number(value) - min) / (max - min)) * 100 : 0;
    return Number.isFinite(pct) ? Math.min(100, Math.max(0, pct)) : 0;
  });
</script>

<input
  type="range"
  {min}
  {max}
  {step}
  {disabled}
  bind:value
  {oninput}
  {onchange}
  aria-label={label}
  style={`--fill: ${fill}%`}
  class={className}
/>
