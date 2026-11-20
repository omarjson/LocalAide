<script lang="ts">
  import { tick } from "svelte";
  import { fly } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import { dur, motion } from "../lib/motion";
  import { contextLabel } from "../lib/chatParts";
  import { formatGb } from "../lib/format";
  import { NO_FILTERS, parseFilters, pickerSections, type PickerFilters } from "../lib/modelPicker";
  import type { ModelChoice, ModelChoices } from "../lib/api/bindings";

  interface Props {
    choices: ModelChoices | null;
    open?: boolean;
    /** A local model is loading or a choice is being applied. */
    busy?: boolean;
    onSelect: (choice: ModelChoice) => void;
    onToggleFavorite: (key: string) => void;
    onRefresh: (providerId: string) => void;
    refreshing?: string | null;
    onManageProviders?: () => void;
    onGetModels?: () => void;
  }

  let {
    choices,
    open = $bindable(false),
    busy = false,
    onSelect,
    onToggleFavorite,
    onRefresh,
    refreshing = null,
    onManageProviders,
    onGetModels,
  }: Props = $props();

  let query = $state("");
  const savedFilters = (() => {
    try {
      return parseFilters(localStorage.getItem("localaide-model-filters"));
    } catch {
      return { ...NO_FILTERS };
    }
  })();
  let filters = $state<PickerFilters>(savedFilters);
  const activeFilterCount = $derived(Object.values(filters).filter(Boolean).length);
  const clearFilters = () => {
    filters = { ...NO_FILTERS };
  };
  $effect(() => {
    try {
      localStorage.setItem("localaide-model-filters", JSON.stringify(filters));
    } catch {
      // Storage is optional; filters still work for this session.
    }
  });
  let cursor = $state(0);
  let searchEl = $state<HTMLInputElement>();
  let rootEl = $state<HTMLDivElement>();
  let listEl = $state<HTMLDivElement>();

  const active = $derived(choices?.models.find((m) => m.active) ?? null);

  const sections = $derived(pickerSections(choices, query, filters));
  const flat = $derived(sections.flatMap((s) => s.items.map((m) => ({ section: s.id, m }))));

  $effect(() => {
    if (open) {
      cursor = Math.max(0, flat.findIndex((f) => f.m.active));
      void tick().then(() => searchEl?.focus());
    } else {
      query = "";
    }
  });
  // Keep the cursor in range as the list filters.
  $effect(() => {
    if (flat.length === 0) cursor = 0;
    else if (cursor >= flat.length) cursor = flat.length - 1;
  });

  const scrollCursorIntoView = () => {
    void tick().then(() => {
      listEl?.querySelector<HTMLElement>(`[data-index="${cursor}"]`)?.scrollIntoView({ block: "nearest" });
    });
  };

  const pick = (m: ModelChoice) => {
    open = false;
    onSelect(m);
  };

  const onKeydown = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      open = false;
      rootEl?.querySelector<HTMLElement>("[aria-haspopup]")?.focus();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      cursor = flat.length ? (cursor + 1) % flat.length : 0;
      scrollCursorIntoView();
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      cursor = flat.length ? (cursor - 1 + flat.length) % flat.length : 0;
      scrollCursorIntoView();
    } else if (e.key === "Enter" && !e.isComposing) {
      e.preventDefault();
      const hit = flat[cursor];
      if (hit) pick(hit.m);
    }
  };

  const onDocumentPointer = (e: PointerEvent) => {
    if (open && e.target instanceof Node && rootEl && !rootEl.contains(e.target)) open = false;
  };

  const sizeLabel = (bytes: number | null) => (bytes ? formatGb(bytes) : "");
  const filterButtons: { id: keyof typeof filters; label: string; icon: "brain" | "eye" | "wrench" | "heart" }[] = [
    { id: "reasoning", label: "Thinks", icon: "brain" },
    { id: "vision", label: "Sees images", icon: "eye" },
    { id: "tools", label: "Uses tools", icon: "wrench" },
    { id: "free", label: "Free", icon: "heart" },
  ];
  const noModels = $derived(!!choices && choices.models.length === 0);
</script>

<svelte:document onpointerdown={onDocumentPointer} />

