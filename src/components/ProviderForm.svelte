<script lang="ts">
  // Add or edit an online provider in one flow: start from a preset (or
  // not), name, address, key, Load models, pick one (or type it), Save.
  // The key is stored the way the app always stores it: in its local
  // settings file, readable only by you.
  import { onMount, tick, untrack } from "svelte";
  import { slide } from "svelte/transition";
  import Button from "./Button.svelte";
  import ErrorBanner from "./ErrorBanner.svelte";
  import Icon from "./Icon.svelte";
  import Switch from "./Switch.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import { openInBrowser } from "../lib/links";
  import { dur } from "../lib/motion";
  import type { CloudConfigStatus, CloudProviderDto, CloudProviderPreset, ProviderModel } from "../lib/api/bindings";

  interface Props {
    /** The provider being edited; null adds a new one. */
    editing: CloudProviderDto | null;
    onSaved: (status: CloudConfigStatus) => void;
    onCancel: () => void;
  }
  let { editing, onSaved, onCancel }: Props = $props();

  let presets = $state<CloudProviderPreset[]>([]);
  let presetId = $state<string | null>(null);
  // The form starts from the provider being edited; Settings remounts it
  // for another provider.
  const initial = untrack(() => editing);
  let label = $state(initial?.label ?? "");
  let url = $state(initial?.base_url ?? "");
  let key = $state("");
  let model = $state(initial?.model ?? "");
  let models = $state<ProviderModel[]>([]);
  let loaded = $state(false);
  let loading = $state(false);
  let listError = $state<string | null>(null);
  let query = $state("");
  let freeOnly = $state(false);
  let saving = $state(false);
  let saveError = $state<string | null>(null);
  let nameEl = $state<HTMLInputElement>();

  onMount(async () => {
    try {
      presets = await api.cloudProviderPresets();
    } catch (e: unknown) {
      console.error(e);
    }
    await tick();
    nameEl?.focus();
  });

  const preset = $derived(presets.find((p) => p.id === presetId) ?? null);
  const groups = $derived(
    [
      { title: "No key needed", items: presets.filter((p) => !p.needs_key && !p.local) },
      { title: "On this computer", items: presets.filter((p) => p.local) },
      { title: "Free key", items: presets.filter((p) => p.needs_key && p.free && !p.local) },
      { title: "Paid", items: presets.filter((p) => p.needs_key && !p.free && !p.local) },
    ].filter((g) => g.items.length > 0),
  );
  const shown = $derived(
    models
      .filter((m) => !freeOnly || m.free)
      .filter((m) => {
        const q = query.trim().toLowerCase();
        return !q || m.id.toLowerCase().includes(q) || (m.name ?? "").toLowerCase().includes(q);
      })
      .slice(0, 200),
  );
  const keyOptional = $derived(!!preset && !preset.needs_key);

  const choosePreset = (id: string) => {
    const p = presets.find((x) => x.id === id);
    if (!p) return;
    presetId = id;
    label = p.label;
    url = p.base_url;
    model = p.model;
    key = p.public_key ?? "";
    models = [];
    loaded = false;
    listError = null;
    saveError = null;
  };

  const loadModels = async () => {
    loading = true;
    listError = null;
    try {
      models = await api.cloudListModels(url.trim(), key.trim() || undefined, editing?.id);
      loaded = true;
      freeOnly = models.some((m) => m.free) && !models.every((m) => m.free) && !model;
      if (models.length === 0) listError = "The provider sent an empty list. Type the model name below.";
      else if (!model) model = models[0].id;
    } catch (e: unknown) {
      models = [];
      loaded = false;
      listError = errorMessage(e);
    } finally {
      loading = false;
    }
  };

  const save = async () => {
    saveError = null;
    if (!label.trim() || !url.trim() || !model.trim()) {
      saveError = "Fill in the name, the address and a model.";
      return;
    }
    saving = true;
    try {
      const status = await api.saveCloudProvider({
        id: editing?.id,
        label: label.trim(),
        base_url: url.trim(),
        model: model.trim(),
        api_key: key ? key.trim() : undefined,
      });
      onSaved(status);
    } catch (e: unknown) {
      saveError = errorMessage(e);
    } finally {
      saving = false;
    }
  };
