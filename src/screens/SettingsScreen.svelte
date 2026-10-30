<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  let ramBudget = $state(70);
  let cloudLlm = $state(false);
  let modelPriority = $state("local");
  let saving = $state(false);
  let saved = $state(false);

  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  onMount(async () => {
    try {
      const cfg = await invoke<any>("config_load");
      ramBudget = cfg.ram_budget_mb ? Math.round((cfg.ram_budget_mb / 1024) / 0.7) : 70;
      cloudLlm = !!cfg.cloud_llm_enabled;
      modelPriority = cfg.selected_models?.length ? "local" : modelPriority;
    } catch {}
  });

  $effect(() => {
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(async () => {
      saving = true;
      try {
        const current = await invoke<any>("config_load");
        const cfg = {
          ram_budget_mb: Math.round(ramBudget * 0.7 * 1024),
          cloud_llm_enabled: cloudLlm,
          selected_models: current.selected_models ?? [],
          presets: current.presets ?? []
        };
        await invoke("config_save", { cfg });
        saved = true;
        setTimeout(() => saved = false, 1500);
      } finally {
        saving = false;
      }
    }, 500);
  });
</script>

<div class="max-w-2xl mx-auto">
  <div class="border-b border-zinc-800 pb-4 mb-8">
    <h2 class="text-lg font-semibold tracking-tight">Settings</h2>
    <p class="text-xs text-zinc-500">RAM budget, model priority, cloud LLM</p>
  </div>

  <div class="space-y-6">
    <div class="border border-zinc-800 rounded-2xl p-5 bg-zinc-900/60">
      <div class="flex items-center justify-between mb-4">
        <div>
          <div class="font-medium text-zinc-100">RAM budget</div>
          <div class="text-xs text-zinc-500">Share of usable RAM reserved for models</div>
        </div>
        <span class="text-sm font-mono font-semibold text-fuchsia-300">{ramBudget}%</span>
      </div>
      <input type="range" min={20} max={90} bind:value={ramBudget} class="w-full accent-fuchsia-600"/>
      <div class="mt-3 h-2 rounded-full bg-zinc-800 overflow-hidden">
        <div class="h-full bg-gradient-to-r from-fuchsia-600 to-cyan-500 transition-all" style={`width:${ramBudget}%`}></div>
      </div>
      <div class="mt-2 text-[11px] text-zinc-500 font-mono">≈ {(ramBudget * 0.7).toFixed(0)}% of total usable RAM</div>
    </div>

    <div class="border border-zinc-800 rounded-2xl p-5 bg-zinc-900/60">
      <div class="flex items-center justify-between mb-3">
        <div class="font-medium text-zinc-100">Cloud LLM</div>
        <label class="relative inline-flex items-center cursor-pointer">
          <input type="checkbox" bind:checked={cloudLlm} class="sr-only peer"/>
          <div class="w-11 h-6 rounded-full bg-zinc-800 peer-checked:bg-fuchsia-600 after:absolute after:left-[2px] after:top-[2px] after:w-4 after:h-4 after:bg-white after:rounded-full after:transition after:peer-checked:translate-x-5"></div>
        </label>
      </div>
      <p class="text-xs text-zinc-500">Optional cloud LLMs via OpenRouter / NVIDIA NIM. Local-first by default.</p>
    </div>

    <div class="border border-zinc-800 rounded-2xl p-5 bg-zinc-900/60">
      <div class="font-medium text-zinc-100 mb-3">Model priority</div>
      <div class="space-y-2">
        <label class="flex items-center gap-3 cursor-pointer">
          <input type="radio" name="priority" bind:group={modelPriority} value="local" class="accent-fuchsia-600"/>
          <span class="text-sm text-zinc-300">Local models only (private, no telemetry)</span>
        </label>
        <label class="flex items-center gap-3 cursor-pointer">
          <input type="radio" name="priority" bind:group={modelPriority} value="cloud" class="accent-fuchsia-600"/>
          <span class="text-sm text-zinc-300">Prefer cloud when available</span>
        </label>
      </div>
    </div>

    <div class="pt-4 flex items-center gap-3">
      <span class="text-xs text-zinc-500">Live auto‑save</span>
      {#if saving}
        <span class="text-xs text-fuchsia-300">Saving…</span>
      {:else if saved}
        <span class="text-xs text-emerald-400">Saved</span>
      {/if}
    </div>
  </div>
</div>
