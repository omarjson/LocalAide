<script lang="ts">
  import { api, errorMessage } from "../lib/api/client";
  import type { CatalogStatus, ModelInfo, ModelPreset } from "../lib/api/bindings";
  import { onMount } from "svelte";
  import ErrorBanner from "../components/ErrorBanner.svelte";

  type Model = {
    id: string;
    name: string;
    size: string;
    ram: string;
    ramGb: number;
    quant: string;
    license: string;
    status: "installed" | "available" | "installing";
    progress?: number;
    active?: boolean;
    kind?: string;
  };

  let models = $state<Model[]>([]);
  let loading = $state(true);
  let query = $state("");
  let presets = $state<ModelPreset[]>([]);
  let activePreset = $state<string | null>(null);
  let presetInstalling = $state(false);
  let verifying = $state<string | null>(null);
  let verifyStatus = $state<Record<string, boolean>>({});
  let actionError = $state<string | null>(null);
  // From the system probe: models above this estimated RAM get a warning.
  let budgetGb = $state(0);
  // Catalog provenance: built-in (compiled in) or refreshed (fetched from
  // the project repo). Null until the first status call lands.
  let catalog = $state<CatalogStatus | null>(null);
  let catalogBusy = $state(false);

  const verifyModel = async (id: string) => {
    verifying = id;
    actionError = null;
    try {
      const res = await api.verifyModel(id);
      verifyStatus = { ...verifyStatus, [id]: res.verified };
      if (!res.verified) {
        actionError = `Checksum verification failed for ${id}: ${res.error ?? "mismatched hash"}`;
      }
    } catch (e) {
      actionError = errorMessage(e);
    } finally {
      verifying = null;
    }
  };

  const refresh = async () => {
    try {
      const list = await api.listModels();
      models = list.map((m: ModelInfo) => ({
        id: m.id,
        name: m.name,
        size: m.size_bytes ? `${(m.size_bytes / 1024 / 1024).toFixed(0)} MB` : "?",
        ram: `${m.ram_estimate_gb} GB`,
        ramGb: m.ram_estimate_gb,
        status: m.installed ? "installed" : "available",
        quant: m.quant,
        license: m.license,
        active: m.active ?? false,
        kind: m.kind
      }));
    } catch {
      models = [];
    } finally {
      loading = false;
    }
  };

  // Fetch the latest manifest from the project repository and swap the
  // active catalog. Fails closed: the backend keeps the old list on any
  // validation problem.
  const refreshCatalogData = async () => {
    catalogBusy = true;
    actionError = null;
    try {
      catalog = await api.refreshCatalog();
      await refresh();
    } catch (e) {
      actionError = errorMessage(e);
    } finally {
      catalogBusy = false;
    }
  };

  // Delete the cached catalog and restore the list compiled into the app.
  const resetCatalogData = async () => {
    catalogBusy = true;
    actionError = null;
    try {
      catalog = await api.resetCatalog();
      await refresh();
    } catch (e) {
      actionError = errorMessage(e);
    } finally {
      catalogBusy = false;
    }
  };

  const formatDate = (unixSeconds: number) =>
    new Date(unixSeconds * 1000).toLocaleDateString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric"
    });

  onMount(() => {
    let active = true;
    void (async () => {
      await refresh();
      if (!active) return;
      try {
        const probe = await api.systemProbe();
        budgetGb = probe.budget_gb;
      } catch {}
      try {
        presets = await api.getModelPresets();
      } catch {}
      try {
        catalog = await api.getCatalogStatus();
      } catch {}
    })();

    return () => {
      active = false;
    };
  });

  const startInstall = async (id: string) => {
    actionError = null;
    models = models.map(m => m.id === id ? { ...m, status: "installing", progress: 0 } : m);
    try {
      await api.downloadModel(id, (evt) => {
        models = models.map(m => m.id === id
          ? { ...m, status: "installing", progress: Math.round(evt.percent) }
          : m);
      });
      await refresh();
    } catch (e) {
      models = models.map(m => m.id === id
        ? { ...m, status: "available", progress: undefined }
        : m);
      actionError = errorMessage(e);
    }
  };

  const cancelInstall = async (id: string) => {
    try { await api.cancelModelDownload(id); } catch {}
    models = models.map(m => m.id === id ? { ...m, status: "available", progress: undefined } : m);
  };

  const installPreset = async (id: string) => {
    actionError = null;
    activePreset = id;
    presetInstalling = true;
    try {
      await api.downloadModelPreset(id);
      await refresh();
    } catch (e) {
      if (!/cancel/i.test(errorMessage(e))) {
        actionError = errorMessage(e);
      }
    }
    presetInstalling = false;
    activePreset = null;
  };

  const cancelPresetInstall = async (id: string) => {
    try {
      await api.cancelPresetDownload(id);
    } catch {}
  };

  const activateModel = async (id: string) => {
    actionError = null;
    try {
      await api.activateModel(id);
      models = models.map(m => ({ ...m, active: m.id === id }));
    } catch (e) {
      actionError = errorMessage(e);
    }
  };

  const unloadModel = async () => {
    actionError = null;
    try {
      await api.unloadInferenceModel();
    } catch (e) {
      actionError = errorMessage(e);
    }
  };

  const removeModel = async (id: string) => {
    actionError = null;
    try {
      await api.removeModel(id);
      await refresh();
    } catch (e) {
      actionError = errorMessage(e);
    }
  };

  const filtered = $derived(models.filter(m => m.name.toLowerCase().includes(query.toLowerCase())));

  const kindLabel = (k?: string) => {
    switch (k) {
      case "chat": return "Chat";
      case "code": return "Code";
      case "embedding": return "Embedding";
      case "reranker": return "Reranker";
      default: return k?.toUpperCase() ?? "MODEL";
    }
  };
