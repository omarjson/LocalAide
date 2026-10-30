<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";

  type Model = {
    id: string;
    name: string;
    size: string;
    ram: string;
    quant: string;
    status: "installed" | "available" | "installing";
    progress?: number;
    active?: boolean;
    kind?: string;
  };

  let models = $state<Model[]>([]);
  let loading = $state(true);
  let query = $state("");
  let presets = $state<any[]>([]);
  let activePreset = $state<string | null>(null);
  let presetInstalling = $state(false);

  let listeners: Array<() => void> = [];

  const refresh = async () => {
    try {
      const list = await invoke<any[]>("model_list");
      models = list.map(m => ({
        id: m.id,
        name: m.name,
        size: m.file_size ? `${(m.file_size/1024/1024).toFixed(0)} MB` : "?",
        ram: `${m.ram_estimate_gb} GB`,
        status: m.installed ? "installed" : "available",
        quant: m.quant ?? "—",
        active: m.active ?? false,
        kind: m.kind ?? "llm"
      }));
    } catch {
      models = [];
    } finally {
      loading = false;
    }
  };

  onMount(async () => {
    await refresh();
    try {
      presets = await invoke<any[]>("model_presets");
    } catch {}

    // Live download progress straight from the backend.
    listeners.push(await listen<any>("model_download_progress", (event) => {
      const p = event.payload;
      if (!p?.id) return;
      models = models.map(m => m.id === p.id
        ? { ...m, status: "installing", progress: Math.round(p.percent ?? 0) }
        : m);
    }));

    listeners.push(await listen<any>("model_download_done", async (event) => {
      const p = event.payload;
      if (!p?.id) return;
      if (p.ok) {
        await refresh();
      } else {
        models = models.map(m => m.id === p.id ? { ...m, status: "available", progress: undefined } : m);
      }
    }));

    return () => listeners.forEach(u => u());
  });

  const startInstall = async (id: string) => {
    models = models.map(m => m.id === id ? { ...m, status: "installing", progress: 0 } : m);
    try {
      await invoke("model_download", { id });
    } catch (e) {
      models = models.map(m => m.id === id
        ? { ...m, status: "available", progress: undefined }
        : m);
      alert(String(e));
    }
  };

  const cancelInstall = async (id: string) => {
    try { await invoke("model_download_cancel", { id }); } catch {}
  };

  const installPreset = async (id: string) => {
    activePreset = id;
    presetInstalling = true;
    try {
      await invoke("model_download_preset", { id });
      await refresh();
    } catch {}
    presetInstalling = false;
    activePreset = null;
  };

  const activateModel = async (id: string) => {
    try {
      await invoke("model_activate", { id });
      models = models.map(m => ({ ...m, active: m.id === id }));
    } catch {}
  };

  const removeModel = async (id: string) => {
    if (!confirm("Remove model?")) return;
    try {
      await invoke("model_remove", { id });
      models = models.filter(m => m.id !== id);
    } catch {}
  };

  const filtered = $derived(models.filter(m => m.name.toLowerCase().includes(query.toLowerCase())));

  const kindLabel = (k?: string) => {
    switch (k) {
      case "llm": return "LLM";
      case "embedding": return "Embedding";
      case "stt": return "STT";
      case "tts": return "TTS";
      case "vision": return "Vision";
      default: return "—";
    }
  };
</script>

