<script lang="ts">
  import { onMount } from "svelte";
  import Button from "../components/Button.svelte";
  import ConfirmDialog from "../components/ConfirmDialog.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import ErrorBanner from "../components/ErrorBanner.svelte";
  import Icon from "../components/Icon.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import SettingsGroup from "../components/SettingsGroup.svelte";
  import StatusBadge from "../components/StatusBadge.svelte";
  import Tabs from "../components/Tabs.svelte";
  import { showToast } from "../lib/toast.svelte";
  import { updateVoice, voice } from "../lib/voice.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import { popover } from "../lib/actions/popover";
  import type { CatalogStatus, HfFileInfo, HfRepo, HfSort, ModelInfo, ModelPreset } from "../lib/api/bindings";
  import { fly, slide } from "svelte/transition";
  import { dur, stagger } from "../lib/motion";
  import { openLegal } from "../lib/legal.svelte";
  import { formatBytes, formatGb } from "../lib/format";

  type Model = {
    id: string;
    name: string;
    size: string;
    ram: string;
    ramGb: number;
    quant: string;
    license: string;
    /** "pausing": Pause was pressed and the download is still stopping. */
    status: "installed" | "available" | "installing" | "pausing" | "paused";
    progress?: number;
    downloadedBytes?: number;
    totalBytes?: number;
    speedBps?: number;
    active?: boolean;
    kind?: string;
    vision?: boolean;
    source?: string;
    verified?: boolean;
  };

  let models = $state<Model[]>([]);
  // Downloads this screen started and that have not ended yet.
  const downloadingIds = new Set<string>();
  let loading = $state(true);
  let query = $state("");
  let kindFilter = $state("all");
  let presets = $state<ModelPreset[]>([]);
  let activePreset = $state<string | null>(null);
  let presetInstalling = $state(false);
  let verifying = $state<string | null>(null);
  let verifyStatus = $state<Record<string, boolean>>({});
  let searchModelBusy = $state<string | null>(null);
  let actionError = $state<string | null>(null);
  let loadingModelId = $state<string | null>(null);
  let budgetGb = $state(0);
  let catalog = $state<CatalogStatus | null>(null);
  let catalogBusy = $state(false);
  let hfQuery = $state("");
  let hfBusy = $state(false);
  let hfRepos = $state<HfRepo[]>([]);
  let hfExpanded = $state<string | null>(null);
  let hfFiles = $state<Record<string, HfFileInfo[]>>({});
  let hfFilesBusy = $state<string | null>(null);
  let hfKinds = $state<Record<string, string>>({});
  let hfAdding = $state<string | null>(null);
  let showAllModels = $state(false);
  let showMoreOptions = $state(false);
  let showCommunity = $state(false);
  let hfSort = $state<HfSort>("trending");
  let hfUnfiltered = $state(false);
  let hfSearched = $state("");
  const hfSorts: { value: HfSort; label: string; icon: "flame" | "download" | "heart" | "clock" }[] = [
    { value: "trending", label: "Trending", icon: "flame" },
    { value: "downloads", label: "Most downloaded", icon: "download" },
    { value: "likes", label: "Most liked", icon: "heart" },
    { value: "newest", label: "Newest", icon: "clock" },
  ];
  let openModelMenu = $state<string | null>(null);
  let pendingRemoveId = $state<string | null>(null);

  /** "1.2 GB left · about 3 min", from a smoothed speed. */
  const formatEta = (downloaded: number, total: number, speed: number) => {
    const left = Math.max(0, total - downloaded);
    if (total <= 0) return "";
    const size = `${formatBytes(left)} left`;
    if (!speed || left === 0) return size;
    const seconds = Math.ceil(left / speed);
    const time = seconds < 60 ? `${seconds} s` : seconds < 3600 ? `${Math.round(seconds / 60)} min` : `${Math.floor(seconds / 3600)} h ${Math.round((seconds % 3600) / 60)} min`;
    return `${size} · about ${time}`;
  };
  const kindTabs = [
    { value: "all", label: "All" },
    { value: "chat", label: "Chat" },
    { value: "code", label: "Code" },
    { value: "embedding", label: "Meaning" },
    { value: "reranker", label: "Ranking" },
    { value: "stt", label: "Voice" },
  ];

  const refresh = async () => {
    try {
      const list = await api.listModels();
      models = list.map((m: ModelInfo) => ({
        id: m.id,
        name: m.name,
        // One unit (GB) beside the memory estimate; the total includes companion files.
        size: (m.total_bytes || m.size_bytes) ? formatGb(m.total_bytes || m.size_bytes) : "Unknown",
        ram: `${m.ram_estimate_gb} GB`,
        ramGb: m.ram_estimate_gb,
        // A partial file left from a pause (or a closed app) can resume.
        status: m.installed
          ? "installed"
          : downloadingIds.has(m.id)
            ? "installing"
            : (m.partial_bytes ?? 0) > 0
              ? "paused"
              : "available",
        downloadedBytes: m.installed ? undefined : m.partial_bytes,
        totalBytes: m.total_bytes,
        progress: !m.installed && m.total_bytes ? Math.round(((m.partial_bytes ?? 0) / m.total_bytes) * 100) : undefined,
        quant: m.quant,
        license: m.license,
        active: m.active ?? false,
        kind: m.kind,
        vision: m.vision ?? false,
        source: m.source,
        verified: m.verified,
      }));
    } catch (e: unknown) {
      models = [];
      actionError = errorMessage(e);
    } finally {
      loading = false;
    }
  };

  const verifyModel = async (id: string) => {
    verifying = id;
    actionError = null;
    try {
      const res = await api.verifyModel(id);
      verifyStatus = { ...verifyStatus, [id]: res.verified };
      if (!res.verified) {
        actionError = `File check failed for ${id}: ${res.error ?? "The file hash does not match."}`;
      }
    } catch (e: unknown) {
      actionError = errorMessage(e);
    } finally {
      verifying = null;
    }
  };

  let searchModelId = $state<string | null>(null);
  void api.libraryStatus().then((s) => (searchModelId = s.embedding_ready ? s.embedding_model : null)).catch(() => {});
  const useForSearch = async (id: string) => {
    searchModelBusy = id;
    actionError = null;
    try {
      // Remembered across restarts; the library gets vectors from this model.
      const status = await api.libraryEmbeddingSelect(id);
      searchModelId = status.embedding_model;
      showToast("Search now finds related wording with this model.");
    } catch (e: unknown) {
      actionError = errorMessage(e);
    } finally {
      searchModelBusy = null;
    }
  };

  const refreshCatalogData = async () => {
    catalogBusy = true;
    actionError = null;
    try {
      catalog = await api.refreshCatalog();
      await refresh();
    } catch (e: unknown) {
      actionError = errorMessage(e);
    } finally {
      catalogBusy = false;
    }
  };

  const resetCatalogData = async () => {
    catalogBusy = true;
    actionError = null;
    try {
      catalog = await api.resetCatalog();
      await refresh();
    } catch (e: unknown) {
      actionError = errorMessage(e);
    } finally {
      catalogBusy = false;
    }
  };

  // An empty search lists the most popular GGUF models in the chosen order.
  const hfSearchRepos = async () => {
    hfBusy = true;
    actionError = null;
    hfExpanded = null;
    try {
      if (hfUnfiltered) {
        // Makers tag these two ways; ask for both and merge.
        const [a, b] = await Promise.all([
          api.hfSearch(hfQuery.trim(), 20, hfSort, "uncensored"),
          api.hfSearch(hfQuery.trim(), 20, hfSort, "abliterated"),
        ]);
        const seen = new Set<string>();
        hfRepos = [...a, ...b].filter((repo) => !seen.has(repo.id) && seen.add(repo.id));
      } else {
        hfRepos = await api.hfSearch(hfQuery.trim(), 30, hfSort);
      }
      hfSearched = hfQuery.trim();
      if (hfRepos.length === 0) actionError = "No public model folders matched that search.";
    } catch (e: unknown) {
      actionError = errorMessage(e);
    } finally {
      hfBusy = false;
    }
  };

  const hfToggleRepo = async (repo: string) => {
    if (hfExpanded === repo) {
      hfExpanded = null;
      return;
    }
    hfExpanded = repo;
    if (hfFiles[repo]) return;
    hfFilesBusy = repo;
    actionError = null;
    try {
      const files = await api.hfRepoFiles(repo);
      hfFiles = { ...hfFiles, [repo]: files };
    } catch (e: unknown) {
      actionError = errorMessage(e);
      hfExpanded = null;
    } finally {
      hfFilesBusy = null;
    }
  };

  const applyProgress = (id: string, event: { percent: number; downloaded_bytes: number; total_bytes: number; speed_bps: number; status: string }) => {
    models = models.map((model) => model.id === id
      ? {
          ...model,
          status:
            event.status === "paused"
              ? "paused"
              : event.status === "complete"
                ? "installed"
                : model.status === "pausing"
                  ? "pausing"
                  : "installing",
          progress: Math.round(event.percent),
          downloadedBytes: event.downloaded_bytes || model.downloadedBytes,
          totalBytes: event.total_bytes || model.totalBytes,
          // Smooth the 200 ms samples, so the time left does not jump.
          speedBps: event.speed_bps > 0 ? (model.speedBps ? model.speedBps * 0.8 + event.speed_bps * 0.2 : event.speed_bps) : model.speedBps,
        }
      : model);
  };
  const hfAddAndDownload = async (repo: string, file: HfFileInfo) => {
    hfAdding = file.path;
    actionError = null;
    try {
      const license = hfRepos.find((item) => item.id === repo)?.license ?? null;
      const withMmproj = (hfFiles[repo] ?? []).some((item) => item.is_mmproj && item.path !== file.path);
      const kind = hfKinds[repo] ?? "chat";
      const res = await api.hfCustomAdd(repo, file.path, kind, license, withMmproj);
      await refresh();
      models = models.map((model) => model.id === res.id
        ? { ...model, status: "installing", progress: 0 }
        : model);
      await runDownload(res.id);
    } catch (e: unknown) {
      if (!/cancel/i.test(errorMessage(e))) actionError = errorMessage(e);
      await refresh();
    } finally {
      hfAdding = null;
    }
  };

  const pauseInstall = async (id: string) => {
    try {
      const stopping = await api.pauseModelDownload(id);
      // Stay on "Pausing…" until the download really ends; resuming before
      // that would race the old download for the same partial file.
      models = models.map((model) => model.id === id ? { ...model, status: stopping ? "pausing" : "paused", speedBps: 0 } : model);
    } catch (e: unknown) {
      actionError = errorMessage(e);
    }
  };

  /** Run one download to its end: finished, paused, or failed. */
  const runDownload = async (id: string) => {
    downloadingIds.add(id);
    try {
      await api.downloadModel(id, (event) => applyProgress(id, event));
    } finally {
      downloadingIds.delete(id);
    }
    // A pause ends the call normally; keep the row on Resume.
    const row = models.find((model) => model.id === id);
    if (row?.status === "pausing") models = models.map((m) => m.id === id ? { ...m, status: "paused" } : m);
    await refresh();
  };

  const startInstall = async (id: string) => {
    actionError = null;
    models = models.map((model) => model.id === id ? { ...model, status: "installing" } : model);
    try {
      await runDownload(id);
    } catch (e: unknown) {
      // Cancel is the user's own choice, not an error to show.
      if (!/cancel/i.test(errorMessage(e))) actionError = errorMessage(e);
      await refresh();
    }
  };

  const cancelInstall = async (id: string) => {
    try {
      await api.cancelModelDownload(id);
    } catch (e: unknown) {
      actionError = errorMessage(e);
    }
    // Cancel discards the partial file; the row goes back to Download.
    models = models.map((model) => model.id === id ? { ...model, status: "available", progress: undefined, downloadedBytes: undefined, speedBps: 0 } : model);
    if (!downloadingIds.has(id)) await refresh();
  };

  const installPreset = async (id: string) => {
    actionError = null;
    activePreset = id;
    presetInstalling = true;
    try {
      await api.downloadModelPreset(id);
      await refresh();
    } catch (e: unknown) {
      if (!/cancel/i.test(errorMessage(e))) actionError = errorMessage(e);
    }
    presetInstalling = false;
    activePreset = null;
  };

  const cancelPresetInstall = async (id: string) => {
    try {
      await api.cancelPresetDownload(id);
    } catch (e: unknown) {
      actionError = errorMessage(e);
    }
  };

  const activateModel = async (id: string) => {
    actionError = null;
    loadingModelId = id;
    try {
      // The backend loads the model into the engine and marks it active.
      await api.activateModel(id);
      await refresh();
    } catch (e: unknown) {
      actionError = errorMessage(e);
      await refresh();
    } finally {
      loadingModelId = null;
    }
  };

  const unloadModel = async () => {
    actionError = null;
    try {
      await api.unloadInferenceModel();
      models = models.map((model) => ({ ...model, active: false }));
    } catch (e: unknown) {
      actionError = errorMessage(e);
    }
  };

  const removeModel = async (id: string) => {
    actionError = null;
    try {
      await api.removeModel(id);
      await refresh();
    } catch (e: unknown) {
      actionError = errorMessage(e);
    }
  };

  const askRemoveModel = (id: string) => {
    openModelMenu = null;
    pendingRemoveId = id;
  };

  const confirmRemoveModel = async () => {
    const id = pendingRemoveId;
    pendingRemoveId = null;
    if (id) await removeModel(id);
  };

  const filtered = $derived(
    models.filter((model) =>
      model.name.toLowerCase().includes(query.toLowerCase()) &&
      (kindFilter === "all" || model.kind === kindFilter)
    )
  );

  const activeModel = $derived(models.find((model) => model.active && model.status === "installed") ?? null);
  const recommended = $derived(
    filtered.filter((model) => model.kind === "chat" && !model.active && model.status !== "installing").slice(0, 3)
  );
  const remainingModels = $derived(
    filtered.filter((model) => !recommended.some((item) => item.id === model.id))
  );
  const helperModels = $derived(filtered.filter((model) => model.kind === "embedding" || model.kind === "reranker" || model.kind === "stt"));
  const catalogCount = $derived(catalog?.model_count ?? models.length);

  const kindLabel = (kind?: string) => {
    switch (kind) {
      case "chat": return "Chat";
      case "code": return "Code";
      case "embedding": return "Meaning";
      case "reranker": return "Ranking";
      case "stt": return "Voice";
      default: return kind?.toUpperCase() ?? "MODEL";
    }
  };

  const modelDescription = (model: Model) => {
    if (model.kind === "embedding") return `Helps Search find related ideas. ${model.size} download.`;
    if (model.kind === "stt") return `Turns speech into text for the microphone button. ${model.size} download.`;
    if (model.kind === "reranker") return `Helps Search sort results. ${model.size} download.`;
    if (model.kind === "code") return `For code and technical questions. ${model.size} download.`;
    return `${model.vision ? "Can read images. " : "For everyday questions. "}${model.size} download.`;
  };

  const formatDate = (unixSeconds: number) => new Date(unixSeconds * 1000).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });

  const revealAllModels = () => {
    showAllModels = true;
    showMoreOptions = true;
    setTimeout(() => document.getElementById("all-models")?.scrollIntoView({ behavior: "smooth", block: "start" }), 0);
  };

  onMount(() => {
    let active = true;
    void (async () => {
      await refresh();
      if (!active) return;
      try {
        const probe = await api.systemProbe();
        budgetGb = probe.budget_gb;
      } catch (e: unknown) {
        console.error(e);
      }
      try {
        presets = await api.getModelPresets();
      } catch (e: unknown) {
        console.error(e);
      }
      try {
        catalog = await api.getCatalogStatus();
      } catch (e: unknown) {
        console.error(e);
      }
    })();

    return () => {
      active = false;
    };
  });
