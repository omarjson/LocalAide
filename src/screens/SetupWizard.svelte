<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import { onMount } from "svelte";
  import { api } from "../lib/api/client";
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

  const presets: Preset[] = [
    { id: "tiny", name: "Tiny", ram: "~1.0 GB RAM", ramGb: 1.0, fits: "4 GB+ total RAM", models: ["Qwen3.5 0.8B"] },
    { id: "small", name: "Small", ram: "~4 GB RAM", ramGb: 4.0, fits: "8 GB+ total RAM", models: ["Qwen3.5 2B", "Gemma 4 E2B (QAT)"] },
    { id: "balanced", name: "Balanced", ram: "~9.5 GB RAM", ramGb: 9.5, fits: "16 GB+ total RAM", models: ["Qwen3.5 9B", "Gemma 4 E4B (QAT)"] },
    { id: "advanced", name: "Advanced", ram: "~20 GB RAM", ramGb: 20.0, fits: "32 GB total RAM", models: ["Gemma 4 26B-A4B (MoE)", "Qwen3.8 27B (vision)"] },
  ];

  let step = $state<Step>("detect");
  let ramTotal = $state(0);
  let ramUsable = $state(0);
  let budget = $state(0);
  let selected = $state<Preset | null>(null);
  let progress = $state(0);
  let status = $state("Detecting system memory...");

  const budgetGb = $derived(budget / 1024 / 1024 / 1024);
  const fittingPresets = $derived(presets.filter(p => p.ramGb <= budgetGb));
  const recommended = $derived(fittingPresets[fittingPresets.length - 1] ?? presets[0]);

  const detect = async () => {
    try {
      const probe = await api.systemProbe();
      ramTotal = probe.total_ram_gb * 1024 * 1024 * 1024;
      ramUsable = probe.available_ram_gb * 1024 * 1024 * 1024;
      budget = probe.budget_gb * 1024 * 1024 * 1024;
      step = "preset";
      status = `Recommended: ${recommended.name} preset fits your ${probe.budget_gb.toFixed(1)} GB budget.`;
    } catch (e: any) {
      status = "System probe unavailable: " + (e?.message ?? String(e));
    }
  };

  const select = (p: Preset) => { selected = p; step = "confirm"; status = `Install ${p.name} preset (~${p.ram})?`; };
  const install = async () => {
    if (!selected) return;
    step = "download"; status = `Downloading ${selected.name} models...`;
    try {
      await api.downloadModelPreset(selected.id);
      step = "done"; status = `${selected.name} preset installed.`;
    } catch (e: any) {
      status = `Install failed: ${e?.message ?? String(e)}`;
      step = "confirm";
    }
  };

  onMount(detect);
</script>

<div class="max-w-2xl mx-auto">
  <div class="rounded-2xl border border-ink-800 bg-ink-900/60 p-6 shadow-[0_0_80px_rgba(217,70,239,0.08)]">
    <h2 class="text-xl font-semibold tracking-tight mb-4">First run — Simple setup</h2>

    {#if step === "detect"}
      <div class="flex flex-col items-center gap-3 py-10">
        <div class="w-12 h-12 rounded-full border-4 border-ink-700 border-t-ember-500 animate-spin"></div>
        <p class="text-sm text-ink-400">{status}</p>
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
      <p class="text-sm text-ink-400 mb-4">Presets that fit your budget:</p>
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
      <div class="w-full bg-ink-800 rounded-full h-2 mb-3"><div class="bg-ember-500 h-2 rounded-full transition-all" style="width:{progress}%"></div></div>
      <div class="text-xs font-mono text-ink-400">{progress}%</div>
    {:else if step === "done"}
      <div class="flex items-start gap-3 mb-6">
        <span class="size-6 grid place-items-center rounded-full bg-emerald-500/20 text-emerald-300">✓</span>
        <div>
          <div class="font-medium text-ink-100">Setup complete</div>
          <div class="text-sm text-ink-400">{status}</div>
        </div>
      </div>
      <button onclick={() => location.reload()} class="px-4 py-2 rounded-xl bg-ember-600 text-ink-950 text-sm font-medium">Open app</button>
    {/if}
  </div>
</div>

<style>
  .animate-spin { animation: spin 1s linear infinite; }
  @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
</style>
