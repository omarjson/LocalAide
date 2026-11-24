<script lang="ts">
  import ErrorBanner from "../components/ErrorBanner.svelte";
  import Icon from "../components/Icon.svelte";
  import ProgressBar from "../components/ProgressBar.svelte";
  import { onMount } from "svelte";
  import { api, errorMessage } from "../lib/api/client";
  import type { HfFileInfo, HfRepo, ModelInfo } from "../lib/api/bindings";
  import { formatBytes } from "../lib/format";
  import { fly } from "svelte/transition";
  import { enterScreen } from "../lib/motion";
  import { openLegal } from "../lib/legal.svelte";
  import GhostMascot, { type GhostMood } from "../components/GhostMascot.svelte";
  import ProviderForm from "../components/ProviderForm.svelte";
  import { moduleOn } from "../lib/dev.svelte";
  import { applyTheme, currentTheme, type Theme } from "../lib/theme";
  import type { CloudConfigStatus } from "../lib/api/bindings";

  type Step = "detect" | "preset" | "confirm" | "online" | "download" | "done";

  // Checking the computer takes a moment and needs nothing from the user,
  // so it is not a step of its own: it is the start of "Choose a model".
  const stepList: { id: Step; label: string }[] = [
    { id: "preset", label: "Choose" },
    { id: "download", label: "Download" },
    { id: "done", label: "Say hi" },
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
  // One clear path: the Tiny model. If the backend list has no Tiny, the
  // old pick (the largest size that fits) stays the fallback.
  const recommended = $derived<Preset | undefined>(
    presets.find((p) => p.id === "tiny") ?? fittingPresets[fittingPresets.length - 1] ?? presets[0],
  );
  // Nothing fits the memory set aside for models: an online provider is
  // the better first suggestion.
  const tightMemory = $derived(presets.length > 0 && fittingPresets.length === 0);
  const onlineAvailable = $derived(moduleOn("online"));
  const otherPresets = $derived(presets.filter((p) => p.id !== recommended?.id));

  // ---- Presentation only (no calls): plain words, the mascot, the disclosure ----

  /** How well a size fits, in words instead of memory figures. */
  const fitWords = (p: Preset): string =>
    p.ramGb > budgetGb ? "Too big for this computer" : p.ramGb <= budgetGb / 2 ? "Fits easily" : "Fits";
  const SIZE_BLURBS: Record<string, string> = {
    tiny: "Quick and light. Great for everyday questions, notes and drafts.",
    small: "A little smarter, still quick.",
    balanced: "Thoughtful answers for a roomy computer.",
    advanced: "The deepest answers, for computers with lots of memory.",
  };
  const blurb = (p: Preset): string => SIZE_BLURBS[p.id] ?? p.models.join(", ");
  // Other sizes, online providers and community models wait behind one
  // disclosure; it opens by itself when nothing fits.
  let moreOpen = $state(false);
  $effect(() => {
    if (tightMemory) moreOpen = true;
  });
  const mood = $derived<GhostMood>(
    step === "detect" ? "looking" : step === "download" ? "working" : step === "done" ? "done" : "hello",
  );
  const bubble = $derived(
    step === "detect"
      ? detectError
        ? "Hmm, I couldn't get a good look at this computer."
        : "Taking a quick look at this computer…"
      : step === "preset"
        ? "Hi! I'm your LocalAide. I live right here on this computer, so what you tell me stays with you."
        : step === "confirm"
          ? "Good choice. Ready when you are."
          : step === "online"
            ? "Pick a provider and I'll use it when you choose one of its models."
            : step === "download"
              ? "Fetching my brain. This is the only part that needs the internet."
              : "All set! Say hi whenever you like.",
  );
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
        ? `${recommended.name} fits this computer. It's quick, light, and a great place to start.`
        : "No model sizes are available right now.";
    } catch (e: unknown) {
      detectError = true;
      presetError = true;
      setupError = errorMessage(e);
      status = "We could not check this computer.";
    }
  };

  const select = (p: Preset) => { selected = p; step = "confirm"; status = `Install ${p.name}? ${fitWords(p)} on this computer.`; };
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

