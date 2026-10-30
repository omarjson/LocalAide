<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { formatBytes } from "../lib/format";

  type Step = "detect" | "preset" | "confirm" | "download" | "done";

  type Preset = {
    id: string;
    name: string;
    ram: string;
    fits: string;
    models: string[];
  };

  const presets: Preset[] = [
    { id: "tiny", name: "Tiny", ram: "~1.9 GB", ramGb: 1.9, fits: "4–8 GB total RAM", models: ["EmbeddingGemma 2 multimodal", "Qwen3.5-0.8B Q4", "Whisper Tiny", "Piper small"] },
    { id: "small", name: "Small", ram: "~2.7 GB", ramGb: 2.7, fits: "8–12 GB total RAM", models: ["EmbeddingGemma 2 multimodal", "Qwen3.5-2B Q4", "Whisper Base"] },
    { id: "balanced", name: "Balanced", ram: "~4.3 GB", ramGb: 4.3, fits: "8–12 GB total RAM", models: ["EmbeddingGemma 2 multimodal", "Qwen3.5-4B Q4", "Whisper Base", "Piper"] },
    { id: "advanced", name: "Advanced", ram: "~9.5 GB", ramGb: 9.5, fits: "12 GB+ total RAM", models: ["Qwen3.5-9B Q4", "EmbeddingGemma 2 multimodal", "Vision SLM"] },
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
      const probe = await invoke("system_probe") as any;
      ramTotal = probe.total_ram;
      ramUsable = probe.usable_ram;
      budget = probe.budget_ram;
      step = "preset";
      status = `Recommended: ${recommended.name} preset fits your ${budgetGb.toFixed(1)} GB budget.`;
    } catch (e) {
      status = "System probe unavailable: " + String(e);
    }
  };

  const select = (p: Preset) => { selected = p; step = "confirm"; status = `Install ${p.name} preset (~${p.ram})?`; };
  const install = async () => {
    if (!selected) return;
    step = "download"; status = `Downloading ${selected.name} models...`;
    try {
      const unsub = await listen("model_download_progress", (event: any) => {
        const p = event.payload;
        progress = Math.round(p?.percent ?? 0);
      });
      await invoke("model_download_preset", { id: selected.id });
      await unsub();
      step = "done"; status = `${selected.name} preset installed. Health check passed.`;
    } catch (e) {
      status = `Install failed: ${String(e)}`;
      step = "confirm";
    }
  };

  onMount(detect);
</script>

<div class="max-w-2xl mx-auto">
  <div class="rounded-2xl border border-zinc-800 bg-zinc-900/60 p-6 shadow-[0_0_80px_rgba(217,70,239,0.08)]">
    <h2 class="text-xl font-semibold tracking-tight mb-4">First run — Simple setup</h2>

    {#if step === "detect"}
      <div class="flex flex-col items-center gap-3 py-10">
        <div class="w-12 h-12 rounded-full border-4 border-zinc-700 border-t-fuchsia-500 animate-spin"></div>
        <p class="text-sm text-zinc-400">{status}</p>
      </div>
    {:else if step === "preset"}
      <div class="grid grid-cols-3 gap-2 text-sm mb-6">
        <div class="px-3 py-2 rounded-xl border border-zinc-800 bg-zinc-950">
          <span class="block text-xs text-zinc-500">Total</span>
          <span class="font-mono text-zinc-200">{formatBytes(ramTotal)}</span>
        </div>
        <div class="px-3 py-2 rounded-xl border border-zinc-800 bg-zinc-950">
          <span class="block text-xs text-zinc-500">Usable</span>
          <span class="font-mono text-zinc-200">{formatBytes(ramUsable)}</span>
        </div>
        <div class="px-3 py-2 rounded-xl border border-fuchsia-500/30 bg-fuchsia-500/10">
          <span class="block text-xs text-fuchsia-300">70% budget</span>
          <span class="font-mono text-fuchsia-200">{formatBytes(budget)}</span>
        </div>
      </div>
      <p class="text-sm text-zinc-400 mb-4">Presets that fit your budget:</p>
      <div class="space-y-2">
        {#each presets as p}
          {@const fits = p.ramGb <= budgetGb}
          {@const isRec = p.id === recommended.id}
          <button onclick={() => fits && select(p)} class={`w-full text-left px-4 py-3 rounded-xl border transition ${fits ? 'border-zinc-800 hover:border-fuchsia-500/50 hover:bg-white/[0.02]' : 'border-zinc-900 bg-zinc-950/50 opacity-50 cursor-not-allowed'} ${isRec && fits ? 'ring-1 ring-fuchsia-500/40' : ''}`}>
            <div class="flex items-center justify-between">
              <div>
                <div class="font-medium text-zinc-100 flex items-center gap-2">
                  {p.name}
                  {#if isRec && fits}
                    <span class="text-[10px] px-1.5 py-0.5 rounded bg-fuchsia-500/20 text-fuchsia-300 border border-fuchsia-500/30">Recommended</span>
                  {/if}
                </div>
                <div class="text-xs text-zinc-500">{p.ram} · {p.fits} · {fits ? 'Fits' : 'Too large'}</div>
              </div>
              <div class="text-xs text-zinc-500">{fits ? '→' : '✕'}</div>
            </div>
            <ul class="mt-2 text-xs text-zinc-500 list-disc list-inside">
              {#each p.models as m}<li>{m}</li>{/each}
            </ul>
          </button>
        {/each}
      </div>
    {:else if step === "confirm"}
      <div class="bg-zinc-950 rounded-xl p-4 mb-6 border border-zinc-800">
        <div class="font-medium text-zinc-100">{selected?.name} preset</div>
        <div class="text-sm text-zinc-400">{selected?.models.join(", ")}</div>
      </div>
      <p class="text-sm text-zinc-400 mb-6">{status}</p>
      <div class="flex gap-3">
        <button onclick={() => step = "preset"} class="px-4 py-2 rounded-xl bg-white/5 border border-zinc-800 text-sm text-zinc-300 hover:bg-white/10">Change preset</button>
        <button onclick={install} class="px-4 py-2 rounded-xl bg-gradient-to-r from-fuchsia-600 to-cyan-500 text-white text-sm font-medium">Install</button>
      </div>
    {:else if step === "download"}
      <p class="text-sm text-zinc-400 mb-4">{status}</p>
      <div class="w-full bg-zinc-800 rounded-full h-2 mb-3"><div class="bg-fuchsia-500 h-2 rounded-full transition-all" style="width:{progress}%"></div></div>
      <div class="text-xs font-mono text-zinc-500">{progress}%</div>
    {:else if step === "done"}
      <div class="flex items-start gap-3 mb-6">
        <span class="size-6 grid place-items-center rounded-full bg-emerald-500/20 text-emerald-300">✓</span>
        <div>
          <div class="font-medium text-zinc-100">Setup complete</div>
          <div class="text-sm text-zinc-400">{status}</div>
        </div>
      </div>
      <button onclick={() => location.reload()} class="px-4 py-2 rounded-xl bg-gradient-to-r from-fuchsia-600 to-cyan-500 text-white text-sm font-medium">Open app</button>
    {/if}
  </div>
</div>

<style>
  .animate-spin { animation: spin 1s linear infinite; }
  @keyframes spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }
</style>
