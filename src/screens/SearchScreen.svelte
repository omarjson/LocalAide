<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  let query = $state("");
  let mode = $state<"semantic" | "keyword" | "hybrid">("hybrid");
  let results = $state<any[]>([]);
  let stats = $state<any>(null);
  let folder = $state("");
  let watching = $state(false);
  let loading = $state(false);

  onMount(async () => {
    try {
      stats = await invoke("indexer_stats");
    } catch {}
  });

  const search = async () => {
    if (!query.trim()) return;
    loading = true;
    try {
      results = await invoke("search_index", { query, mode, limit: 50 });
    } catch (e) {
      console.error(e);
    } finally {
      loading = false;
    }
  };

  const indexFolder = async () => {
    if (!folder.trim()) return;
    loading = true;
    try {
      const res = await invoke("index_folder", { folder });
      await refreshStats();
      query = "";
      results = [];
    } catch (e) {
      console.error(e);
    } finally {
      loading = false;
    }
  };

  const refreshStats = async () => {
    try { stats = await invoke("indexer_stats"); } catch {}
  };

  const startWatch = async () => {
    if (!folder.trim()) return;
    try {
      await invoke("index_watch_start", { folder });
      watching = true;
    } catch (e) {
      console.error(e);
    }
  };

  const removeDoc = async (path: string) => {
    try {
      await invoke("index_remove", { path });
      results = results.filter(r => r.doc.path !== path);
      await refreshStats();
    } catch (e) {
      console.error(e);
    }
  };

  $effect(() => {
    if (query.length > 2) search();
  });
</script>

<div class="h-full flex flex-col p-6 max-w-6xl mx-auto">
  <div class="flex items-center gap-3 mb-4">
    <input bind:value={query} placeholder="Search docs (hybrid semantic + keyword)…"
      class="flex-1 bg-zinc-950 border border-zinc-800 rounded-xl px-4 py-2.5 text-sm text-zinc-100 placeholder-zinc-500 focus:outline-none focus:border-fuchsia-500"/>
    <select bind:value={mode} class="bg-zinc-900 border border-zinc-800 rounded-lg px-2 py-2 text-sm text-zinc-200">
      <option value="hybrid">Hybrid</option>
      <option value="semantic">Semantic</option>
      <option value="keyword">Keyword</option>
    </select>
    <button onclick={search} disabled={loading} class="px-4 py-2 rounded-xl bg-gradient-to-r from-fuchsia-600 to-cyan-500 text-white text-sm disabled:opacity-40">Search</button>
  </div>

  <div class="grid md:grid-cols-3 gap-6 flex-1 min-h-0">
    <div class="md:col-span-2 flex flex-col min-h-0">
      <div class="flex items-center justify-between mb-2">
        <div class="text-xs uppercase tracking-widest text-zinc-500">Results</div>
        <div class="text-xs text-zinc-600">{results.length} hits</div>
      </div>
      <div class="flex-1 overflow-y-auto space-y-3 pr-2">
        {#each results as r}
          <div class="bg-zinc-950 border border-zinc-800 rounded-2xl p-4">
            <div class="flex items-start justify-between gap-3">
              <div>
                <div class="text-sm font-medium text-zinc-100">{r.doc.title}</div>
                <div class="text-xs text-zinc-500">{r.doc.path}</div>
              </div>
              <div class="flex items-center gap-2">
                <span class="text-[10px] px-2 py-0.5 rounded-full bg-white/5 border border-zinc-800 text-zinc-400">{r.source}</span>
                <span class="text-[10px] px-2 py-0.5 rounded-full bg-white/5 border border-zinc-800 text-zinc-400">{(r.score ?? 0).toFixed(3)}</span>
                <button onclick={() => removeDoc(r.doc.path)} class="text-[11px] px-2 py-1 rounded-md bg-white/5 border border-zinc-800 text-zinc-400 hover:text-zinc-200">Remove</button>
              </div>
            </div>
            <div class="mt-2 text-xs text-zinc-300 whitespace-pre-wrap line-clamp-6">{r.doc.content.slice(0, 600)}</div>
          </div>
        {/each}
        {#if !loading && results.length === 0}
          <div class="text-sm text-zinc-600">No results. Try a query or index a folder.</div>
        {/if}
      </div>
    </div>

    <div class="flex flex-col gap-4">
      <div class="bg-zinc-950 border border-zinc-800 rounded-2xl p-4">
        <div class="text-xs uppercase tracking-widest text-zinc-500 mb-2">Index</div>
        {#if stats}
          <div class="text-sm text-zinc-300 space-y-1">
            <div>Docs: <span class="text-zinc-100">{stats.doc_count}</span></div>
            <div>Path: <span class="text-zinc-500 text-xs break-all">{stats.storage_path}</span></div>
          </div>
        {/if}
        <button onclick={refreshStats} class="mt-3 text-xs px-2 py-1 rounded-md bg-white/5 border border-zinc-800 text-zinc-400">Refresh</button>
      </div>

      <div class="bg-zinc-950 border border-zinc-800 rounded-2xl p-4">
        <div class="text-xs uppercase tracking-widest text-zinc-500 mb-2">Indexer</div>
        <input bind:value={folder} placeholder="/path/to/index" class="w-full bg-zinc-900 border border-zinc-800 rounded-xl px-3 py-2 text-xs text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-fuchsia-500 mb-2"/>
        <div class="flex gap-2">
          <button onclick={indexFolder} disabled={loading} class="flex-1 px-3 py-2 rounded-xl bg-white/5 border border-zinc-800 text-xs text-zinc-300">Index folder</button>
          <button onclick={startWatch} disabled={watching} class="flex-1 px-3 py-2 rounded-xl bg-white/5 border border-zinc-800 text-xs text-zinc-300 disabled:opacity-50">{watching ? "Watching" : "Watch"}</button>
        </div>
      </div>
    </div>
  </div>
</div>
