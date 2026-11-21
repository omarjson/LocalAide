<script lang="ts">
  // The chat's model menu: the provider at the top (tap it to switch),
  // that provider's models with a search, and the thinking effort as a
  // slider for the active model (Ctrl+T steps through it).
  import { tick, untrack } from "svelte";
  import { fly } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import { dur, motion } from "../lib/motion";
  import { contextLabel, effortLabel } from "../lib/chatParts";
  import { formatGb } from "../lib/format";
  import { providerKey, providerList, providerModels } from "../lib/modelPicker";
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
    /** The active model's thinking effort, and the levels it has. */
    effort?: string;
    onEffort?: (effort: string) => void;
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
    effort = "auto",
    onEffort,
  }: Props = $props();

  const active = $derived(choices?.models.find((m) => m.active) ?? null);
  const providers = $derived(providerList(choices));
  // The provider whose models are listed; starts at the active model's.
  let viewing = $state<string | null>(null);
  let view = $state<"models" | "providers">("models");
  let query = $state("");
  let cursor = $state(0);
  let searchEl = $state<HTMLInputElement>();
  let rootEl = $state<HTMLDivElement>();
  let listEl = $state<HTMLDivElement>();

  const shown = $derived(viewing ?? (active ? providerKey(active) : providers[0]?.key ?? "local"));
  const provider = $derived(providers.find((p) => p.key === shown) ?? null);
  const list = $derived(providerModels(choices, shown, query));
  const elsewhere = $derived(query.trim() && list.length === 0 ? (choices?.models ?? []).filter((m) => providerKey(m) !== shown && `${m.name} ${m.model}`.toLowerCase().includes(query.trim().toLowerCase())).slice(0, 8) : []);
  const rows = $derived([...list, ...elsewhere]);

  const levels = $derived(active && active.efforts.length > 0 ? ["auto", ...active.efforts.filter((e) => e !== "auto")] : []);
  const level = $derived(Math.max(0, levels.indexOf(effort)));

  $effect(() => {
    if (open) {
      untrack(() => {
        view = "models";
        viewing = null;
        cursor = Math.max(0, rows.findIndex((m) => m.active));
      });
      void tick().then(() => searchEl?.focus());
    } else {
      query = "";
    }
  });
  $effect(() => {
    if (rows.length === 0) cursor = 0;
    else if (cursor >= rows.length) cursor = rows.length - 1;
  });

  const pick = (m: ModelChoice) => {
    open = false;
    onSelect(m);
  };

  const onKeydown = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      if (view === "providers") {
        view = "models";
        return;
      }
      open = false;
      rootEl?.querySelector<HTMLElement>("[aria-haspopup]")?.focus();
    } else if (view === "models" && (e.key === "ArrowDown" || e.key === "ArrowUp")) {
      e.preventDefault();
      const n = rows.length;
      if (n) cursor = (cursor + (e.key === "ArrowDown" ? 1 : n - 1)) % n;
      void tick().then(() => listEl?.querySelector<HTMLElement>(`[data-index="${cursor}"]`)?.scrollIntoView({ block: "nearest" }));
    } else if (view === "models" && e.key === "Enter" && !e.isComposing) {
      e.preventDefault();
      const hit = rows[cursor];
      if (hit) pick(hit);
    }
  };

  const onDocumentPointer = (e: PointerEvent) => {
    if (open && e.target instanceof Node && rootEl && !rootEl.contains(e.target)) open = false;
  };

  const noModels = $derived(!!choices && choices.models.length === 0);
  const chipLabel = $derived(busy ? "Loading…" : active ? active.name : noModels ? "Add a model" : "Choose a model");
  const fill = $derived(levels.length > 1 ? (level / (levels.length - 1)) * 100 : 0);
</script>

<svelte:document onpointerdown={onDocumentPointer} />

