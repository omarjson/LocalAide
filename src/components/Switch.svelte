<script lang="ts">
  // The one on/off switch used across Settings. The knob is positioned from
  // the left in both states, so it never jumps or leaves the track.
  let {
    checked,
    onchange,
    disabled = false,
    label,
    title,
    class: className = "",
  }: {
    checked: boolean;
    /** Called with the new value when the user flips the switch. */
    onchange: (next: boolean) => void;
    disabled?: boolean;
    /** Read by screen readers. */
    label: string;
    title?: string;
    class?: string;
  } = $props();
</script>

<button
  type="button"
  role="switch"
  aria-checked={checked}
  aria-label={label}
  {title}
  {disabled}
  onclick={() => onchange(!checked)}
  class={`switch relative inline-flex h-6 w-11 shrink-0 items-center rounded-full border transition-colors duration-200 ease-out disabled:cursor-default disabled:opacity-50 ${checked ? "border-transparent bg-ink-100" : "border-edge bg-ink-800 hover:border-ink-500"} ${className}`}
>
  <span
    aria-hidden="true"
    class={`absolute left-[3px] h-4 w-4 rounded-full shadow-sm transition-[transform,background-color] duration-200 ease-out ${checked ? "translate-x-5 bg-ink-950" : "translate-x-0 bg-ink-300"}`}
  ></span>
</button>
