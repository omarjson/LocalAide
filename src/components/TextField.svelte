<script lang="ts">
  import Icon, { type IconName } from "./Icon.svelte";

  let {
    value = $bindable(""),
    label,
    placeholder = "",
    type = "text",
    ariaLabel,
    icon,
    disabled = false,
    class: className = "",
  }: {
    value?: string;
    label?: string;
    placeholder?: string;
    type?: "text" | "search" | "password" | "number";
    ariaLabel?: string;
    icon?: IconName;
    disabled?: boolean;
    class?: string;
  } = $props();
</script>

<label class={`block ${className}`}>
  {#if label}<span class="mb-1 block text-sm text-ink-400">{label}</span>{/if}
  <span class="field-shell flex h-10 items-center gap-2 rounded-md border border-ink-700 bg-ink-950 px-3 has-[:disabled]:opacity-50">
    {#if icon}<Icon name={icon} class="h-[18px] w-[18px] shrink-0 text-ink-400" />{/if}
    <input
      bind:value
      {type}
      {placeholder}
      {disabled}
      aria-label={ariaLabel ?? label}
      class="field-bare min-w-0 flex-1 bg-transparent text-sm text-ink-100 placeholder:text-ink-400"
    />
  </span>
</label>