<div class="relative" bind:this={rootEl}>
  <button type="button" aria-haspopup="dialog" aria-expanded={open} onclick={() => (open = !open)} disabled={!choices} class="model-chip" title="Choose the model (Ctrl+Shift+M)">
    {#if busy}
      <span class="h-3 w-3 animate-spin rounded-full border-2 border-ink-700 border-t-ember-500" aria-hidden="true"></span>
    {:else if active && !active.local}
      <Icon name="cloud" class="h-3.5 w-3.5 text-ink-400" />
    {:else}
      <span class={`h-1.5 w-1.5 shrink-0 rounded-full ${active && choices?.local_loaded ? "bg-emerald-400" : "bg-ink-600"}`} aria-hidden="true"></span>
    {/if}
    <span class="max-w-[12rem] truncate font-medium text-ink-100">{chipLabel}</span>
    {#if active && levels.length > 0 && effort !== "auto" && !busy}<span class="text-ink-400">{effortLabel(effort)}</span>{/if}
    <Icon name="chevron-down" class={`h-3 w-3 shrink-0 text-ink-500 transition-transform duration-200 ${open ? "rotate-180" : ""}`} />
  </button>

  {#if open && choices}
    <div role="dialog" aria-label="Choose a model" tabindex="-1" onkeydown={onKeydown} class="picker" in:fly={{ y: 6, duration: dur(motion.base), easing: motion.enter }}>
      {#if view === "providers"}
        <div class="p-1.5" role="listbox" aria-label="Providers">
          {#each providers as p (p.key)}
            <button type="button" role="option" aria-selected={p.key === shown} class="prov-row" onclick={() => { viewing = p.key; view = "models"; query = ""; void tick().then(() => searchEl?.focus()); }}>
              <span class="prov-icon"><Icon name={p.key === "local" ? "computer" : p.router ? "plug" : "cloud"} class="h-3.5 w-3.5" /></span>
              <span class="min-w-0 flex-1 truncate text-sm text-ink-100">{p.label}</span>
              {#if p.badge}<span class="badge">{p.badge}</span>{/if}
              <span class="w-4">{#if p.key === shown}<Icon name="check" class="h-4 w-4 text-ink-200" />{/if}</span>
            </button>
          {/each}
        </div>
        <div class="border-t border-ink-800 p-1.5">
          {#if onManageProviders}
            <button type="button" class="prov-row" onclick={() => { open = false; onManageProviders?.(); }}>
              <span class="prov-icon"><Icon name="plus" class="h-3.5 w-3.5" /></span>
              <span class="flex-1 text-sm text-ink-100">{choices.online_allowed ? "New provider" : "Online models are off"}</span>
              <span class="badge">API</span>
            </button>
          {/if}
          {#if onGetModels}
            <button type="button" class="prov-row" onclick={() => { open = false; onGetModels?.(); }}>
              <span class="prov-icon"><Icon name="models" class="h-3.5 w-3.5" /></span>
              <span class="flex-1 text-sm text-ink-100">Manage models</span>
            </button>
          {/if}
        </div>
      {:else}
        <button type="button" class="prov-head" onclick={() => (view = "providers")} aria-label={`Provider: ${provider?.label ?? "This computer"}. Change provider`}>
          <span class="prov-icon"><Icon name={shown === "local" ? "computer" : provider?.router ? "plug" : "cloud"} class="h-3.5 w-3.5" /></span>
          <span class="min-w-0 flex-1 truncate text-left text-sm font-semibold text-ink-50">{provider?.label ?? "This computer"}</span>
          {#if provider?.badge}<span class="badge">{provider.badge}</span>{/if}
          <Icon name="chevron-right" class="h-4 w-4 text-ink-500" />
        </button>

        <div class="flex items-center gap-2 px-3 pb-1 pt-2.5">
          <span class="text-xs font-semibold text-ink-500">Models</span>
          <span class="flex min-w-0 flex-1 items-center gap-1.5">
            <Icon name="search" class="h-3.5 w-3.5 shrink-0 text-ink-500" />
            <input bind:this={searchEl} bind:value={query} type="search" role="combobox" aria-expanded="true" aria-controls="model-list" aria-activedescendant={rows[cursor] ? `model-opt-${cursor}` : undefined} aria-label="Search models" placeholder="Search models…" class="field-bare min-w-0 flex-1 bg-transparent text-sm text-ink-100 placeholder:text-ink-500" />
          </span>
          {#if provider?.providerId}
            <button type="button" onclick={() => provider?.providerId && onRefresh(provider.providerId)} disabled={refreshing === provider.providerId} class="refresh" title="Ask the provider for its models again"><Icon name="refresh" class={`h-3 w-3 ${refreshing === provider.providerId ? "animate-spin" : ""}`} />{provider.needsRefresh ? "Load" : ""}</button>
          {/if}
        </div>

        <div id="model-list" role="listbox" aria-label="Models" class="max-h-[min(20rem,45vh)] overflow-y-auto px-1.5 pb-1.5" bind:this={listEl}>
          {#if noModels}
            <p class="px-3 py-5 text-center text-sm text-ink-400">No models yet. Download one to run on this computer, or add an online provider.</p>
          {:else if rows.length === 0}
            <p class="px-3 py-5 text-center text-sm text-ink-400">{query.trim() ? `No model matches “${query.trim()}”.` : provider?.needsRefresh ? "Press Load to see this provider's models." : "No models here yet."}</p>
          {/if}
          {#each rows as m, index (m.key)}
            {#if index === list.length && elsewhere.length > 0}<div class="px-2.5 pb-1 pt-2 text-xs font-semibold text-ink-500">Other providers</div>{/if}
            <div id={`model-opt-${index}`} data-index={index} role="option" aria-selected={m.active} tabindex="-1" onclick={() => pick(m)} onkeydown={(e) => { if (e.key === " ") { e.preventDefault(); pick(m); } }} onmousemove={() => (cursor = index)} class={`row ${index === cursor ? "row-cursor" : ""}`}>
              <span class="min-w-0 flex-1">
                <span class={`block truncate text-sm ${m.active ? "font-semibold text-ink-50" : "text-ink-100"}`}>{m.name}</span>
                {#if index >= list.length || m.size_bytes || m.context}
                  <span class="block truncate text-xs text-ink-500">{index >= list.length ? `${m.provider_label} · ` : ""}{m.size_bytes ? formatGb(m.size_bytes) : ""}{m.size_bytes && m.context ? " · " : ""}{m.context ? `${contextLabel(m.context)} context` : ""}</span>
                {/if}
              </span>
              <span class="flex shrink-0 items-center gap-1">
                {#if m.vision}<span class="badge">Vision</span>{/if}
                {#if m.reasoning}<span class="badge">Thinks</span>{/if}
                {#if m.free && !m.local}<span class="badge badge-free">Free</span>{/if}
                <button type="button" onclick={(e) => { e.stopPropagation(); onToggleFavorite(m.key); }} aria-pressed={m.favorite} aria-label={m.favorite ? `Unstar ${m.name}` : `Star ${m.name}`} class={`star ${m.favorite ? "star-on" : ""}`}><Icon name={m.favorite ? "star-filled" : "star"} class="h-3.5 w-3.5" /></button>
                <span class="grid w-4 place-items-center">{#if m.active}<Icon name="check" class="h-4 w-4 text-ink-100" />{/if}</span>
              </span>
            </div>
          {/each}
        </div>

        {#if levels.length > 1 && onEffort}
          <div class="border-t border-ink-800 px-3 pb-3 pt-2.5">
            <div class="flex items-center gap-2 text-sm">
              <span class="text-ink-400">Effort</span>
              <span class="font-semibold text-ink-50">{effortLabel(levels[level])}</span>
              <kbd class="ml-auto rounded bg-ink-800 px-1.5 py-0.5 text-[11px] text-ink-400">Ctrl+T</kbd>
            </div>
            <div class="effort mt-2" style={`--fill:${fill}%`}>
              <input type="range" min="0" max={levels.length - 1} step="1" value={level} aria-label="Thinking effort" aria-valuetext={effortLabel(levels[level])} oninput={(e) => onEffort?.(levels[Number((e.currentTarget as HTMLInputElement).value)])} />
              <span class="effort-track" aria-hidden="true"><span class="effort-fill"></span>{#each levels as _, i (i)}<span class="effort-dot" style={`left:${levels.length > 1 ? (i / (levels.length - 1)) * 100 : 0}%`}></span>{/each}</span>
            </div>
          </div>
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .model-chip {
    display: inline-flex;
    height: 2rem;
    align-items: center;
    gap: 0.4rem;
    border-radius: 0.6rem;
    padding: 0 0.55rem;
    font-size: 0.8125rem;
    color: var(--color-ink-300);
    transition: background-color 150ms ease-out;
  }
  .model-chip:hover:not(:disabled),
  .model-chip[aria-expanded="true"] {
    background: color-mix(in oklab, var(--color-veil) 6%, transparent);
  }
  .model-chip:disabled {
    opacity: 0.5;
  }
  .picker {
    position: absolute;
    right: 0;
    bottom: calc(100% + 0.5rem);
    z-index: 40;
    width: min(21rem, calc(100vw - 2rem));
    overflow: hidden;
    border-radius: 0.85rem;
    border: 1px solid var(--color-edge);
    background: var(--color-ink-950);
    box-shadow: 0 24px 48px -20px rgb(0 0 0 / 0.6);
    transform-origin: bottom right;
  }
  .prov-head {
    display: flex;
    width: 100%;
    align-items: center;
    gap: 0.6rem;
    border-bottom: 1px solid var(--color-ink-800);
    padding: 0.6rem 0.75rem;
    transition: background-color 120ms ease-out;
  }
  .prov-head:hover {
    background: color-mix(in oklab, var(--color-veil) 5%, transparent);
  }
  .prov-row {
    display: flex;
    width: 100%;
    align-items: center;
    gap: 0.6rem;
    border-radius: 0.5rem;
    padding: 0.45rem 0.55rem;
    text-align: left;
    transition: background-color 120ms ease-out;
  }
  .prov-row:hover,
  .prov-row:focus-visible {
    background: color-mix(in oklab, var(--color-veil) 6%, transparent);
  }
  .prov-icon {
    display: grid;
    height: 1.5rem;
    width: 1.5rem;
    flex-shrink: 0;
    place-items: center;
    border-radius: 0.4rem;
    background: var(--color-ink-800);
    color: var(--color-ink-300);
  }
  .badge {
    border-radius: 0.3rem;
    background: var(--color-ink-800);
    padding: 0.05rem 0.35rem;
    font-size: 0.6875rem;
    font-weight: 500;
    color: var(--color-ink-300);
  }
  .badge-free {
    background: color-mix(in oklab, #10b981 16%, transparent);
    color: #6ee7b7;
  }
  .refresh {
    display: inline-flex;
    align-items: center;
    gap: 0.25rem;
    border-radius: 0.35rem;
    padding: 0.15rem 0.35rem;
    font-size: 0.75rem;
    color: var(--color-ink-500);
  }
  .refresh:hover {
    color: var(--color-ink-100);
  }
  .row {
    display: flex;
    cursor: pointer;
    align-items: center;
    gap: 0.6rem;
    border-radius: 0.5rem;
    padding: 0.45rem 0.55rem;
  }
  .row-cursor {
    background: color-mix(in oklab, var(--color-veil) 6%, transparent);
  }
  .star {
    display: grid;
    height: 1.4rem;
    width: 1.4rem;
    place-items: center;
    border-radius: 0.35rem;
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
  .effort {
    position: relative;
    height: 1.6rem;
  }
  .effort input {
    position: absolute;
    inset: 0;
    z-index: 2;
    width: 100%;
    margin: 0;
    opacity: 0;
    cursor: pointer;
  }
  .effort-track {
    position: absolute;
    left: 0.6rem;
    right: 0.6rem;
    top: 50%;
    height: 0.9rem;
    transform: translateY(-50%);
    border-radius: 9999px;
    background: var(--color-ink-800);
  }
  .effort-fill {
    position: absolute;
    inset: 0 auto 0 -0.6rem;
    width: calc(var(--fill) + 0.6rem);
    min-width: 1.2rem;
    border-radius: 9999px;
    background: linear-gradient(90deg, #22c55e, #0ea5e9 140%);
    transition: width 160ms cubic-bezier(0.23, 1, 0.32, 1);
  }
  .effort-fill::after {
    content: "";
    position: absolute;
    right: -0.45rem;
    top: 50%;
    height: 1.25rem;
    width: 1.25rem;
    transform: translateY(-50%);
    border-radius: 9999px;
    background: #fff;
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.4);
  }
  .effort-dot {
    position: absolute;
    top: 50%;
    height: 0.25rem;
    width: 0.25rem;
    transform: translate(-50%, -50%);
    border-radius: 9999px;
    background: rgb(255 255 255 / 0.55);
    pointer-events: none;
  }
  .effort input:focus-visible ~ .effort-track {
    outline: 2px solid var(--color-ink-50);
    outline-offset: 3px;
  }
  @media (prefers-reduced-motion: reduce) {
    .effort-fill {
      transition: none;
    }
  }
</style>