</script>

<div class="max-w-5xl mx-auto space-y-8">
  {#if actionError}
    <ErrorBanner message={actionError} onDismiss={() => actionError = null} />
  {/if}
  <div class="flex items-center justify-between">
    <div>
      <h2 class="text-lg font-semibold tracking-tight">Models</h2>
      <p class="text-xs text-ink-400">Catalog of local GGUF models. Download once, run 100% offline.</p>
    </div>
    <div class="flex items-center gap-3">
      <div class="relative">
        <input
          type="text"
          placeholder="Filter models…"
          bind:value={query}
          class="bg-ink-900 border border-ink-800 rounded-lg px-3 py-1.5 text-xs text-ink-200 placeholder-ink-500 focus:outline-none focus:border-ink-700 w-56 font-mono"
        />
      </div>
    </div>
  </div>

  {#if catalog}
    <div class="border border-ink-800 rounded-2xl px-4 py-3 bg-ink-900/40 flex items-center justify-between gap-4">
      <div class="min-w-0">
        <div class="text-sm font-medium text-ink-200 flex items-center gap-2">
          Catalog v{catalog.version}
          <span
            class="text-xs font-mono px-1.5 py-0.5 rounded border {catalog.source === 'refreshed'
              ? 'border-emerald-500/30 bg-emerald-500/10 text-emerald-300'
              : 'border-ink-700 text-ink-400'}"
            title={catalog.source === 'refreshed' ? 'Fetched from the project repository' : 'Compiled into this build'}
          >
            {catalog.source}
          </span>
        </div>
        <div class="text-xs text-ink-400 font-mono mt-0.5">
          {catalog.model_count} models · checked {formatDate(catalog.updated_at)}{#if catalog.fetched_at} · refreshed {formatDate(catalog.fetched_at)}{/if}
        </div>
      </div>
      <div class="flex items-center gap-2 shrink-0">
        {#if catalog.source === "refreshed"}
          <button
            onclick={resetCatalogData}
            disabled={catalogBusy}
            class="px-3 py-1.5 text-xs font-medium rounded-lg border border-ink-700 hover:bg-ink-800 disabled:opacity-40 text-ink-300 transition-colors"
            title="Delete the cached catalog and restore the list built into this app"
          >
            Reset
          </button>
        {/if}
        <button
          onclick={refreshCatalogData}
          disabled={catalogBusy}
          class="px-3 py-1.5 text-xs font-medium rounded-lg bg-ink-800 hover:bg-ink-700 disabled:opacity-40 text-ink-200 transition-colors"
          title="Fetch the newest catalog from the project repository over HTTPS"
        >
          {catalogBusy ? "Working…" : "Refresh catalog"}
        </button>
      </div>
    </div>
  {/if}

  {#if presets.length > 0}
    <div class="border border-ink-800 rounded-2xl p-4 bg-ink-900/40">
      <div class="text-sm font-medium text-ink-200 mb-3">Hardware presets</div>
      <div class="grid grid-cols-1 md:grid-cols-4 gap-3">
        {#each presets as p}
          <div class="border border-ink-800 rounded-xl p-3 bg-ink-950/60 flex flex-col justify-between">
            <div>
              <div class="font-medium text-sm text-ink-100">{p.name}</div>
              <div class="text-xs text-ink-400 mt-0.5">{p.description}</div>
              <div class="text-[11px] font-mono text-ember-400 mt-2">~{p.ram_gb} GB RAM</div>
            </div>
            <button
              onclick={() => installPreset(p.id)}
              disabled={presetInstalling}
              class="mt-3 w-full py-1.5 px-3 text-xs font-medium rounded-lg bg-ink-800 hover:bg-ink-700 disabled:opacity-40 text-ink-200 transition-colors"
            >
              {activePreset === p.id ? "Installing…" : "Install preset"}
            </button>
            {#if activePreset === p.id}
              <button
                onclick={() => cancelPresetInstall(p.id)}
                class="mt-2 w-full py-1.5 px-3 text-xs font-medium rounded-lg border border-rose-500/30 text-rose-400 hover:bg-rose-500/10 transition-colors"
              >
                Cancel
              </button>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  {/if}

  {#if loading}
    <div class="py-12 text-center text-xs text-ink-400 font-mono">Loading model catalog…</div>
  {:else}
    <div class="grid grid-cols-1 gap-3">
      {#each filtered as m}
        <div class="border border-ink-800 rounded-xl p-4 bg-ink-900/40 flex items-center justify-between hover:border-ink-700 transition-colors">
          <div class="flex items-start gap-3">
            <span class="text-xs uppercase font-mono px-2 py-0.5 rounded bg-ink-800 text-ink-400 mt-0.5">
              {kindLabel(m.kind)}
            </span>
            <div>
              <div class="flex items-center gap-2">
                <span class="font-medium text-sm text-ink-100">{m.name}</span>
                <span class="text-xs font-mono px-1.5 rounded border border-ink-700 text-ink-300" title="Model license">{m.license}</span>
                {#if m.active}
                  <span class="text-xs font-mono px-1.5 py-0.5 rounded bg-emerald-500/20 text-emerald-300 border border-emerald-500/30">Active</span>
                {/if}
                {#if budgetGb > 0 && m.ramGb > budgetGb}
                  <span
                    class="text-xs font-mono px-1.5 py-0.5 rounded bg-amber-500/15 text-amber-300 border border-amber-500/30"
                    title="Estimated RAM exceeds your computed budget ({budgetGb.toFixed(1)} GB)"
                  >
                    Over budget
                  </span>
                {/if}
              </div>
              <div class="flex items-center gap-4 text-xs text-ink-400 font-mono mt-1">
                <span>Size: {m.size}</span>
                <span>RAM: {m.ram}</span>
                <span>Quant: {m.quant}</span>
              </div>
              {#if m.status === "installing"}
                <div class="mt-2 w-64">
                  <div class="flex justify-between text-xs font-mono text-ink-400 mb-1">
                    <span>Downloading…</span>
                    <span>{m.progress ?? 0}%</span>
                  </div>
                  <div class="h-1.5 bg-ink-800 rounded-full overflow-hidden">
                    <div class="h-full bg-ember-500 transition-all duration-300" style="width: {m.progress ?? 0}%"></div>
                  </div>
                </div>
              {/if}
            </div>
          </div>

          <div class="flex items-center gap-2">
            {#if m.status === "installed"}
              {#if verifyStatus[m.id]}
                <span class="text-xs font-mono px-2 py-1 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
                  ✓ Verified
                </span>
              {:else}
                <button
                  onclick={() => verifyModel(m.id)}
                  disabled={verifying === m.id}
                  class="px-2.5 py-1.5 text-xs font-medium rounded-lg border border-ink-700 hover:bg-ink-800 text-ink-300 transition-colors"
                  title="Verify cryptographic SHA-256 integrity"
                >
                  {verifying === m.id ? "Checking…" : "Verify"}
                </button>
              {/if}
              {#if !m.active}
                <button
                  onclick={() => activateModel(m.id)}
                  class="px-3 py-1.5 text-xs font-medium rounded-lg bg-ink-800 hover:bg-ink-700 text-ink-200 transition-colors"
                >
                  Activate
                </button>
              {:else}
                <button
                  onclick={unloadModel}
                  class="px-3 py-1.5 text-xs font-medium rounded-lg border border-ink-700 hover:bg-ink-800 text-ink-300 transition-colors"
                  title="Stop the local engine and free the model's RAM; the model stays installed"
                >
                  Unload
                </button>
              {/if}
              <button
                onclick={() => removeModel(m.id)}
                class="px-3 py-1.5 text-xs font-medium rounded-lg text-rose-400 hover:bg-rose-500/10 border border-rose-500/20 transition-colors"
              >
                Delete
              </button>
            {:else if m.status === "installing"}
              <button
                onclick={() => cancelInstall(m.id)}
                class="px-3 py-1.5 text-xs font-medium rounded-lg text-amber-400 hover:bg-amber-500/10 border border-amber-500/20 transition-colors"
              >
                Cancel
              </button>
            {:else}
              <button
                onclick={() => startInstall(m.id)}
                class="px-3 py-1.5 text-xs font-medium rounded-lg bg-ember-600 hover:bg-ember-500 text-ink-950 transition-colors"
              >
                Download
              </button>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