<div class="max-w-5xl mx-auto space-y-6">
  <div class="flex items-center justify-between">
    <div>
      <h2 class="text-lg font-semibold tracking-tight">Model manager</h2>
      <p class="text-xs text-zinc-500">Catalog · Hugging Face GGUF · SHA256 verified</p>
    </div>
    <button class="px-4 py-2 rounded-xl bg-white/5 border border-zinc-800 hover:bg-white/10 text-sm font-medium flex items-center gap-2">
      <span class="text-base">+</span> Add model
    </button>
  </div>

  {#if presets.length > 0}
    <div class="border border-zinc-800 rounded-2xl p-4 bg-zinc-900/60">
      <div class="text-sm font-medium text-zinc-100 mb-3">Quick install presets</div>
      <div class="grid grid-cols-2 md:grid-cols-4 gap-2">
        {#each presets as p}
          <button
            onclick={() => installPreset(p.id)}
            disabled={presetInstalling}
            class="text-left px-3 py-2 rounded-xl border border-zinc-800 hover:border-fuchsia-500/40 bg-zinc-950 disabled:opacity-50">
            <div class="text-xs font-medium text-zinc-100">{p.name}</div>
            <div class="text-[10px] text-zinc-500 mt-0.5">{p.total_ram_gb} GB · {p.fits_ram_gb}</div>
            {#if activePreset === p.id && presetInstalling}
              <div class="text-[10px] text-fuchsia-300 mt-1">Installing…</div>
            {/if}
          </button>
        {/each}
      </div>
    </div>
  {/if}

  <div class="flex items-center gap-3">
    <input bind:value={query} placeholder="Search models…" class="w-full max-w-sm bg-zinc-950 border border-zinc-800 rounded-xl px-4 py-2 text-sm text-zinc-100 placeholder-zinc-500 focus:outline-none focus:border-fuchsia-500"/>
    <div class="text-xs text-zinc-500 font-mono">{filtered.length} models</div>
  </div>

  <div class="border border-zinc-800 rounded-2xl overflow-hidden bg-zinc-900/60">
    {#if loading}
      <div class="py-16 text-center text-zinc-500">Loading catalog…</div>
    {:else}
      <div class="overflow-x-auto">
        <table class="w-full text-left">
          <thead class="border-b border-zinc-800 bg-zinc-900/80">
            <tr class="text-[11px] uppercase tracking-widest text-zinc-500">
              <th class="px-4 py-3 font-medium">Model</th>
              <th class="px-4 py-3 font-medium">Type</th>
              <th class="px-4 py-3 font-medium">File size</th>
              <th class="px-4 py-3 font-medium">RAM est.</th>
              <th class="px-4 py-3 font-medium">Quant</th>
              <th class="px-4 py-3 font-medium">Status</th>
              <th class="px-4 py-3 font-medium">Action</th>
            </tr>
          </thead>
          <tbody class="divide-y divide-zinc-800">
            {#each filtered as m, i}
              <tr class="hover:bg-white/[0.02] transition">
                <td class="px-4 py-3 text-sm text-zinc-100 font-medium">
                  {m.name}
                  {#if m.active}
                    <span class="ml-2 inline-flex items-center px-2 py-0.5 rounded-full text-[10px] bg-fuchsia-500/15 text-fuchsia-300 border border-fuchsia-500/25">active</span>
                  {/if}
                </td>
                <td class="px-4 py-3 text-sm text-zinc-400">{kindLabel(m.kind)}</td>
                <td class="px-4 py-3 text-sm text-zinc-400 font-mono">{m.size}</td>
                <td class="px-4 py-3 text-sm text-zinc-400 font-mono">{m.ram}</td>
                <td class="px-4 py-3 text-sm text-zinc-400">{m.quant}</td>
                <td class="px-4 py-3">
                  {#if m.status === "installing"}
                    <div class="w-32">
                      <div class="text-[11px] text-zinc-400 mb-1">Downloading… {m.progress ?? 0}%</div>
                      <div class="h-1.5 bg-zinc-800 rounded-full overflow-hidden">
                        <div class="h-full bg-fuchsia-500 transition-all" style="width:{m.progress ?? 0}%"></div>
                      </div>
                    </div>
                  {:else}
                    <span class={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs ${m.status === 'installed' ? 'bg-emerald-500/10 text-emerald-300 border border-emerald-500/20' : 'bg-zinc-800 text-zinc-400 border border-zinc-700'}`}>
                      <span class="w-1.5 h-1.5 rounded-full bg-current"></span>
                      {m.status === 'installed' ? 'Installed' : 'Available'}
                    </span>
                  {/if}
                </td>
                <td class="px-4 py-3">
                  <div class="flex items-center gap-2">
                    {#if m.status === 'installing'}
                      <button onclick={() => cancelInstall(m.id)} class="px-2.5 py-1 rounded-lg border border-red-500/30 text-[11px] text-red-300 hover:bg-red-500/10">Cancel</button>
                    {:else if m.status === 'installed'}
                      <button onclick={() => activateModel(m.id)} class="px-2.5 py-1 rounded-lg border text-[11px] {m.active ? 'bg-fuchsia-500/20 border-fuchsia-500/30 text-fuchsia-200' : 'border-zinc-700 text-zinc-300 hover:bg-white/5'}">{m.active ? 'Active' : 'Activate'}</button>
                      <button onclick={() => removeModel(m.id)} class="px-2.5 py-1 rounded-lg border border-zinc-700 text-[11px] text-zinc-300 hover:bg-white/5">Remove</button>
                    {:else}
                      <button onclick={() => startInstall(m.id)} class="px-3 py-1.5 rounded-lg border border-zinc-700 text-xs text-zinc-300 hover:bg-white/5">Download</button>
                    {/if}
                  </div>
                </td>
              </tr>
            {:else}
              <tr>
                <td colspan="7" class="px-4 py-12 text-center text-zinc-500">No models match your search</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>
</div>
