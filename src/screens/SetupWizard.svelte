<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import { onMount } from "svelte";
  import { api } from "../lib/api/client";
  import type { ModelInfo } from "../lib/api/bindings";
  import { formatBytes } from "../lib/format";

  type Step = "detect" | "preset" | "confirm" | "download" | "done";

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

  let { onFinished }: { onFinished?: () => void } = $props();

  let step = $state<Step>("detect");
  let ramTotal = $state(0);
  let ramUsable = $state(0);
  let budget = $state(0);
  let selected = $state<Preset | null>(null);
  let progress = $state(0);
  let status = $state("Detecting system memory...");
  let detectError = $state(false);

  const budgetGb = $derived(budget / 1024 / 1024 / 1024);
  const fittingPresets = $derived(presets.filter(p => p.ramGb <= budgetGb));
  const recommended = $derived(fittingPresets[fittingPresets.length - 1] ?? presets[0]);

  const detect = async () => {
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
      status = `Recommended: ${recommended.name} preset fits your ${probe.budget_gb.toFixed(1)} GB budget.`;
    } catch (e: any) {
      detectError = true;
      presetError = true;
      status = "System probe unavailable: " + (e?.message ?? String(e));
    }
  };

  const select = (p: Preset) => { selected = p; step = "confirm"; status = `Install ${p.name} preset (~${p.ram})?`; };
  const install = async () => {
    if (!selected) return;
    const preset = selected;
    step = "download"; status = `Downloading ${preset.name} models...`;
    progress = 0;
    try {
      await api.downloadModelPreset(preset.id, (evt) => {
        if (evt.status === "failed") return;
        progress = Math.round(evt.percent);
        status = `Downloading ${preset.name} models (${evt.model_id})...`;
      });
      step = "done"; status = `${preset.name} preset installed.`;
    } catch (e: any) {
      const msg = String(e?.message ?? e);
      status = /cancel/i.test(msg) ? "Install cancelled." : `Install failed: ${msg}`;
      step = "confirm";
    }
  };

  const cancelInstall = async () => {
    if (!selected) return;
    try {
      await api.cancelPresetDownload(selected.id);
    } catch {}
  };

  onMount(detect);
</script>

<div class="max-w-2xl mx-auto">
  <div class="rounded-2xl border border-ink-800 bg-ink-900/60 p-6 shadow-[0_0_80px_rgba(255,122,26,0.08)]">
    <h2 class="text-lg font-semibold tracking-tight mb-4">First run — Simple setup</h2>

    {#if step === "detect"}
      <div class="flex flex-col items-center gap-3 py-10">
        {#if detectError}
          <p class="text-sm text-rose-300">{status}</p>
          <button
            onclick={detect}
            class="px-4 py-2 rounded-xl bg-ember-600 text-ink-950 text-sm font-medium transition-colors"
          >
            Retry
          </button>
        {:else}
          <div class="w-12 h-12 rounded-full border-4 border-ink-700 border-t-ember-500 animate-spin"></div>
          <p class="text-sm text-ink-400">{status}</p>
        {/if}
      </div>
    {:else if step === "preset"}
      <div class="grid grid-cols-3 gap-2 text-sm mb-6">
        <div class="px-3 py-2 rounded-xl border border-ink-800 bg-ink-950">
          <span class="block text-xs text-ink-400">Total</span>
          <span class="font-mono text-ink-200">{formatBytes(ramTotal)}</span>
        </div>
        <div class="px-3 py-2 rounded-xl border border-ink-800 bg-ink-950">
          <span class="block text-xs text-ink-400">Usable</span>
          <span class="font-mono text-ink-200">{formatBytes(ramUsable)}</span>
        </div>
        <div class="px-3 py-2 rounded-xl border border-ember-500/30 bg-ember-500/10">
          <span class="block text-xs text-ember-300">70% budget</span>
          <span class="font-mono text-ember-200">{formatBytes(budget)}</span>
        </div>
      </div>
      <p class="text-sm text-ink-400 mb-4">
        {#if presetError}
          Presets could not be loaded; retry detection.
        {:else}
          Presets that fit your budget:
        {/if}
      </p>
      <div class="space-y-2">
        {#each presets as p}
          {@const fits = p.ramGb <= budgetGb}
          {@const isRec = p.id === recommended.id}
          <button onclick={() => fits && select(p)} class={`w-full text-left px-4 py-3 rounded-xl border transition ${fits ? 'border-ink-800 hover:border-ember-500/50 hover:bg-white/[0.02]' : 'border-ink-900 bg-ink-950/50 opacity-50 cursor-not-allowed'} ${isRec && fits ? 'ring-1 ring-ember-500/40' : ''}`}>
            <div class="flex items-center justify-between">
              <div>
                <div class="font-medium text-ink-100 flex items-center gap-2">
                  {p.name}
                  {#if isRec && fits}
                    <span class="text-xs px-1.5 py-0.5 rounded bg-ember-500/20 text-ember-300 border border-ember-500/30">Recommended</span>
                  {/if}
                </div>
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
    {:else if step === "confirm"}
      <div class="bg-ink-950 rounded-xl p-4 mb-6 border border-ink-800">
        <div class="font-medium text-ink-100">{selected?.name} preset</div>
        <div class="text-sm text-ink-400">{selected?.models.join(", ")}</div>
      </div>
      <p class="text-sm text-ink-400 mb-6">{status}</p>
      <div class="flex gap-3">
        <button onclick={() => step = "preset"} class="px-4 py-2 rounded-xl bg-white/5 border border-ink-800 text-sm text-ink-300 hover:bg-white/10">Change preset</button>
        <button onclick={install} class="px-4 py-2 rounded-xl bg-ember-600 text-ink-950 text-sm font-medium">Install</button>
      </div>
    {:else if step === "download"}
      <p class="text-sm text-ink-400 mb-4">{status}</p>
      <div class="w-full bg-ink-800 rounded-full h-2 mb-3"><div class="bg-ember-500 h-2 rounded-full transition-all duration-300" style="width:{progress}%"></div></div>
      <div class="text-xs font-mono text-ink-400 mb-4">{progress}%</div>
      <button
        onclick={cancelInstall}
        class="px-4 py-2 rounded-xl border border-rose-500/30 text-rose-400 hover:bg-rose-500/10 text-sm transition-colors"
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
      <button onclick={() => (onFinished ? onFinished() : location.reload())} class="px-4 py-2 rounded-xl bg-ember-600 text-ink-950 text-sm font-medium">Open app</button>
    {/if}
  </div>
</div>

<style>
  .animate-spin { animation: spin 1s linear infinite; }
  @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
</style>