<div class="relative" bind:this={rootEl}>
  <button
    type="button"
    aria-haspopup="dialog"
    aria-expanded={open}
    onclick={() => (open = !open)}
    disabled={!choices}
    class="model-chip"
    title="Choose the model (Ctrl+Shift+M)"
  >
    {#if busy}
      <span class="h-3 w-3 animate-spin rounded-full border-2 border-ink-700 border-t-ember-500" aria-hidden="true"></span>
    {:else if active && !active.local}
      <Icon name="cloud" class="h-3.5 w-3.5 text-ember-300" />
    {:else}
      <span class={`h-2 w-2 shrink-0 rounded-full ${active && choices?.local_loaded ? "bg-emerald-400" : "bg-ink-600"}`} aria-hidden="true"></span>
    {/if}
    <span class="max-w-[11rem] truncate">{busy ? "Loading…" : active ? active.name : noModels ? "Add a model" : "Choose a model"}</span>
    <Icon name="chevron-down" class={`h-3 w-3 shrink-0 transition-transform duration-200 ${open ? "rotate-180" : ""}`} />
  </button>

  {#if open && choices}
    <div
      role="dialog"
      aria-label="Choose a model"
      tabindex="-1"
      onkeydown={onKeydown}
      class="picker"
      in:fly={{ y: 6, duration: dur(motion.base), easing: motion.enter }}
    >
      <div class="border-b border-ink-800 p-2.5">
        <span class="field-shell flex h-9 items-center gap-2 rounded-lg border border-ink-700 bg-ink-900 px-2.5">
          <Icon name="search" class="h-4 w-4 shrink-0 text-ink-500" />
          <input
            bind:this={searchEl}
            bind:value={query}
            type="search"
            role="combobox"
            aria-expanded="true"
            aria-controls="model-list"
            aria-activedescendant={flat[cursor] ? `model-opt-${cursor}` : undefined}
            aria-label="Find a model"
            placeholder="Find a model"
            class="field-bare min-w-0 flex-1 bg-transparent text-sm text-ink-100 placeholder:text-ink-500"
          />
        </span>
        <div class="mt-2 flex items-center gap-1.5" role="group" aria-label="Show only models that">
          <div class="flex min-w-0 flex-wrap gap-1.5">
          {#each filterButtons as f (f.id)}
            <button
              type="button"
              aria-pressed={filters[f.id]}
              onclick={() => (filters[f.id] = !filters[f.id])}
              class={`filter ${filters[f.id] ? "filter-on" : ""}`}
            ><Icon name={f.icon} class="h-3.5 w-3.5" />{f.label}</button>
          {/each}
          </div>
          {#if activeFilterCount > 0}
            <button type="button" onclick={clearFilters} class="ml-auto shrink-0 rounded px-1.5 py-1 text-xs text-ink-400 underline underline-offset-2 hover:text-ink-100">Clear ({activeFilterCount})</button>
          {/if}
        </div>
      </div>

      <div id="model-list" role="listbox" aria-label="Models" class="max-h-[min(26rem,55vh)] overflow-y-auto p-1.5" bind:this={listEl}>
        {#if noModels}
          <div class="px-3 py-6 text-center text-sm text-ink-400">
            <p>No models yet. Download one to run on this computer, or turn on an online provider.</p>
          </div>
        {/if}
        {#each sections as section (section.id)}
          <div class="flex items-center justify-between px-2.5 pb-1 pt-2.5">
            <span class="text-xs font-semibold text-ink-500">{section.label}</span>
            {#if section.providerId}
              <button
                type="button"
                onclick={() => section.providerId && onRefresh(section.providerId)}
                disabled={refreshing === section.providerId}
                class="flex items-center gap-1 rounded px-1.5 py-0.5 text-xs text-ink-500 transition-colors hover:bg-veil/5 hover:text-ink-100 disabled:opacity-60"
                title="Ask the provider for its models again"
              ><Icon name="refresh" class={`h-3 w-3 ${refreshing === section.providerId ? "animate-spin" : ""}`} />{section.needsRefresh ? "Load models" : "Refresh"}</button>
            {/if}
          </div>
          {#each section.items as m (section.id + m.key)}
            {@const index = flat.findIndex((f) => f.section === section.id && f.m.key === m.key)}
            <div
              id={`model-opt-${index}`}
              data-index={index}
              role="option"
              aria-selected={m.active}
              tabindex="-1"
              onclick={() => pick(m)}
              onkeydown={(e) => { if (e.key === " ") { e.preventDefault(); pick(m); } }}
              onmousemove={() => (cursor = index)}
              class={`row group ${index === cursor ? "row-cursor" : ""}`}
            >
              <span class="grid h-7 w-7 shrink-0 place-items-center rounded-md bg-ink-800 text-ink-300">
                <Icon name={m.local && m.provider_id === null ? "computer" : m.local ? "plug" : "cloud"} class="h-4 w-4" />
              </span>
              <span class="min-w-0 flex-1">
                <span class={`block truncate text-sm ${m.active ? "font-semibold text-ember-200" : "text-ink-100"}`}>{m.name}</span>
                <span class="block truncate text-xs text-ink-500">
                  {m.provider_label}{m.name !== m.model ? ` · ${m.model}` : ""}{m.size_bytes ? ` · ${sizeLabel(m.size_bytes)}` : ""}
                </span>
              </span>
              <span class="flex shrink-0 items-center gap-1.5 text-ink-400">
                {#if m.free && !m.local}<span class="rounded bg-emerald-500/15 px-1.5 text-xs font-medium text-emerald-300">Free</span>{/if}
                {#if m.reasoning}<span class="cap" title="Thinks before it answers"><Icon name="brain" class="h-3.5 w-3.5" /><span class="sr-only">Thinks</span></span>{/if}
                {#if m.vision}<span class="cap" title="Reads images"><Icon name="eye" class="h-3.5 w-3.5" /><span class="sr-only">Reads images</span></span>{/if}
                {#if m.tools}<span class="cap" title="Uses tools"><Icon name="wrench" class="h-3.5 w-3.5" /><span class="sr-only">Uses tools</span></span>{/if}
                {#if m.context}<span class="w-10 text-right text-xs tabular-nums text-ink-500" title={`Context window: ${m.context.toLocaleString()} tokens`}>{contextLabel(m.context)}</span>{/if}
                <button
                  type="button"
                  onclick={(e) => { e.stopPropagation(); onToggleFavorite(m.key); }}
                  aria-pressed={m.favorite}
                  aria-label={m.favorite ? `Unstar ${m.name}` : `Star ${m.name}`}
                  class={`star ${m.favorite ? "star-on" : ""}`}
                ><Icon name={m.favorite ? "star-filled" : "star"} class="h-3.5 w-3.5" /></button>
                <span class="w-4">{#if m.active}<Icon name="check" class="h-4 w-4 text-ember-300" />{/if}</span>
              </span>
            </div>
          {:else}
            {#if section.providerId}
              <p class="px-3 pb-2 text-xs text-ink-500">{section.needsRefresh ? "Press Load models to see what this provider offers." : section.starred ? "Its starred models are listed above." : "No model matches."}</p>
            {/if}
          {/each}
        {/each}
        {#if !noModels && flat.length === 0 && query.trim()}
          <p class="px-3 py-6 text-center text-sm text-ink-400">No model matches “{query.trim()}”.</p>
        {/if}
      </div>

      <div class="flex items-center gap-1 border-t border-ink-800 p-1.5">
        {#if onGetModels}
          <button type="button" onclick={() => { open = false; onGetModels?.(); }} class="foot"><Icon name="download" class="h-3.5 w-3.5" />Get local models</button>
        {/if}
        {#if onManageProviders}
          <button type="button" onclick={() => { open = false; onManageProviders?.(); }} class="foot" title={choices.online_allowed ? "Manage online providers" : "Online models are switched off in Settings"}><Icon name="cloud" class="h-3.5 w-3.5" />{choices.online_allowed ? "Online providers" : "Online models are off"}</button>
        {/if}
        <span class="ml-auto hidden whitespace-nowrap pr-2 text-xs text-ink-500 md:inline">Arrow keys and Enter</span>
      </div>
    </div>
  {/if}
</div>

<style>
  .model-chip {
    display: inline-flex;
    height: 1.75rem;
    align-items: center;
    gap: 0.45rem;
    border-radius: 9999px;
    border: 1px solid var(--color-ink-700);
    padding: 0 0.55rem;
    font-size: 0.75rem;
    color: var(--color-ink-200);
    transition: border-color 150ms ease-out, color 150ms ease-out;
  }
  .model-chip:hover:not(:disabled),
  .model-chip[aria-expanded="true"] {
    border-color: var(--color-edge);
    color: var(--color-ink-50);
  }
  .model-chip:disabled {
    opacity: 0.5;
  }
  .picker {
    position: absolute;
    right: 0;
    bottom: calc(100% + 0.5rem);
    z-index: 40;
    width: min(30rem, calc(100vw - 2rem));
    overflow: hidden;
    border-radius: 0.75rem;
    border: 1px solid var(--color-edge);
    background: var(--color-ink-950);
    box-shadow: 0 18px 40px -16px rgb(0 0 0 / 0.55);
  }
  .filter {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    border-radius: 9999px;
    border: 1px solid var(--color-ink-700);
    padding: 0.15rem 0.55rem;
    font-size: 0.8125rem;
    color: var(--color-ink-300);
    transition: border-color 150ms ease-out, background-color 150ms ease-out, color 150ms ease-out;
  }
  .filter:hover {
    color: var(--color-ink-50);
  }
  .filter-on {
    border-color: color-mix(in oklab, var(--color-ember-500) 45%, transparent);
    background: color-mix(in oklab, var(--color-ember-500) 12%, transparent);
    color: var(--color-ember-200);
  }
  .row {
    display: flex;
    cursor: pointer;
    align-items: center;
    gap: 0.65rem;
    border-radius: 0.5rem;
    padding: 0.4rem 0.55rem;
  }
  .row-cursor {
    background: color-mix(in oklab, var(--color-veil) 6%, transparent);
  }
  .cap {
    display: inline-grid;
    place-items: center;
  }
  .star {
    display: grid;
    height: 1.5rem;
    width: 1.5rem;
    place-items: center;
    border-radius: 0.4rem;
    color: var(--color-ink-500);
    opacity: 0;
    transition: opacity 120ms ease-out, color 120ms ease-out;
  }
  .row:hover .star,
  .row-cursor .star,
  .star:focus-visible,
  .star-on {
    opacity: 1;
  }
  .star:hover,
  .star-on {
    color: var(--color-ember-300);
  }
  .foot {
    display: inline-flex;
    white-space: nowrap;
    align-items: center;
    gap: 0.4rem;
    border-radius: 0.5rem;
    padding: 0.35rem 0.6rem;
    font-size: 0.8125rem;
    color: var(--color-ink-400);
    transition: background-color 150ms ease-out, color 150ms ease-out;
  }
  .foot:hover {
    background: color-mix(in oklab, var(--color-veil) 6%, transparent);
    color: var(--color-ink-100);
  }
</style>
