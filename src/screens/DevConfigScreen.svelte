<script lang="ts">
  import { api } from "../lib/api/client";
  import { onMount } from "svelte";

  let ramTotal = $state(16);
  let selected = $state<Set<string>>(new Set(["qwen3-embedding-0.6b-q8"]));
  let models = $state<{ id: string; name: string; ram: number; size: string }[]>([]);
  let totalRam = $state(0);
  let overBudget = $state(false);
  let loading = $state(true);
  let presetName = $state("");

  let presets = $state<any[]>([]);

  onMount(async () => {
    try {
      const list = await api.listModels();
      models = list.map(m => ({
        id: m.id,
        name: m.name,
        ram: Number(m.ram_estimate_gb),
        size: m.size_bytes ? `${(m.size_bytes/1024/1024).toFixed(0)} MB` : "?"
      }));
      const cfg = await api.loadConfig();
      ramTotal = cfg.ram_budget_mb ? Math.round(cfg.ram_budget_mb/1024) : ramTotal;
      if (cfg.selected_models?.length) selected = new Set(cfg.selected_models);
      recalc();
    } catch {}
    finally { loading = false; }
    try { presets = JSON.parse(localStorage.getItem("dev-presets") || "[]"); } catch {}
  });

  async function recalc() {
    if (models.length === 0) return;
    const ids = Array.from(selected);
    try {
      const res = await api.computeRam(ids);
      totalRam = res.total_ram_gb;
    } catch {
      totalRam = Array.from(selected).reduce((sum, id) => sum + (models.find(m=>m.id===id)?.ram||0), 0);
    }
    overBudget = totalRam > ramTotal * 0.7;
  }

  $effect(() => { recalc(); });

  const savePreset = () => {
    if (!presetName.trim()) return;
    const data = { ramTotal, selected: Array.from(selected) };
    const list = [...presets, { name: presetName.trim(), data }];
    localStorage.setItem("dev-presets", JSON.stringify(list));
    presets = list;
    presetName = "";
  };

  const loadPreset = (p: { name: string; data: { ramTotal: number; selected: string[] } }) => {
    ramTotal = p.data.ramTotal;
    selected = new Set(p.data.selected);
  };
</script>

<div class="max-w-6xl mx-auto space-y-6">
  <div>
    <h2 class="text-lg font-semibold tracking-tight">Dev configurator</h2>
    <p class="text-xs text-ink-400">Manual model picker with live RAM calculator</p>
  </div>

  <div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
    <div class="space-y-4">
      <div class="border border-ink-800 rounded-2xl p-5 bg-ink-900/60">
        <div class="flex items-center justify-between mb-4">
          <div class="font-medium text-ink-100">Available RAM</div>
          <div class="flex items-center gap-2">
            <input bind:value={ramTotal} type="number" class="w-24 bg-ink-950 border border-ink-700 rounded-lg px-2 py-1 text-sm font-mono text-ink-200 focus:outline-none focus:border-ember-500"/>
            <span class="text-xs text-ink-400">GB</span>
          </div>
        </div>
        <div class="text-xs text-ink-400 mb-3">Budget = 70% of usable RAM</div>
        <div class="w-full bg-ink-800 rounded-full h-2 mb-3">
          <div class={`h-2 rounded-full transition-all ${!overBudget ? 'bg-emerald-500' : 'bg-red-500'}`} style="width: {Math.min(100, (totalRam / (ramTotal * 0.7)) * 100)}%"></div>
        </div>
        <div class="text-sm font-mono text-ink-300">
          Selected: {totalRam.toFixed(2)} GB · budget: {(ramTotal * 0.7).toFixed(2)} GB
          <span class={`${overBudget ? 'text-red-400' : 'text-emerald-400'}`}> · {overBudget ? 'over budget' : 'fits'}</span>
        </div>
      </div>

      <div class="border border-ink-800 rounded-2xl bg-ink-900/60 divide-y divide-ink-800 overflow-hidden">
        {#each models as m}
          <label class="flex items-center gap-3 p-4 hover:bg-white/[0.02] cursor-pointer transition">
            <input type="checkbox" bind:group={selected} value={m.id} class="accent-ember-600"/>
            <div class="flex-1">
              <div class="text-sm font-medium text-ink-100">{m.name}</div>
              <div class="text-xs text-ink-400 font-mono">{m.size} · ~{m.ram.toFixed(2)} GB RAM</div>
            </div>
          </label>
        {/each}
      </div>
    </div>

    <div class="space-y-4">
      <div class="border border-ink-800 rounded-2xl p-5 bg-ink-900/60">
        <div class="font-medium text-ink-100 mb-2">Preset actions</div>
        <div class="flex gap-2 mb-3">
          <input bind:value={presetName} placeholder="Preset name" class="flex-1 bg-ink-950 border border-ink-800 rounded-lg px-2 py-1.5 text-xs text-ink-200 focus:outline-none focus:border-ember-500"/>
          <button onclick={savePreset} class="px-3 py-1.5 rounded-lg bg-white/5 border border-ink-800 text-xs text-ink-300 hover:bg-white/10">Save</button>
        </div>
        <div class="space-y-1 max-h-32 overflow-auto">
          {#each presets as p}
            <button onclick={() => loadPreset(p)} class="w-full text-left px-2 py-1 rounded bg-white/5 border border-ink-800 text-xs text-ink-300 hover:bg-white/10">{p.name}</button>
          {:else}
            <div class="text-xs text-ink-400">No saved presets</div>
          {/each}
        </div>
      </div>
    </div>
  </div>
</div>