<div class="setup mx-auto max-w-2xl">
  <!-- The ghost leads every step; its line says what is happening. -->
  <div class="flex items-end gap-4" in:fly={enterScreen()}>
    {#key mood}
      <GhostMascot {mood} class="h-24 w-24 shrink-0 sm:h-28 sm:w-28" />
    {/key}
    <div class="bubble min-w-0 flex-1" aria-live="polite">
      <p class="text-[15px] leading-6 text-ink-100">{bubble}</p>
    </div>
  </div>

  <ol class="mt-6 mb-4 flex items-center gap-3" aria-label="Setup steps">
    {#each stepList as item, i (item.id)}
      {@const current = stepIndex(step)}
      <li class="flex items-center gap-2" aria-current={i === current ? "step" : undefined}>
        <span class={`step-dot ${i < current ? "step-dot-done" : i === current ? "step-dot-now" : ""}`}>
          {#if i < current}<Icon name="check" class="h-3 w-3" />{/if}
        </span>
        <span class={`text-sm ${i === current ? "font-semibold text-ink-50" : "text-ink-400"}`}>{item.label}</span>
        {#if i < stepList.length - 1}<span class="h-px w-6 bg-ink-700 sm:w-10" aria-hidden="true"></span>{/if}
      </li>
    {/each}
  </ol>

  {#if setupError}<div class="mb-4"><ErrorBanner message={setupError} onRetry={retrySetup} onDismiss={() => setupError = null} /></div>{/if}

  <div class="card">
  {#key step}
  <div in:fly={enterScreen()}>

    {#if step === "detect"}
      <div class="py-6">
        {#if detectError}
          <p class="text-sm text-rose-300">{status}</p>
          <button onclick={detect} class="btn-primary mt-4">Try again</button>
        {:else}
          <p class="text-sm text-ink-400">{status}</p>
          <div class="sweep mt-4" aria-hidden="true"><span></span></div>
        {/if}
      </div>
    {:else if step === "preset"}
      {#if presetError || !recommended}
        <p class="text-sm text-ink-400">I couldn't load the model sizes. Try again, or open More options to connect an online provider.</p>
      {:else}
        <!-- The one recommended path. -->
        <div class="flex flex-wrap items-center gap-4">
          <div class="min-w-0 flex-1">
            <div class="flex flex-wrap items-center gap-2">
              <h2 class="text-xl font-semibold text-ink-50">{recommended.name}</h2>
              {#if !tightMemory}<span class="pill-ember">Recommended</span>{/if}
              <span class={`pill ${recommended.ramGb > budgetGb ? "pill-warn" : "pill-ok"}`}>{fitWords(recommended)}</span>
            </div>
            <p class="mt-1 text-sm leading-6 text-ink-300">{blurb(recommended)}</p>
            <p class="mt-1 text-xs text-ink-500">Runs on this computer. You can switch any time on the Models screen.</p>
          </div>
          <button onclick={installRecommended} class="btn-primary shrink-0">
            <Icon name="download" class="h-4 w-4" />Install {recommended.name}
          </button>
        </div>
        {#if tightMemory}
          <p class="mt-4 text-sm text-amber-300">This computer is short on room for models, so answers may be slow. More options below has an online choice that needs no download.</p>
        {/if}
      {/if}

      <details class="more mt-5" bind:open={moreOpen}>
        <summary class="more-summary">
          <Icon name="chevron-right" class="more-chevron h-4 w-4" />More options
        </summary>
        <div class="mt-3 space-y-4">
          {#if otherPresets.length > 0}
            <div>
              <h3 class="mb-2 text-sm font-semibold text-ink-200">Other sizes</h3>
              <div class="space-y-1.5">
                {#each otherPresets as p (p.id)}
                  {@const fits = p.ramGb <= budgetGb}
                  <button onclick={() => fits && select(p)} disabled={!fits} class={`option ${fits ? "" : "option-off"}`}>
                    <span class="min-w-0 flex-1">
                      <span class="block font-medium text-ink-100">{p.name}</span>
                      <span class="block text-xs text-ink-400">{blurb(p)}</span>
                    </span>
                    <span class={`pill shrink-0 ${fits ? "pill-ok" : "pill-off"}`}>{fitWords(p)}</span>
                  </button>
                {/each}
              </div>
            </div>
          {/if}

          {#if onlineAvailable}
            <button onclick={() => { setupError = null; step = "online"; }} class="option">
              <span class="grid h-8 w-8 shrink-0 place-items-center rounded-lg bg-ink-800 text-ink-200"><Icon name="cloud" class="h-4 w-4" /></span>
              <span class="min-w-0 flex-1">
                <span class="block text-sm font-medium text-ink-100">Use an online provider{#if tightMemory} <span class="pill-ember ml-1">Recommended</span>{/if}</span>
                <span class="block text-xs text-ink-400">Nothing to download. Some are free and need no key. Messages go to the provider you choose.</span>
              </span>
              <Icon name="chevron-right" class="h-4 w-4 shrink-0 text-ink-500" />
            </button>
          {/if}

          <div>
            {#if !communityOpen}
              <button onclick={() => (communityOpen = true)} class="option">
                <span class="grid h-8 w-8 shrink-0 place-items-center rounded-lg bg-ink-800 text-ink-200"><Icon name="globe" class="h-4 w-4" /></span>
                <span class="min-w-0 flex-1">
                  <span class="block text-sm font-medium text-ink-100">Community models from Hugging Face</span>
                  <span class="block text-xs text-ink-400">Search public models. Their files and licenses are not checked by LocalAide.</span>
                </span>
                <Icon name="chevron-right" class="h-4 w-4 shrink-0 text-ink-500" />
              </button>
            {:else}
              <div class="text-sm font-semibold text-ink-200 mb-2">Community models (Hugging Face)</div>
              <div class="flex gap-2">
                <input
                  type="text"
                  bind:value={wfQuery}
                  placeholder="Search public community models…"
                  onkeydown={(e) => e.key === "Enter" && wfSearch()}
                  class="flex-1 bg-ink-950 border border-edge rounded-lg px-3 py-2 text-sm text-ink-200 placeholder:text-ink-600 outline-none focus:border-ember-500/60"
                />
                <button
                  onclick={wfSearch}
                  disabled={wfBusy || !wfQuery.trim()}
                  class="px-3 py-2 bg-ink-800 hover:bg-ink-700 disabled:opacity-40 rounded-lg text-sm font-medium text-ink-200 transition-colors"
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
        </div>
      </details>

      {#if onSkip}
        <div class="mt-4 flex justify-end">
          <button onclick={onSkip} class="text-sm text-ink-400 hover:text-ink-200 hover:underline">Set up later</button>
        </div>
      {/if}
    {:else if step === "online"}
      <p class="mb-3 text-sm leading-6 text-ink-400">Choose a provider. Its models appear in the chat's model menu; a message goes online only while the chat uses one of them. A key stays on this computer and is sent only to that provider.</p>
      <ProviderForm editing={null} onSaved={onlineSaved} onCancel={() => (step = "preset")} />
    {:else if step === "confirm"}
      <div class="flex flex-wrap items-center gap-2">
        <h2 class="text-xl font-semibold text-ink-50">{selected?.name}</h2>
        {#if selected}<span class={`pill ${selected.ramGb > budgetGb ? "pill-warn" : "pill-ok"}`}>{fitWords(selected)}</span>{/if}
      </div>
      {#if selected}<p class="mt-1 text-sm text-ink-300">{blurb(selected)}</p>{/if}
      <p class="mt-3 text-sm text-ink-400">{status}</p>
      <div class="mt-5 flex flex-wrap items-center gap-3">
        <button onclick={install} class="btn-primary"><Icon name="download" class="h-4 w-4" />Install</button>
        <button onclick={() => step = "preset"} class="btn-quiet">Back</button>
        {#if onSkip}
          <button onclick={onSkip} class="ml-auto text-sm text-ink-400 hover:text-ink-200 hover:underline">Set up later</button>
        {/if}
      </div>
      <p class="mt-4 text-xs text-ink-400">Installing needs an internet connection. If you are offline, choose Set up later and download a model from the Models screen when you are back online.</p>
    {:else if step === "download"}
      <p class="text-sm text-ink-300">{status}</p>
      <div class="mt-4 flex items-center gap-3">
        <ProgressBar value={progress} label="Installing the model" class="h-2.5 flex-1" />
        <span class="w-12 text-right text-sm font-semibold tabular-nums text-ink-100">{progress}%</span>
      </div>
      <button
        onclick={communityModelId ? cancelCommunity : cancelInstall}
        class="mt-5 rounded-lg border border-rose-500/30 px-4 py-2 text-sm text-rose-400 transition-colors hover:bg-rose-500/10"
      >
        Cancel
      </button>
    {:else if step === "done"}
      <div class="flex items-start gap-3">
        <span class="done-check"><Icon name="check" class="h-4 w-4" /></span>
        <div class="min-w-0">
          <h2 class="text-xl font-semibold text-ink-50">All set!</h2>
          <p class="mt-1 text-sm text-ink-400">{status}</p>
        </div>
      </div>
      <div class="mt-5 flex flex-wrap items-center gap-3 rounded-lg border border-edge bg-ink-950 px-4 py-3">
        <span class="text-sm text-ink-300">Look</span>
        <div class="flex gap-1.5" role="radiogroup" aria-label="Theme">
          {#each [{ id: "dark", label: "Dark" }, { id: "light", label: "Light" }] as option (option.id)}
            <button type="button" role="radio" aria-checked={theme === option.id} onclick={() => void chooseTheme(option.id as Theme)} class={`rounded-lg border px-3 py-1.5 text-sm transition-colors ${theme === option.id ? "border-ember-500/50 bg-ember-500/15 text-ember-200" : "border-edge text-ink-300 hover:bg-veil/5"}`}>{option.label}</button>
          {/each}
        </div>
        <span class="text-xs text-ink-500">More in Settings → General → Appearance.</span>
      </div>
      <button onclick={() => (onFinished ? onFinished() : location.reload())} class="btn-primary mt-5">
        <Icon name="chat" class="h-4 w-4" />Start chatting
      </button>
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
  .bubble {
    position: relative;
    margin-bottom: 1.25rem;
    border-radius: 1rem;
    border: 1px solid var(--color-edge);
    background: var(--color-ink-900);
    padding: 0.75rem 1rem;
    box-shadow: 0 8px 20px -14px rgb(0 0 0 / 0.5);
  }
  /* The tail points at the ghost. */
  .bubble::before {
    content: "";
    position: absolute;
    left: -7px;
    bottom: 14px;
    height: 12px;
    width: 12px;
    transform: rotate(45deg);
    border-left: 1px solid var(--color-edge);
    border-bottom: 1px solid var(--color-edge);
    background: var(--color-ink-900);
  }
  .card {
    border-radius: 0.75rem;
    border: 1px solid var(--color-edge);
    background: color-mix(in oklab, var(--color-ink-900) 70%, transparent);
    padding: 1.5rem;
  }
  .step-dot {
    display: grid;
    height: 1.25rem;
    width: 1.25rem;
    place-items: center;
    border-radius: 9999px;
    border: 2px solid var(--color-ink-700);
    transition: background-color 300ms ease-out, border-color 300ms ease-out;
  }
  .step-dot-now {
    border-color: var(--color-ember-500);
    background: color-mix(in oklab, var(--color-ember-500) 25%, transparent);
  }
  .step-dot-done {
    border-color: transparent;
    background: var(--color-ember-500);
    color: var(--color-on-ember);
  }
  .btn-primary {
    display: inline-flex;
    min-height: 44px;
    align-items: center;
    gap: 0.5rem;
    border-radius: 0.6rem;
    background: var(--color-ember-600);
    padding: 0 1.25rem;
    font-size: 0.9375rem;
    font-weight: 600;
    color: var(--color-on-ember);
    transition: background-color 150ms ease-out, transform 120ms ease-out;
  }
  .btn-primary:hover {
    background: var(--color-ember-500);
  }
  .btn-primary:active {
    transform: translateY(1px);
  }
  .btn-quiet {
    min-height: 44px;
    border-radius: 0.6rem;
    border: 1px solid var(--color-edge);
    padding: 0 1rem;
    font-size: 0.875rem;
    color: var(--color-ink-300);
  }
  .btn-quiet:hover {
    background: color-mix(in oklab, var(--color-veil) 5%, transparent);
  }
  .pill,
  .pill-ember {
    border-radius: 9999px;
    padding: 0.05rem 0.55rem;
    font-size: 0.8125rem;
    font-weight: 500;
  }
  .pill-ember {
    border: 1px solid color-mix(in oklab, var(--color-ember-500) 35%, transparent);
    background: color-mix(in oklab, var(--color-ember-500) 15%, transparent);
    color: var(--color-ember-300);
  }
  .pill-ok {
    background: color-mix(in oklab, #10b981 14%, transparent);
    color: var(--color-emerald-300);
  }
  .pill-warn {
    background: color-mix(in oklab, #f59e0b 14%, transparent);
    color: var(--color-amber-300);
  }
  .pill-off {
    background: var(--color-ink-800);
    color: var(--color-ink-400);
  }
  .more-summary {
    display: inline-flex;
    cursor: pointer;
    list-style: none;
    align-items: center;
    gap: 0.35rem;
    border-radius: 0.5rem;
    padding: 0.25rem 0.4rem 0.25rem 0.15rem;
    font-size: 0.875rem;
    font-weight: 500;
    color: var(--color-ink-300);
    user-select: none;
  }
  .more-summary::-webkit-details-marker {
    display: none;
  }
  .more-summary:hover {
    color: var(--color-ink-50);
  }
  .more :global(.more-chevron) {
    transition: transform 150ms ease-out;
  }
  .more[open] :global(.more-chevron) {
    transform: rotate(90deg);
  }
  .option {
    display: flex;
    width: 100%;
    align-items: center;
    gap: 0.75rem;
    border-radius: 0.6rem;
    border: 1px solid var(--color-edge);
    padding: 0.65rem 0.85rem;
    text-align: left;
    transition: background-color 150ms ease-out, border-color 150ms ease-out;
  }
  .option:hover:not(:disabled) {
    border-color: color-mix(in oklab, var(--color-ember-500) 50%, transparent);
    background: color-mix(in oklab, var(--color-veil) 4%, transparent);
  }
  .option-off {
    cursor: not-allowed;
    opacity: 0.6;
  }
  /* Indeterminate: a short bar sweeps across while the computer is checked. */
  .sweep {
    height: 4px;
    overflow: hidden;
    border-radius: 9999px;
    background: var(--color-ink-800);
  }
  .sweep span {
    display: block;
    height: 100%;
    width: 35%;
    border-radius: 9999px;
    background: var(--color-ember-500);
    animation: sweep 1.2s ease-in-out infinite;
  }
  @keyframes sweep {
    from { transform: translateX(-100%); }
    to { transform: translateX(290%); }
  }
  /* The done moment: one pop and one ring, then still. */
  .done-check {
    position: relative;
    display: grid;
    height: 2rem;
    width: 2rem;
    flex-shrink: 0;
    place-items: center;
    border-radius: 9999px;
    background: var(--color-ember-500);
    color: var(--color-on-ember);
    animation: done-pop 600ms cubic-bezier(0.34, 1.56, 0.64, 1) 1;
  }
  .done-check::after {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: 9999px;
    border: 2px solid var(--color-ember-500);
    opacity: 0;
    animation: done-ring 900ms ease-out 150ms 1;
  }
  @keyframes done-pop {
    0% { transform: scale(0.4); }
    100% { transform: scale(1); }
  }
  @keyframes done-ring {
    0% { transform: scale(1); opacity: 0.8; }
    100% { transform: scale(2.4); opacity: 0; }
  }
  @media (prefers-reduced-motion: reduce) {
    .sweep span,
    .done-check,
    .done-check::after {
      animation: none;
    }
  }
</style>
