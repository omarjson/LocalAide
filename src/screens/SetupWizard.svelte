<script lang="ts">
  import ErrorBanner from "../components/ErrorBanner.svelte";
  import Icon from "../components/Icon.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import ProgressBar from "../components/ProgressBar.svelte";
  import { onMount } from "svelte";
  import { api, errorMessage } from "../lib/api/client";
  import type { HfFileInfo, HfRepo, ModelInfo } from "../lib/api/bindings";
  import { formatBytes } from "../lib/format";
  import { fly } from "svelte/transition";
  import { enterScreen } from "../lib/motion";
  import { openLegal } from "../lib/legal.svelte";
  import LogoMark from "../components/LogoMark.svelte";
  import ProviderForm from "../components/ProviderForm.svelte";
  import { moduleOn } from "../lib/dev.svelte";
  import { applyTheme, currentTheme, type Theme } from "../lib/theme";
  import type { CloudConfigStatus } from "../lib/api/bindings";

  type Step = "detect" | "preset" | "confirm" | "online" | "download" | "done";

  // Checking the computer takes a moment and needs nothing from the user,
  // so it is not a step of its own: it is the start of "Choose a model".
  const stepList: { id: Step; label: string }[] = [
    { id: "preset", label: "Choose a model" },
    { id: "download", label: "Install" },
    { id: "done", label: "Ready" },
  ];
  const stepIndex = (value: Step): number => (value === "download" ? 1 : value === "done" ? 2 : 0);

  type Preset = {
    id: string;
    name: string;
    ram: string;
    ramGb: number;
    fits: string;
    models: string[];
  };

  // Presets come from the backend (the same list the Models screen uses),
  // with model ids resolved to display names from the catalog.
  let presets = $state<Preset[]>([]);
  let presetError = $state(false);

  let {
    onFinished,
    onBusyChange,
    onSkip,
  }: {
    onFinished?: () => void;
    /** Told when a download starts and ends, so the app can hold navigation. */
    onBusyChange?: (busy: boolean) => void;
    /** Leaves setup without a model, for example when offline. */
    onSkip?: () => void;
  } = $props();

  let step = $state<Step>("detect");
  let ramTotal = $state(0);
  let ramUsable = $state(0);
  let budget = $state(0);
  let selected = $state<Preset | null>(null);
  let progress = $state(0);
  let status = $state("Detecting system memory...");
  let detectError = $state(false);
  let setupError = $state<string | null>(null);

  // Navigation waits while a download runs, so it stays in view.
  $effect(() => {
    onBusyChange?.(step === "download");
  });
  $effect(() => () => onBusyChange?.(false));

  const budgetGb = $derived(budget / 1024 / 1024 / 1024);
  const fittingPresets = $derived(presets.filter(p => p.ramGb <= budgetGb));
  const recommended = $derived<Preset | undefined>(fittingPresets[fittingPresets.length - 1] ?? presets[0]);
  // Nothing fits the memory set aside for models: an online provider is
  // the better first suggestion.
  const tightMemory = $derived(presets.length > 0 && fittingPresets.length === 0);
  const onlineAvailable = $derived(moduleOn("online"));
  const otherPresets = $derived(presets.filter((p) => p.id !== recommended?.id));
  let theme = $state<Theme>(currentTheme());
  const chooseTheme = async (value: Theme) => {
    theme = value;
    applyTheme(value);
    try {
      await api.setTheme(value);
    } catch (e: unknown) {
      console.warn("The theme could not be saved", e);
    }
  };

  const detect = async () => {
    setupError = null;
    try {
      const probe = await api.systemProbe();
      ramTotal = probe.total_ram_gb * 1024 * 1024 * 1024;
      ramUsable = probe.available_ram_gb * 1024 * 1024 * 1024;
      budget = probe.budget_gb * 1024 * 1024 * 1024;
      detectError = false;

      // Backend presets are the single source of truth (same list as the
      // Models screen). Model ids resolve to display names via the catalog.
      const [backendPresets, catalog] = await Promise.all([
        api.getModelPresets(),
        api.listModels(),
      ]);
      const nameOf = new Map(catalog.map((m: ModelInfo) => [m.id, m.name]));
      presets = backendPresets.map((p) => ({
        id: p.id,
        name: p.name,
        ram: `~${p.ram_gb.toFixed(1)} GB RAM`,
        ramGb: p.ram_gb,
        fits: p.description.replace(/^Fits /, "") + " total RAM",
        models: p.models.map((id) => nameOf.get(id) ?? id),
      }));
      presetError = false;
      step = "preset";
      status = recommended
        ? `Recommended: ${recommended.name} fits the ${probe.budget_gb.toFixed(1)} GB this computer can give to models.`
        : "No model sizes are available right now.";
    } catch (e: unknown) {
      detectError = true;
      presetError = true;
      setupError = errorMessage(e);
      status = "We could not check this computer.";
    }
  };

  const select = (p: Preset) => { selected = p; step = "confirm"; status = `Install ${p.name} (${p.ram})?`; };
  // The recommended size installs in one click; other sizes confirm first.
  const installRecommended = () => {
    if (!recommended) return;
    selected = recommended;
    void install();
  };
  const onlineSaved = (saved: CloudConfigStatus) => {
    const provider = saved.providers.at(-1);
    status = provider
      ? `${provider.label} is connected. Pick one of its models in the chat's model menu. Messages go to it only while the chat uses one of its models.`
      : "The provider is connected.";
    step = "done";
  };
  const install = async () => {
    if (!selected) return;
    setupError = null;
    const preset = selected;
    step = "download"; status = `Downloading ${preset.name} models...`;
    progress = 0;
    try {
      await api.downloadModelPreset(preset.id, (evt) => {
        if (evt.status === "failed") return;
        progress = Math.round(evt.percent);
        status = progress >= 100
          ? "Starting the model. The first start also downloads the engine for this processor, so it can take a minute."
          : `Downloading ${preset.name} (${evt.model_id})...`;
      });
      step = "done"; status = `${preset.name} is installed and ready to chat. You can switch models any time on the Models screen.`;
    } catch (e: unknown) {
      const message = errorMessage(e);
      setupError = /cancel/i.test(message) ? null : message;
      status = /cancel/i.test(message) ? "Install cancelled." : "The model could not be installed.";
      step = "confirm";
    }
  };

  const cancelInstall = async () => {
    if (!selected) return;
    try {
      await api.cancelPresetDownload(selected.id);
    } catch (e: unknown) {
      setupError = `The download could not stop: ${errorMessage(e)}`;
    }
  };

  // ---- Community models (Hugging Face) ----

  let communityOpen = $state(false);
  let wfQuery = $state("");
  let wfRepos = $state<HfRepo[]>([]);
  let wfBusy = $state(false);
  let wfExpanded = $state<string | null>(null);
  let wfFiles = $state<Record<string, HfFileInfo[]>>({});
  let wfFilesBusy = $state<string | null>(null);
  let wfAdding = $state<string | null>(null);
  let wfError = $state<string | null>(null);
  // Set while a community download runs so Cancel hits the right job.
  let communityModelId = $state<string | null>(null);

  const wfSearch = async () => {
    if (!wfQuery.trim()) return;
    wfBusy = true;
    wfError = null;
    wfExpanded = null;
    try {
      wfRepos = await api.hfSearch(wfQuery.trim(), 10);
      if (!wfRepos.length) wfError = "No community models matched that search.";
    } catch (e) {
      wfError = errorMessage(e);
    } finally {
      wfBusy = false;
    }
  };

  const wfToggleRepo = async (repo: string) => {
    if (wfExpanded === repo) {
      wfExpanded = null;
      return;
    }
    wfExpanded = repo;
    if (wfFiles[repo]) return;
    wfFilesBusy = repo;
    wfError = null;
    try {
      const files = await api.hfRepoFiles(repo);
      wfFiles = { ...wfFiles, [repo]: files };
    } catch (e) {
      wfError = errorMessage(e);
      wfExpanded = null;
    } finally {
      wfFilesBusy = null;
    }
  };

  const wfInstall = async (repo: string, file: HfFileInfo) => {
    wfAdding = file.path;
    wfError = null;
    try {
      const license = wfRepos.find((r) => r.id === repo)?.license ?? null;
      const withMmproj = (wfFiles[repo] ?? []).some((f) => f.is_mmproj && f.path !== file.path);
      const res = await api.hfCustomAdd(repo, file.path, "chat", license, withMmproj);
      const label = file.path.split("/").pop() ?? file.path;
      communityModelId = res.id;
      step = "download";
      progress = 0;
      status = `Downloading ${label}...`;
      await api.downloadModel(res.id, (evt) => {
        if (evt.status === "failed") return;
        progress = Math.round(evt.percent);
        status = `Downloading ${label} (${Math.round(evt.percent)}%)...`;
      });
      // Downloads do not start a model by themselves, so load it for chat now.
      status = `Starting ${label}...`;
      try {
        await api.activateModel(res.id);
        status = `${label} is ready. You can switch models any time on the Models screen.`;
      } catch (e: unknown) {
        status = `${label} is installed but did not start: ${errorMessage(e)} Try it from the Models screen.`;
      }
      step = "done";
    } catch (e: unknown) {
      const message = errorMessage(e);
      wfError = /cancel/i.test(message) ? null : `The model could not be installed: ${message}`;
      status = /cancel/i.test(message) ? "Install cancelled." : "The community model could not be installed.";
      step = "preset";
    } finally {
      wfAdding = null;
      communityModelId = null;
    }
  };

  const cancelCommunity = async () => {
    if (!communityModelId) return;
    try {
      await api.cancelModelDownload(communityModelId);
    } catch (e: unknown) {
      setupError = `The download could not stop: ${errorMessage(e)}`;
    }
  };

  const retrySetup = () => {
    if (step === "detect") return void detect();
    if (step === "confirm" && selected) return void install();
    return void detect();
  };

  onMount(detect);
