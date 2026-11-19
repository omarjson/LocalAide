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

  // The selected pill slides between tabs: one indicator, sized and moved to
  // the active button.
  let buttons = $state<HTMLButtonElement[]>([]);
  let indicator = $state<{ left: number; width: number } | null>(null);

  $effect(() => {
    const index = items.findIndex((item) => item.value === value);
    const el = buttons[index];
    if (!el) {
      indicator = null;
      return;
    }
    const measure = () => {
      indicator = { left: el.offsetLeft, width: el.offsetWidth };
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(el);
    return () => observer.disconnect();
  });

  const onKeydown = (event: KeyboardEvent, index: number) => {
    if (event.key !== "ArrowRight" && event.key !== "ArrowLeft") return;
    event.preventDefault();
    const step = event.key === "ArrowRight" ? 1 : -1;
    const next = (index + step + items.length) % items.length;
    onChange(items[next].value);
    buttons[next]?.focus();
  };
</script>

<div class="relative inline-flex max-w-full gap-1 rounded-md bg-ink-800 p-1" role="tablist" aria-label={ariaLabel}>
  {#if indicator}
    <span
      aria-hidden="true"
      class="tab-indicator pointer-events-none absolute top-1 bottom-1 rounded bg-ink-900 shadow-sm"
      style={`left:${indicator.left}px;width:${indicator.width}px`}
    ></span>
  {/if}
  {#each items as item, i (item.value)}
    <button
      bind:this={buttons[i]}
      type="button"
      role="tab"
      aria-selected={value === item.value}
      tabindex={value === item.value ? 0 : -1}
      onclick={() => onChange(item.value)}
      onkeydown={(event) => onKeydown(event, i)}
      class={`relative h-7 rounded px-3 text-sm transition-colors duration-150 ${value === item.value
        ? "text-ink-50 font-semibold"
        : "text-ink-300 hover:text-ink-50"}`}
    >
      {item.label}
    </button>
  {/each}
</div>

<style>
  .tab-indicator {
    transition:
      left 220ms cubic-bezier(0.22, 1, 0.36, 1),
      width 220ms cubic-bezier(0.22, 1, 0.36, 1);
  }
</style>