</script>

<div class="mx-auto max-w-5xl pb-10">
  <PageHeader
    title="Models"
    description="The AI that runs on this computer. Download one, and it works without the internet."
  >
    {#snippet actions()}
      <Button variant="secondary" onclick={revealAllModels}>
        <Icon name="search" class="h-[18px] w-[18px]" />
        Find more models
      </Button>
    {/snippet}
  </PageHeader>

  {#if actionError}
    <div class="mb-4">
      <ErrorBanner message={actionError} onRetry={() => void refresh()} onDismiss={() => actionError = null} />
    </div>
  {/if}

  {#if activeModel}
    <div class="rounded-lg border border-ink-700 bg-ink-900">
      <div class="flex flex-col gap-4 p-5 sm:flex-row sm:items-center">
        <div class="grid h-12 w-12 shrink-0 place-items-center rounded-lg bg-ink-50">
          <Icon name="models" class="h-6 w-6 text-ember-500" />
        </div>
        <div class="min-w-0 flex-1">
          <div class="flex flex-wrap items-center gap-2">
            <h2 class="text-lg font-semibold text-ink-50">{activeModel.name}</h2>
            <StatusBadge label="In use" tone="ok" dot />
          </div>
          <div class="mt-1 flex flex-wrap gap-x-4 gap-y-1 text-sm text-ink-400">
            <span>Size <b class="font-semibold text-ink-200">{activeModel.size}</b></span>
            <span>Needs <b class="font-semibold text-ink-200">{activeModel.ram}</b> memory</span>
            {#if activeModel.vision}<span>Can read <b class="font-semibold text-ink-200">images</b></span>{/if}
          </div>
        </div>
        <div class="flex items-center gap-2">
          <Button variant="secondary" onclick={unloadModel}>Stop using</Button>
          <div class="relative" use:popover={{ open: openModelMenu === activeModel.id, onClose: () => openModelMenu = null }}>
            <Button
              variant="subtle"
              class="h-8 w-8 px-0"
              ariaLabel="More options for active model"
              ariaExpanded={openModelMenu === activeModel.id}
              ariaHaspopup="menu"
              onclick={() => openModelMenu = openModelMenu === activeModel.id ? null : activeModel.id}
            >
              <Icon name="more" class="h-[18px] w-[18px]" />
            </Button>
            {#if openModelMenu === activeModel.id}
              <div role="menu" aria-label="Options for the active model" class="absolute right-0 top-9 z-10 min-w-44 rounded-md border border-edge bg-ink-900 p-1 shadow-xl">
                <button type="button" role="menuitem" onclick={() => void verifyModel(activeModel.id)} class="flex w-full items-center gap-2 rounded px-3 py-2 text-left text-sm text-ink-200 hover:bg-veil/5 focus:bg-veil/5 focus:outline-none">
                  <Icon name="check" class="h-4 w-4" />
                  {verifying === activeModel.id ? "Checking…" : "Check file"}
                </button>
                <button type="button" role="menuitem" onclick={() => askRemoveModel(activeModel.id)} class="flex w-full items-center gap-2 rounded px-3 py-2 text-left text-sm text-rose-400 hover:bg-rose-500/10 focus:bg-rose-500/10 focus:outline-none">
                  <Icon name="trash" class="h-4 w-4" />
                  Remove model
                </button>
              </div>
            {/if}
          </div>
        </div>
      </div>
    </div>
  {:else}
    <EmptyState icon="models" title="No model is in use" description="Choose a model below to start chatting.">
      {#snippet action()}
        <Button variant="primary" onclick={revealAllModels}>Browse models</Button>
      {/snippet}
    </EmptyState>
  {/if}

  {#if !loading && recommended.length > 0}
    <h2 class="mb-2 mt-7 text-[15px] font-semibold text-ink-50">Recommended for this computer</h2>
    <SettingsGroup>
      {#each recommended as model, index (model.id)}
        <div class={`flex flex-col gap-4 p-4 sm:flex-row sm:items-center sm:px-[18px] ${index > 0 ? 'border-t border-ink-800' : ''}`}>
          <div class="grid h-9 w-9 shrink-0 place-items-center rounded-md bg-ink-800 text-ink-200"><Icon name="models" class="h-[18px] w-[18px]" /></div>
          <div class="min-w-0 flex-1">
            <div class="font-semibold text-ink-50">{model.name}</div>
            <div class="text-sm text-ink-400">{modelDescription(model)}</div>
            {#if model.status === "installing" || model.status === "pausing" || model.status === "paused"}
              <div class="mt-2 max-w-sm">
                <div class="mb-1 flex flex-wrap justify-between gap-x-3 text-sm text-ink-400"><span>{model.status === "paused" ? "Paused" : model.status === "pausing" ? "Pausing…" : "Downloading…"}</span><span>{model.progress ?? 0}% · {formatBytes(model.downloadedBytes ?? 0)} / {formatBytes(model.totalBytes ?? 0)}</span></div>
                <div class="h-1.5 overflow-hidden rounded-full bg-ink-800"><div class="h-full bg-ember-500 transition-all" style={`width:${model.progress ?? 0}%`}></div></div>
                <div class="mt-1 flex flex-wrap justify-between gap-x-3 text-xs text-ink-500"><span>{model.status === "installing" && model.speedBps ? `${formatBytes(model.speedBps)}/s` : model.status === "installing" ? "Starting…" : "Resume continues where it stopped"}</span><span>{formatEta(model.downloadedBytes ?? 0, model.totalBytes ?? 0, model.status === "installing" ? (model.speedBps ?? 0) : 0)}</span></div>
              </div>
            {/if}
          </div>
          <div class="flex shrink-0 items-center gap-2">
            {#if budgetGb > 0 && model.ramGb > budgetGb}
              <StatusBadge label="May be too big" tone="warn" />
            {:else}
              <StatusBadge label="Fits" tone="ok" />
            {/if}
            {#if model.status === "installing"}
              <Button variant="secondary" onclick={() => void pauseInstall(model.id)}>Pause</Button>
              <Button variant="danger" onclick={() => void cancelInstall(model.id)}>Cancel</Button>
            {:else if model.status === "pausing"}
              <Button variant="secondary" disabled>Pausing…</Button>
            {:else if model.status === "paused"}
              <Button variant="primary" onclick={() => void startInstall(model.id)}>Resume</Button>
              <Button variant="danger" onclick={() => void cancelInstall(model.id)}>Cancel</Button>
            {:else if model.status === "installed"}
              <Button variant="secondary" disabled={loadingModelId !== null} onclick={() => void activateModel(model.id)}>{loadingModelId === model.id ? "Loading…" : "Use this"}</Button>
            {:else}
              <Button variant="secondary" onclick={() => void startInstall(model.id)}>
                <Icon name="download" class="h-4 w-4" />
                Download
              </Button>
            {/if}
          </div>
        </div>
      {/each}
    </SettingsGroup>
  {/if}

  {#if !showAllModels}
    <button type="button" onclick={revealAllModels} class="mt-5 flex items-center gap-2 px-1 py-2 text-sm text-ink-300 hover:text-ink-50">
      <Icon name="chevron-right" class="h-4 w-4" />
      Browse all {catalogCount} models, including helper models and community models
    </button>
  {/if}

  {#if showAllModels}
    <section id="all-models" class="mt-6 scroll-mt-6">
      <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h2 class="text-[15px] font-semibold text-ink-50">All models</h2>
          <p class="mt-1 text-sm text-ink-400">Search the catalog or filter by what a model does.</p>
        </div>
        <input
          type="search"
          placeholder="Find a model"
          aria-label="Find a model"
          bind:value={query}
          class="h-10 w-full rounded-md border border-ink-700 bg-ink-900 px-3 text-sm text-ink-100 outline-none placeholder:text-ink-400 focus:border-edge sm:w-56"
        />
      </div>
      <div class="mt-3 overflow-x-auto">
        <Tabs items={kindTabs} value={kindFilter} onChange={(value) => kindFilter = value} ariaLabel="Model type" />
      </div>

      {#if loading}
        <div class="mt-3 rounded-lg border border-ink-700 bg-ink-900 px-4 py-12 text-center text-sm text-ink-400">Loading the model list…</div>
      {:else if remainingModels.length === 0}
        <div class="mt-3 rounded-lg border border-ink-700 bg-ink-900 px-4 py-12 text-center text-sm text-ink-400">No models match that search.</div>
      {:else}
        <SettingsGroup>
          <div class="mt-3">
            {#each remainingModels as model, index (model.id)}
              <div class={`flex flex-col gap-4 p-4 sm:flex-row sm:items-center sm:px-[18px] ${index > 0 ? 'border-t border-ink-800' : ''} ${model.ramGb > budgetGb && budgetGb > 0 ? 'opacity-75' : ''}`}>
                <div class="grid h-9 w-9 shrink-0 place-items-center rounded-md bg-ink-800 text-ink-200"><Icon name="models" class="h-[18px] w-[18px]" /></div>
                <div class="min-w-0 flex-1">
                  <div class="flex flex-wrap items-center gap-2">
                    <span class="font-semibold text-ink-50">{model.name}</span>
                    <span class="rounded border border-ink-700 px-1.5 py-0.5 text-xs text-ink-300">{kindLabel(model.kind)}</span>
                    {#if model.active}<StatusBadge label="In use" tone="ok" dot />{/if}
                    {#if model.source === "community"}<span class="rounded border border-sky-500/30 bg-sky-500/10 px-1.5 py-0.5 text-xs text-sky-300">Community</span>{/if}
                    {#if model.vision}<span class="rounded border border-violet-500/30 bg-violet-500/10 px-1.5 py-0.5 text-xs text-violet-300">Images</span>{/if}
                  </div>
                  <div class="mt-1 flex flex-wrap gap-x-4 gap-y-1 text-sm text-ink-400">
                    <span>{modelDescription(model)}</span>
                    <span>Needs {model.ram}</span>
                    <span>{model.quant}</span>
                  </div>
                  {#if model.status === "installing" || model.status === "pausing" || model.status === "paused"}
                    <div class="mt-2 max-w-sm">
                      <div class="mb-1 flex flex-wrap justify-between gap-x-3 text-sm text-ink-400"><span>{model.status === "paused" ? "Paused" : model.status === "pausing" ? "Pausing…" : "Downloading…"}</span><span>{model.progress ?? 0}% · {formatBytes(model.downloadedBytes ?? 0)} / {formatBytes(model.totalBytes ?? 0)}</span></div>
                      <div class="h-1.5 overflow-hidden rounded-full bg-ink-800"><div class="h-full bg-ember-500 transition-all" style={`width:${model.progress ?? 0}%`}></div></div>
                      <div class="mt-1 flex flex-wrap justify-between gap-x-3 text-xs text-ink-500"><span>{model.status === "installing" && model.speedBps ? `${formatBytes(model.speedBps)}/s` : model.status === "installing" ? "Starting…" : "Resume continues where it stopped"}</span><span>{formatEta(model.downloadedBytes ?? 0, model.totalBytes ?? 0, model.status === "installing" ? (model.speedBps ?? 0) : 0)}</span></div>
                    </div>
                  {/if}
                </div>
                <div class="relative flex shrink-0 items-center gap-2" use:popover={{ open: openModelMenu === model.id, onClose: () => openModelMenu = null }}>
                  {#if model.ramGb > budgetGb && budgetGb > 0}
                    <StatusBadge label="May be too big" tone="warn" />
                  {/if}
                  {#if model.status === "installed"}
                    {#if verifyStatus[model.id] || model.verified}
                      <StatusBadge label="Checked" tone="ok" />
                    {/if}
                    {#if model.kind === "stt"}
                      {#if voice.settings?.model === model.id}
                        <StatusBadge label="Used for voice" tone="ok" dot />
                      {:else}
                        <Button variant="secondary" onclick={() => void updateVoice({ model: model.id }).then(() => showToast("The microphone now uses this model.")).catch((e) => (actionError = errorMessage(e)))}>Use for voice</Button>
                      {/if}
                    {:else if model.active}
                      <Button variant="secondary" onclick={unloadModel}>Stop using</Button>
                    {:else}
                      <Button variant="secondary" disabled={loadingModelId !== null} onclick={() => void activateModel(model.id)}>{loadingModelId === model.id ? "Loading…" : "Use this"}</Button>
                    {/if}
                    {#if model.kind === "embedding"}
                      {#if searchModelId === model.id}
                        <StatusBadge label="Used for Search" tone="ok" dot />
                      {:else}
                        <Button variant="subtle" disabled={searchModelBusy !== null} onclick={() => void useForSearch(model.id)}>
                          {searchModelBusy === model.id ? "Starting…" : "Use for Search"}
                        </Button>
                      {/if}
                    {/if}
                  {:else if model.status === "installing"}
                    <Button variant="secondary" onclick={() => void pauseInstall(model.id)}>Pause</Button>
                    <Button variant="danger" onclick={() => void cancelInstall(model.id)}>Cancel</Button>
                  {:else if model.status === "pausing"}
              <Button variant="secondary" disabled>Pausing…</Button>
            {:else if model.status === "paused"}
                    <Button variant="primary" onclick={() => void startInstall(model.id)}>Resume</Button>
                    <Button variant="danger" onclick={() => void cancelInstall(model.id)}>Cancel</Button>
                  {:else}
                    <Button variant="secondary" onclick={() => void startInstall(model.id)}>
                      <Icon name="download" class="h-4 w-4" />
                      Download
                    </Button>
                  {/if}
                  {#if model.status === "installed"}
                    <!-- Only installed models have file options. -->
                    <Button
                      variant="subtle"
                      class="h-8 w-8 px-0"
                      ariaLabel={`More options for ${model.name}`}
                      ariaExpanded={openModelMenu === model.id}
                      ariaHaspopup="menu"
                      onclick={() => openModelMenu = openModelMenu === model.id ? null : model.id}
                    >
                      <Icon name="more" class="h-[18px] w-[18px]" />
                    </Button>
                  {/if}
                  {#if openModelMenu === model.id && model.status === "installed"}
                    <div role="menu" aria-label={`Options for ${model.name}`} class="absolute right-0 top-9 z-10 min-w-44 rounded-md border border-edge bg-ink-900 p-1 shadow-xl">
                      {#if !verifyStatus[model.id]}
                        <button type="button" role="menuitem" onclick={() => { openModelMenu = null; void verifyModel(model.id); }} class="flex w-full items-center gap-2 rounded px-3 py-2 text-left text-sm text-ink-200 hover:bg-veil/5 focus:bg-veil/5 focus:outline-none">
                          <Icon name="check" class="h-4 w-4" />
                          {verifying === model.id ? "Checking…" : "Check file"}
                        </button>
                      {/if}
                      <button type="button" role="menuitem" onclick={() => askRemoveModel(model.id)} class="flex w-full items-center gap-2 rounded px-3 py-2 text-left text-sm text-rose-400 hover:bg-rose-500/10 focus:bg-rose-500/10 focus:outline-none">
                        <Icon name="trash" class="h-4 w-4" />
                        Remove model
                      </button>
                    </div>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        </SettingsGroup>
      {/if}
    </section>

    <button type="button" onclick={() => showMoreOptions = !showMoreOptions} class="mt-5 flex items-center gap-2 px-1 py-2 text-sm text-ink-300 hover:text-ink-50">
      <Icon name="chevron-right" class={`h-4 w-4 transition-transform ${showMoreOptions ? 'rotate-90' : ''}`} />
      More model choices
    </button>

    {#if showMoreOptions}
      {#if helperModels.length > 0}
        <h2 class="mb-2 mt-5 text-[15px] font-semibold text-ink-50">Helper models</h2>
        <SettingsGroup>
          <div class="flex flex-col gap-4 p-4 sm:flex-row sm:items-center sm:px-[18px]">
            <div class="grid h-9 w-9 shrink-0 place-items-center rounded-md bg-ink-800 text-ink-200"><Icon name="search" class="h-[18px] w-[18px]" /></div>
            <div class="min-w-0 flex-1">
              <div class="font-semibold text-ink-50">Smarter document search</div>
              <div class="text-sm text-ink-400">Small extra models that help Search understand meaning, not just exact words.</div>
            </div>
            <Button variant="subtle" onclick={() => kindFilter = "embedding"}>Show {helperModels.length} models</Button>
          </div>
        </SettingsGroup>
      {/if}

      {#if presets.length > 0}
        <h2 class="mb-2 mt-5 text-[15px] font-semibold text-ink-50">Choose by computer size</h2>
        <SettingsGroup>
          {#each presets as preset, index (preset.id)}
            <div class={`flex flex-col gap-3 p-4 sm:flex-row sm:items-center sm:px-[18px] ${index > 0 ? 'border-t border-ink-800' : ''}`}>
              <div class="min-w-0 flex-1">
                <div class="font-semibold text-ink-50">{preset.name}</div>
                <div class="text-sm text-ink-400">{preset.description} About {preset.ram_gb} GB of memory.</div>
              </div>
              <Button variant="secondary" disabled={presetInstalling} onclick={() => void installPreset(preset.id)}>
                {activePreset === preset.id ? "Installing…" : "Install this set"}
              </Button>
              {#if activePreset === preset.id}
                <Button variant="danger" onclick={() => void cancelPresetInstall(preset.id)}>Cancel</Button>
              {/if}
            </div>
          {/each}
        </SettingsGroup>
      {/if}

      {#if catalog}
        <div class="mt-5 rounded-lg border border-ink-700 bg-ink-900 p-4">
          <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div>
              <div class="font-semibold text-ink-50">Model list</div>
              <div class="mt-1 text-sm text-ink-400">Version {catalog.version} · checked {formatDate(catalog.updated_at)} · {catalog.source === "refreshed" ? "updated from the project" : "built into this app"}</div>
            </div>
            <div class="flex items-center gap-2">
              {#if catalog.source === "refreshed"}
                <Button variant="subtle" disabled={catalogBusy} onclick={() => void resetCatalogData()}>Use built-in list</Button>
              {/if}
              <Button variant="secondary" disabled={catalogBusy} onclick={() => void refreshCatalogData()}>
                <Icon name="refresh" class="h-4 w-4" />
                {catalogBusy ? "Updating…" : "Update list"}
              </Button>
            </div>
          </div>
        </div>
      {/if}

      <div class="mt-5 rounded-lg border border-ink-700 bg-ink-900 p-4">
        <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
          <div>
            <h2 class="font-semibold text-ink-50">Community models</h2>
            <p class="mt-1 text-sm text-ink-400">Search public model folders. Check the license before you use one for work.</p>
          </div>
          <Button variant="subtle" onclick={() => { showCommunity = !showCommunity; if (showCommunity && hfRepos.length === 0) void hfSearchRepos(); }}>{showCommunity ? "Hide" : "Browse"}</Button>
        </div>
        {#if showCommunity}
          <div class="mt-4" transition:slide={{ duration: dur(200) }}>
            <div class="flex gap-2">
              <span class="field-shell flex h-10 min-w-0 flex-1 items-center gap-2 rounded-md border border-ink-700 bg-ink-950 px-3">
                <Icon name="search" class="h-4 w-4 shrink-0 text-ink-400" />
                <input
                  type="search"
                  bind:value={hfQuery}
                  onkeydown={(event) => { if (event.key === "Enter") void hfSearchRepos(); }}
                  placeholder="Search by name, or leave empty to browse"
                  aria-label="Search community models"
                  class="field-bare min-w-0 flex-1 bg-transparent text-sm text-ink-100 placeholder:text-ink-400"
                />
              </span>
              <Button variant="primary" class="h-10" disabled={hfBusy} onclick={() => void hfSearchRepos()}>{hfBusy ? "Searching…" : "Search"}</Button>
            </div>
            <div class="mt-3 flex flex-wrap items-center gap-2">
              {#each hfSorts as option (option.value)}
                <button
                  type="button"
                  aria-pressed={hfSort === option.value}
                  onclick={() => { hfSort = option.value; void hfSearchRepos(); }}
                  class={`inline-flex h-8 items-center gap-1.5 rounded-full border px-3 text-sm transition-colors duration-150 ${hfSort === option.value ? "border-ember-500/50 bg-ember-500/10 text-ember-200" : "border-ink-700 text-ink-300 hover:border-edge hover:text-ink-50"}`}
                >
                  <Icon name={option.icon} class="h-4 w-4" />{option.label}
                </button>
              {/each}
              <span class="mx-1 hidden h-5 w-px bg-ink-700 sm:block" aria-hidden="true"></span>
              <button
                type="button"
                aria-pressed={hfUnfiltered}
                onclick={() => { hfUnfiltered = !hfUnfiltered; void hfSearchRepos(); }}
                class={`inline-flex h-8 items-center gap-1.5 rounded-full border px-3 text-sm transition-colors duration-150 ${hfUnfiltered ? "border-amber-500/50 bg-amber-500/10 text-amber-200" : "border-ink-700 text-ink-300 hover:border-edge hover:text-ink-50"}`}
              >
                <Icon name="alert" class="h-4 w-4" />Unfiltered only
              </button>
            </div>
            {#if hfUnfiltered}
              <div class="mt-3 rounded-md border border-amber-500/30 bg-amber-500/10 p-3 text-sm leading-6 text-amber-200" transition:slide={{ duration: dur(150) }}>
                These models had their safety training removed ("uncensored" or "abliterated"). They answer requests other models refuse and can produce harmful or explicit text. Use them only if you are an adult and it is legal where you live. <button type="button" class="font-semibold underline" onclick={() => openLegal("terms")}>Terms of Use</button>
              </div>
            {/if}
            {#if hfBusy && hfRepos.length === 0}
              <div class="mt-3 space-y-2" aria-hidden="true">
                {#each [0, 1, 2, 3] as i (i)}<div class="skeleton h-[62px]"></div>{/each}
              </div>
            {/if}
            {#if hfRepos.length > 0}
              <p class="mt-3 text-sm text-ink-400">{hfSearched ? `Results for “${hfSearched}”` : hfSorts.find((o) => o.value === hfSort)?.label} · {hfRepos.length} model folders</p>
            {/if}
            {#if hfRepos.length > 0}
              <div class={`mt-2 space-y-2 transition-opacity duration-150 ${hfBusy ? "opacity-60" : ""}`}>
                {#each hfRepos as repo, i (repo.id)}
                  <div class="lift rounded-md border border-ink-700 bg-ink-950 hover:border-edge" in:fly={stagger(i)}>
                    <button type="button" onclick={() => void hfToggleRepo(repo.id)} class="flex w-full items-center justify-between gap-3 p-3 text-left hover:bg-veil/5">
                      <div class="min-w-0">
                        <div class="truncate font-semibold text-ink-100">{repo.id}</div>
                        <div class="mt-1 flex flex-wrap gap-2 text-sm text-ink-400">
                          <span>{repo.downloads.toLocaleString()} downloads</span>
                          <span>{repo.likes} likes</span>
                          {#if repo.license}<span>{repo.license}</span>{/if}
                          {#if repo.uncensored}<StatusBadge label="Unfiltered" tone="warn" />{/if}
                          {#if repo.gated}<StatusBadge label="Needs access" tone="warn" />{/if}
                        </div>
                      </div>
                      <Icon name="chevron-right" class={`h-4 w-4 shrink-0 transition-transform duration-200 ${hfExpanded === repo.id ? 'rotate-90' : ''}`} />
                    </button>
                    {#if hfExpanded === repo.id}
                      <div class="border-t border-ink-800 p-3" transition:slide={{ duration: dur(180) }}>
                        {#if hfFilesBusy === repo.id}
                          <div class="text-sm text-ink-400">Looking for files…</div>
                        {:else}
                          <div class="mb-3 flex flex-wrap items-center gap-2 text-sm text-ink-400">
                            <span>Use as:</span>
                            {#each [["chat", "Chat"], ["embedding", "Meaning"], ["reranker", "Ranking"], ["code", "Code"]] as [kindValue, label]}
                              <button type="button" onclick={() => { hfKinds = { ...hfKinds, [repo.id]: (hfKinds[repo.id] ?? "chat") === kindValue ? "chat" : kindValue }; }} class={`rounded-full border px-2.5 py-1 text-sm ${((hfKinds[repo.id] ?? "chat") === kindValue) ? 'border-ember-500/40 bg-ember-500/10 text-ember-300' : 'border-ink-700 text-ink-400 hover:border-edge'}`}>{label}</button>
                            {/each}
                          </div>
                          {#each (hfFiles[repo.id] ?? []).filter((file) => !file.is_mmproj) as file (file.path)}
                            <div class="flex items-center justify-between gap-3 border-t border-ink-800 py-2.5">
                              <div class="min-w-0">
                                <div class="truncate text-sm text-ink-200">{file.path}</div>
                                <div class="text-sm text-ink-400">{formatBytes(file.size)}</div>
                              </div>
                              <Button variant="secondary" disabled={hfAdding !== null} onclick={() => void hfAddAndDownload(repo.id, file)}>{hfAdding === file.path ? "Adding…" : "Add and download"}</Button>
                            </div>
                          {/each}
                        {/if}
                      </div>
                    {/if}
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {/if}
      </div>
    {/if}
  {/if}
</div>

{#if pendingRemoveId}
  <ConfirmDialog
    title="Remove this model?"
    message="The downloaded model will be removed from this computer. You can download it again later."
    confirmLabel="Remove model"
    onConfirm={() => void confirmRemoveModel()}
    onCancel={() => pendingRemoveId = null}
  />
{/if}