</script>

<div class="mx-auto max-w-3xl">
  <div class="ember-glow -mx-6 mb-2 flex items-center gap-4 rounded-lg px-6 pt-2" in:fly={enterScreen()}>
    <LogoMark class="h-14 w-14 shrink-0 drop-shadow-lg" />
    <div class="min-w-0 flex-1">
      <PageHeader title="Welcome to LocalAide" description="Pick an AI model that fits this computer. It runs here, so your chats stay private." />
    </div>
  </div>
  <ol class="mb-5 flex items-center gap-2" aria-label="Setup steps">
    {#each stepList as item, i (item.id)}
      {@const current = stepIndex(step)}
      <li class="flex min-w-0 flex-1 items-center gap-2" aria-current={i === current ? "step" : undefined}>
        <span class={`grid h-6 w-6 shrink-0 place-items-center rounded-full text-xs font-semibold transition-colors duration-300 ${i < current ? "bg-emerald-500/20 text-emerald-300" : i === current ? "bg-ember-500 text-on-ember" : "bg-ink-800 text-ink-400"}`}>
          {#if i < current}<Icon name="check" class="h-3.5 w-3.5" />{:else}{i + 1}{/if}
        </span>
        <span class={`hidden truncate text-sm sm:inline ${i === current ? "font-semibold text-ink-50" : "text-ink-400"}`}>{item.label}</span>
        {#if i < stepList.length - 1}
          <span class="relative h-px flex-1 overflow-hidden bg-ink-700" aria-hidden="true"><span class={`absolute inset-y-0 left-0 bg-ember-500 transition-[width] duration-500 ease-out ${i < current ? "w-full" : "w-0"}`}></span></span>
        {/if}
      </li>
    {/each}
  </ol>
  {#if setupError}<div class="mb-4"><ErrorBanner message={setupError} onRetry={retrySetup} onDismiss={() => setupError = null} /></div>{/if}
  <div class="rounded-lg border border-edge bg-ink-900/60 p-6 shadow-sm">
  {#key step}
  <div in:fly={enterScreen()}>

    {#if step === "detect"}
      <div class="flex flex-col items-center gap-3 py-10">
        {#if detectError}
          <p class="text-sm text-rose-300">{status}</p>
          <button
            onclick={detect}
            class="px-4 py-2 rounded-lg bg-ember-600 text-on-ember text-sm font-medium transition-colors"
          >
            Retry
          </button>
        {:else}
          <div class="w-12 h-12 rounded-full border-4 border-edge border-t-ember-500 animate-spin"></div>
          <p class="text-sm text-ink-400">{status}</p>
        {/if}
      </div>
    {:else if step === "preset"}
      <div class="grid grid-cols-3 gap-2 text-sm mb-6">
        <div class="px-3 py-2 rounded-lg border border-edge bg-ink-950">
          <span class="block text-xs text-ink-400">Total</span>
          <span class="text-ink-200">{formatBytes(ramTotal)}</span>
        </div>
        <div class="px-3 py-2 rounded-lg border border-edge bg-ink-950">
          <span class="block text-xs text-ink-400">Usable</span>
          <span class="text-ink-200">{formatBytes(ramUsable)}</span>
        </div>
        <div class="px-3 py-2 rounded-lg border border-ember-500/30 bg-ember-500/10">
          <span class="block text-xs text-ember-300">For models</span>
          <span class="text-ember-200">{formatBytes(budget)}</span>
        </div>
      </div>
      <p class="-mt-3 mb-5 text-xs text-ink-500">"For models" is the share of memory LocalAide lets a model use, so the rest of the computer stays quick. You can change it later in Settings → General.</p>
      {#if presetError || !recommended}
        <p class="mb-4 text-sm text-ink-400">Model sizes could not be loaded. Retry, or connect an online provider below.</p>
      {:else}
        <!-- The one-click path: the largest size that fits this computer. -->
        <div class={`rounded-lg border p-4 ${tightMemory ? "border-edge bg-ink-950" : "border-ember-500/40 bg-ember-500/10"}`}>
          <div class="flex flex-wrap items-start justify-between gap-3">
            <div class="min-w-0">
              <div class="flex items-center gap-2 font-medium text-ink-50">
                {recommended.name}
                {#if !tightMemory}<span class="rounded border border-ember-500/30 bg-ember-500/20 px-1.5 py-0.5 text-xs text-ember-300">Recommended</span>{/if}
              </div>
              <div class="mt-0.5 text-xs text-ink-400">{recommended.ram} · {recommended.models.join(", ")}</div>
              {#if tightMemory}
                <p class="mt-2 text-xs text-amber-300">This computer has less memory for models than this size needs. It may run slowly; an online provider below is the smoother start.</p>
              {/if}
            </div>
            <button onclick={installRecommended} class="shrink-0 rounded-lg bg-ember-600 px-4 py-2 text-sm font-medium text-on-ember transition-colors hover:bg-ember-500">Install {recommended.name}</button>
          </div>
        </div>
        {#if otherPresets.length > 0}
          <details class="mt-3 group">
            <summary class="cursor-pointer select-none text-sm text-ink-400 hover:text-ink-200">Other sizes</summary>
            <div class="mt-2 space-y-2">
              {#each otherPresets as p (p.id)}
                {@const fits = p.ramGb <= budgetGb}
                <button onclick={() => fits && select(p)} disabled={!fits} class={`w-full text-left px-4 py-3 rounded-lg border transition ${fits ? 'border-edge hover:border-ember-500/50 hover:bg-veil/5' : 'border-ink-900 bg-ink-950/50 opacity-50 cursor-not-allowed'}`}>
                  <div class="flex items-center justify-between">
                    <div>
                      <div class="font-medium text-ink-100">{p.name}</div>
                      <div class="text-xs text-ink-400">{p.ram} · {p.fits} · {fits ? 'Fits' : 'Too large'}</div>
                    </div>
                    <div class="text-xs text-ink-400">{#if !fits}<Icon name="close" class="w-3.5 h-3.5" />{/if}</div>
                  </div>
                  <ul class="mt-2 text-xs text-ink-400 list-disc list-inside">
                    {#each p.models as m}<li>{m}</li>{/each}
                  </ul>
                </button>
              {/each}
            </div>
          </details>
        {/if}
      {/if}

      {#if onlineAvailable}
        <button onclick={() => { setupError = null; step = "online"; }} class={`mt-4 flex w-full items-center gap-3 rounded-lg border px-4 py-3 text-left transition hover:bg-veil/5 ${tightMemory ? "border-ember-500/40 bg-ember-500/10" : "border-edge hover:border-ember-500/50"}`}>
          <span class="grid h-8 w-8 shrink-0 place-items-center rounded-lg bg-ink-800 text-ink-200"><Icon name="cloud" class="h-4 w-4" /></span>
          <span class="min-w-0 flex-1">
            <span class="block text-sm font-medium text-ink-100">Use an online provider instead{#if tightMemory} <span class="ml-1 rounded border border-ember-500/30 bg-ember-500/20 px-1.5 py-0.5 text-xs text-ember-300">Recommended</span>{/if}</span>
            <span class="block text-xs text-ink-400">Nothing to download. Some providers are free and need no key. Messages go to the provider you choose.</span>
          </span>
          <Icon name="chevron-right" class="h-4 w-4 shrink-0 text-ink-500" />
        </button>
      {/if}

      <div class="mt-6 border-t border-edge pt-4">
        {#if !communityOpen}
          <div class="flex flex-wrap items-center gap-3">
            <button
              onclick={() => (communityOpen = true)}
              class="text-xs text-ember-300 hover:underline"
            >
              Or grab a community model from Hugging Face →
            </button>
            {#if onSkip}
              <button onclick={onSkip} class="ml-auto text-sm text-ink-400 hover:text-ink-200 hover:underline">Set up later</button>
            {/if}
          </div>
        {:else}
          <div class="text-xs font-semibold text-ink-300 mb-2">Community models (Hugging Face)</div>
          <div class="flex gap-2">
            <input
              type="text"
              bind:value={wfQuery}
              placeholder="Search public community models…"
              onkeydown={(e) => e.key === "Enter" && wfSearch()}
              class="flex-1 bg-ink-950 border border-edge rounded-lg px-3 py-2 text-xs text-ink-200 placeholder:text-ink-600 outline-none focus:border-ember-500/60"
            />
            <button
              onclick={wfSearch}
              disabled={wfBusy || !wfQuery.trim()}
              class="px-3 py-2 bg-ink-800 hover:bg-ink-700 disabled:opacity-40 rounded-lg text-xs font-medium text-ink-200 transition-colors"
            >
              {wfBusy ? "Searching…" : "Search"}
            </button>
          </div>
          {#if wfError}
            <div class="mt-2"><ErrorBanner message={wfError} onRetry={() => void wfSearch()} onDismiss={() => wfError = null} /></div>
          {/if}
          <div class="mt-3 space-y-1.5 max-h-72 overflow-auto">
            {#each wfRepos as repo (repo.id)}
              <div class="rounded-lg border border-edge bg-ink-950/60">
                <button
                  onclick={() => wfToggleRepo(repo.id)}
                  class="w-full text-left px-3 py-2 hover:bg-veil/[0.02] transition-colors"
                >
                  <div class="flex items-center justify-between gap-2">
                    <span class="text-xs text-ink-200 font-medium truncate">{repo.id}</span>
                    <span class="text-sm text-ink-400 shrink-0">{repo.downloads.toLocaleString()} downloads</span>
                  </div>
                  <div class="text-sm text-ink-400">
                    {#if repo.license}{repo.license}{/if}
                    {#if repo.gated}<span class="text-amber-300"> · gated — needs a hub login, refused here</span>{/if}
                  </div>
                </button>
                {#if wfExpanded === repo.id}
                  <div class="border-t border-edge px-3 py-2">
                    {#if wfFilesBusy === repo.id}
                      <div class="text-sm text-ink-400">Listing files…</div>
                    {:else if (wfFiles[repo.id] ?? []).length === 0}
                      <div class="text-sm text-ink-400">No model files in this folder.</div>
                    {:else}
                      {#each wfFiles[repo.id] as file (file.path)}
                        <div class="flex items-center justify-between gap-2 py-1">
                          <span class="text-xs text-ink-300 font-mono truncate">{file.path}</span>
                          <span class="flex items-center gap-2 shrink-0">
                            <span class="text-sm text-ink-400">{formatBytes(file.size)}</span>
                            <button
                              onclick={() => wfInstall(repo.id, file)}
                              disabled={wfAdding !== null}
                              class="px-2 py-1 rounded bg-ember-600 text-on-ember text-xs font-medium disabled:opacity-40"
                            >
                              {wfAdding === file.path ? "Adding…" : "Install"}
                            </button>
                          </span>
                        </div>
                      {/each}
                    {/if}
                  </div>
                {/if}
              </div>
            {/each}
          </div>
          <p class="mt-2 text-xs text-ink-500">
            Community model files and licenses are not checked by LocalAide. Each download is checked against the source file.
          </p>
        {/if}
      </div>
    {:else if step === "online"}
      <p class="mb-3 text-sm leading-6 text-ink-400">Choose a provider. Its models appear in the chat's model menu; a message goes online only while the chat uses one of them. A key stays on this computer and is sent only to that provider.</p>
      <ProviderForm editing={null} onSaved={onlineSaved} onCancel={() => (step = "preset")} />
    {:else if step === "confirm"}
      <div class="bg-ink-950 rounded-lg p-4 mb-6 border border-edge">
        <div class="font-medium text-ink-100">{selected?.name} preset</div>
        <div class="text-sm text-ink-400">{selected?.models.join(", ")}</div>
      </div>
      <p class="text-sm text-ink-400 mb-6">{status}</p>
      <div class="flex flex-wrap items-center gap-3">
        <button onclick={() => step = "preset"} class="px-4 py-2 rounded-lg bg-veil/5 border border-edge text-sm text-ink-300 hover:bg-veil/10">Change preset</button>
        <button onclick={install} class="px-4 py-2 rounded-lg bg-ember-600 text-on-ember text-sm font-medium">Install</button>
        {#if onSkip}
          <button onclick={onSkip} class="ml-auto text-sm text-ink-400 hover:text-ink-200 hover:underline">Set up later</button>
        {/if}
      </div>
      <p class="mt-4 text-xs text-ink-400">Installing needs an internet connection. If you are offline, choose Set up later and download a model from the Models screen when you are back online.</p>
    {:else if step === "download"}
      <p class="text-sm text-ink-400 mb-4">{status}</p>
      <ProgressBar value={progress} label="Installing the model" class="mb-3 h-2 w-full" />
      <div class="text-sm text-ink-400 mb-4">{progress}%</div>
      <button
        onclick={communityModelId ? cancelCommunity : cancelInstall}
        class="px-4 py-2 rounded-lg border border-rose-500/30 text-rose-400 hover:bg-rose-500/10 text-sm transition-colors"
      >
        Cancel install
      </button>
    {:else if step === "done"}
      <div class="flex items-start gap-3 mb-6">
        <span class="size-6 grid place-items-center rounded-full bg-emerald-500/20 text-emerald-300"><Icon name="check" class="w-3.5 h-3.5" /></span>
        <div>
          <div class="font-medium text-ink-100">Setup complete</div>
          <div class="text-sm text-ink-400">{status}</div>
        </div>
      </div>
      <div class="mb-6 flex flex-wrap items-center gap-3 rounded-lg border border-edge bg-ink-950 px-4 py-3">
        <span class="text-sm text-ink-300">Look</span>
        <div class="flex gap-1.5" role="radiogroup" aria-label="Theme">
          {#each [{ id: "dark", label: "Dark" }, { id: "light", label: "Light" }] as option (option.id)}
            <button type="button" role="radio" aria-checked={theme === option.id} onclick={() => void chooseTheme(option.id as Theme)} class={`rounded-lg border px-3 py-1.5 text-sm transition-colors ${theme === option.id ? "border-ember-500/50 bg-ember-500/15 text-ember-200" : "border-edge text-ink-300 hover:bg-veil/5"}`}>{option.label}</button>
          {/each}
        </div>
        <span class="text-xs text-ink-500">Colors, fonts and size are in Settings → General → Appearance.</span>
      </div>
      <button onclick={() => (onFinished ? onFinished() : location.reload())} class="px-4 py-2 rounded-lg bg-ember-600 text-on-ember text-sm font-medium transition-colors hover:bg-ember-500">Open app</button>
    {/if}
  </div>
  {/key}
  </div>
  <p class="mt-4 text-center text-xs text-ink-500">
    By using LocalAide you agree to the
    <button type="button" class="underline hover:text-ember-300" onclick={() => openLegal("terms")}>Terms of Use</button>
    and the
    <button type="button" class="underline hover:text-ember-300" onclick={() => openLegal("privacy")}>Privacy Policy</button>.
    No account, no tracking.
  </p>
</div>

<style>
  .animate-spin { animation: spin 1s linear infinite; }
  @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
</style>