</script>

<section class="form" aria-label={editing ? `Edit ${editing.label}` : "Add a provider"} transition:slide={{ duration: dur(150) }}>
  <div class="flex items-center justify-between gap-3">
    <h3 class="text-[15px] font-semibold text-ink-50">{editing ? `Edit ${editing.label}` : "Add a provider"}</h3>
    <Button variant="subtle" ariaLabel="Close" onclick={onCancel}><Icon name="close" class="h-4 w-4" /></Button>
  </div>

  {#if !editing}
    <details class="mt-3 group" open={presetId === null}>
      <summary class="flex cursor-pointer list-none items-center gap-2 text-sm text-ink-300 hover:text-ink-50">
        <Icon name="chevron-right" class="h-4 w-4 transition-transform group-open:rotate-90" />
        {preset ? `Started from ${preset.label}` : "Start from a known provider"}
      </summary>
      <div class="mt-3 space-y-3">
        {#each groups as g (g.title)}
          <div>
            <p class="mb-1.5 text-xs font-medium text-ink-500">{g.title}</p>
            <div class="flex flex-wrap gap-1.5">
              {#each g.items as p (p.id)}
                <button type="button" aria-pressed={presetId === p.id} onclick={() => choosePreset(p.id)} class={`pick ${presetId === p.id ? "pick-on" : ""}`}>
                  <Icon name={p.local ? "computer" : p.needs_key ? "key" : "sparkle"} class="h-3.5 w-3.5" />{p.label}
                </button>
              {/each}
            </div>
          </div>
        {:else}
          <div class="skeleton h-16"></div>
        {/each}
      </div>
    </details>
    {#if preset?.note}
      <p class="mt-3 text-[13px] leading-5 text-ink-400">
        {preset.note}
        {#if preset.key_url}<a class="ml-1 inline-flex items-center gap-1 text-ember-300 hover:underline" href={preset.key_url} rel="noreferrer" onclick={(e) => openInBrowser(e, preset?.key_url ?? "")}>{preset.needs_key ? "Get a key" : preset.local ? "How to install it" : "More about it"}<Icon name="external" class="h-3.5 w-3.5" /></a>{/if}
      </p>
    {/if}
  {/if}

  <div class="mt-4 grid gap-4">
    <label class="field">
      <span class="field-label">Name</span>
      <input bind:this={nameEl} bind:value={label} placeholder="What to call it in the model menu" class="input" />
    </label>
    <label class="field">
      <span class="field-label">Address</span>
      <input bind:value={url} placeholder="https://api.example.com/v1" spellcheck="false" class="input font-mono text-[13px]" />
      <span class="field-hint">The OpenAI-compatible base address. Messages go here when you pick one of its models.</span>
    </label>
    <label class="field">
      <span class="field-label">Key {editing ? "(leave empty to keep the saved key)" : keyOptional ? "(not needed for this provider)" : ""}</span>
      <input type="password" bind:value={key} placeholder={keyOptional ? "Not needed" : editing?.api_key_hint ? `Saved: ${editing.api_key_hint}` : "Paste your key"} autocomplete="off" spellcheck="false" class="input font-mono text-[13px]" />
      <span class="field-hint">Saved only in this app's settings file on this computer, and sent only to the address above.</span>
    </label>

    <div class="field">
      <div class="flex items-end justify-between gap-3">
        <span class="field-label">Model</span>
        <Button variant="secondary" disabled={loading || !url.trim()} onclick={() => void loadModels()}>
          <Icon name="refresh" class={`h-4 w-4 ${loading ? "animate-spin" : ""}`} />{loading ? "Loading…" : loaded ? "Load again" : "Load models"}
        </Button>
      </div>
      {#if listError}<p class="mt-2 text-[13px] text-ember-200" role="status">{listError}</p>{/if}
      {#if models.length > 0}
        <div class="mt-2 overflow-hidden rounded-[10px] border border-ink-700 bg-ink-950" in:slide={{ duration: dur(150) }}>
          <div class="flex items-center gap-2 border-b border-ink-800 px-2.5 py-2">
            <Icon name="search" class="h-4 w-4 shrink-0 text-ink-500" />
            <input type="search" bind:value={query} aria-label="Filter models" placeholder={`Filter ${models.length} models`} class="field-bare min-w-0 flex-1 bg-transparent text-sm text-ink-100 placeholder:text-ink-500" />
            {#if models.some((m) => m.free)}
              <span class="flex shrink-0 items-center gap-2 text-[13px] text-ink-300"><Switch checked={freeOnly} label="Free models only" onchange={(on) => (freeOnly = on)} />Free only</span>
            {/if}
          </div>
          <div class="max-h-56 overflow-y-auto p-1" role="listbox" aria-label="Models">
            {#each shown as m (m.id)}
              <button type="button" role="option" aria-selected={model === m.id} onclick={() => (model = m.id)} class={`row ${model === m.id ? "row-on" : ""}`}>
                <span class="min-w-0 flex-1 truncate font-mono text-[13px]">{m.id}</span>
                {#if m.context_length}<span class="shrink-0 text-xs tabular-nums text-ink-500">{Math.round(m.context_length / 1000)}K</span>{/if}
                {#if m.free}<span class="shrink-0 rounded bg-emerald-500/15 px-1.5 text-xs font-semibold text-emerald-300">Free</span>{/if}
                {#if model === m.id}<Icon name="check" class="h-4 w-4 shrink-0 text-ember-300" />{/if}
              </button>
            {:else}
              <p class="px-2 py-3 text-sm text-ink-400">No models match.</p>
            {/each}
          </div>
        </div>
      {/if}
      {#if models.length === 0}
        <input bind:value={model} placeholder={loaded ? "Type the model name" : "Load the list, or type the model name"} spellcheck="false" aria-label="Model name" class="input mt-2 font-mono text-[13px]" />
      {/if}
    </div>
  </div>

  {#if saveError}<div class="mt-4"><ErrorBanner message={saveError} onDismiss={() => (saveError = null)} /></div>{/if}
  <div class="mt-5 flex justify-end gap-2">
    <Button variant="subtle" onclick={onCancel}>Cancel</Button>
    <Button variant="primary" disabled={saving} onclick={() => void save()}>{saving ? "Saving…" : editing ? "Save changes" : "Add provider"}</Button>
  </div>
</section>

<style>
  .form {
    border-radius: 12px;
    border: 1px solid var(--color-ink-700);
    background: var(--color-ink-900);
    padding: 16px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .field-label {
    font-size: 13px;
    font-weight: 500;
    color: var(--color-ink-200);
  }
  .field-hint {
    font-size: 12px;
    line-height: 18px;
    color: var(--color-ink-500);
  }
  .input {
    height: 40px;
    width: 100%;
    border-radius: 6px;
    border: 1px solid var(--color-ink-700);
    background: var(--color-ink-950);
    padding: 0 12px;
    font-size: 14px;
    color: var(--color-ink-100);
  }
  .input::placeholder {
    color: var(--color-ink-500);
  }
  .pick {
    display: inline-flex;
    height: 32px;
    align-items: center;
    gap: 6px;
    border-radius: 9999px;
    border: 1px solid var(--color-ink-700);
    background: var(--color-ink-950);
    padding: 0 12px;
    font-size: 13px;
    color: var(--color-ink-200);
    transition: border-color 120ms ease-out, background-color 120ms ease-out;
  }
  .pick:hover {
    border-color: var(--color-edge);
  }
  .pick-on {
    border-color: color-mix(in oklab, var(--color-ember-500) 55%, transparent);
    background: color-mix(in oklab, var(--color-ember-500) 12%, transparent);
    color: var(--color-ink-50);
  }
  .row {
    display: flex;
    width: 100%;
    align-items: center;
    gap: 8px;
    border-radius: 6px;
    padding: 6px 8px;
    text-align: left;
    color: var(--color-ink-200);
  }
  .row:hover {
    background: color-mix(in oklab, var(--color-veil) 5%, transparent);
  }
  .row-on {
    background: color-mix(in oklab, var(--color-ember-500) 10%, transparent);
    color: var(--color-ink-50);
  }
</style>
