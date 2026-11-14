<script lang="ts">
  export type TabItem = { value: string; label: string };

  let {
    items,
    value,
    onChange,
    ariaLabel = "Options",
  }: {
    items: TabItem[];
    value: string;
    onChange: (value: string) => void;
    ariaLabel?: string;
  } = $props();
</script>

<div class="inline-flex max-w-full gap-1 rounded-md bg-ink-800 p-1" role="tablist" aria-label={ariaLabel}>
  {#each items as item (item.value)}
    <button
      type="button"
      role="tab"
      aria-selected={value === item.value}
      onclick={() => onChange(item.value)}
      class={`h-7 rounded px-3 text-sm transition-colors ${value === item.value
        ? "bg-ink-900 text-ink-50 font-semibold shadow-sm"
        : "text-ink-300 hover:bg-veil/5 hover:text-ink-50"}`}
    >
      {item.label}
    </button>
  {/each}
</div>
