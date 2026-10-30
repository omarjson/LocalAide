<script lang="ts">
  import { api } from "../lib/api/client";
  import type { IndexerStatsResponse, SearchResultDoc } from "../lib/api/bindings";
  import { onMount } from "svelte";

  let query = $state("");
  let mode = $state<"semantic" | "keyword" | "hybrid">("hybrid");
  let results = $state<SearchResultDoc[]>([]);
  let stats = $state<IndexerStatsResponse | null>(null);
  let folder = $state("");
  let watching = $state(false);
  let loading = $state(false);
  let searchDurationMs = $state<number | null>(null);
  let errorMessage = $state<string | null>(null);
  let searchInputEl = $state<HTMLInputElement | null>(null);

  onMount(async () => {
    await refreshStats();
    await loadInitialLibrary();
    searchInputEl?.focus();
  });

  const loadInitialLibrary = async () => {
    try {
      const docs = await api.listIndex();
      if (!query.trim()) {
        results = docs;
      }
    } catch (e) {
      console.error(e);
    }
  };

  const search = async () => {
    if (!query.trim()) {
      await loadInitialLibrary();
      searchDurationMs = null;
      return;
    }
    loading = true;
    errorMessage = null;
    const start = performance.now();
    try {
      results = await api.searchIndex({ query, mode, limit: 50 });
      searchDurationMs = Math.round(performance.now() - start);
    } catch (e: any) {
      errorMessage = e?.message || "Search failed";
    } finally {
      loading = false;
    }
  };

  const indexFolder = async () => {
    if (!folder.trim()) return;
    loading = true;
    errorMessage = null;
    try {
      const resp = await api.indexFolder(folder);
      await refreshStats();
      query = "";
      await loadInitialLibrary();
    } catch (e: any) {
      errorMessage = e?.message || "Folder indexing failed";
    } finally {
      loading = false;
    }
  };

  const refreshStats = async () => {
    try {
      stats = await api.getIndexerStats();
    } catch (e) {
      console.error(e);
    }
  };

  const startWatch = async () => {
    if (!folder.trim()) return;
    errorMessage = null;
    try {
      await api.watchFolder(folder);
      watching = true;
    } catch (e: any) {
      errorMessage = e?.message || "File watch failed";
    }
  };

  const removeDoc = async (path: string) => {
    try {
      await api.removeIndexedDoc(path);
      results = results.filter(r => r.path !== path);
      await refreshStats();
    } catch (e: any) {
      errorMessage = e?.message || "Failed to remove document";
    }
  };

  const handleKeydown = (e: KeyboardEvent) => {
    if (e.key === "Enter") {
      search();
    } else if (e.key === "Escape") {
      query = "";
      loadInitialLibrary();
    }
  };

  $effect(() => {
    if (query.length > 2) {
      const timer = setTimeout(() => search(), 150);
      return () => clearTimeout(timer);
    } else if (query.length === 0) {
      loadInitialLibrary();
      searchDurationMs = null;
    }
  });
</script>

<div class="max-w-4xl mx-auto space-y-6">
  <div class="flex items-center justify-between">
    <div>
      <h2 class="text-lg font-semibold tracking-tight text-zinc-100">Document Library & Retrieval</h2>
      <p class="text-xs text-zinc-400">Embedded SQLite WAL storage with BM25 keyword matching and vector rank fusion.</p>
    </div>
    {#if stats}
      <div class="flex items-center gap-4 text-xs font-mono text-zinc-300 bg-zinc-900 border border-zinc-800 rounded-lg px-3 py-1.5">
        <span>Docs: {stats.doc_count}</span>
        <span>Size: {(stats.total_bytes / 1024).toFixed(1)} KB</span>
      </div>
    {/if}
  </div>

  {#if errorMessage}
    <div class="p-3 bg-rose-500/10 border border-rose-500/20 rounded-xl text-xs text-rose-300 flex justify-between items-center">
      <span>{errorMessage}</span>
      <button onclick={() => errorMessage = null} class="text-rose-400 hover:text-rose-200">✕</button>
    </div>
  {/if}

  <div class="border border-zinc-800 rounded-2xl p-4 bg-zinc-900/40 space-y-3">
    <div class="flex items-center justify-between">
      <div class="text-xs font-semibold text-zinc-300">Index Local Directory</div>
      {#if watching}
        <span class="inline-flex items-center gap-1.5 text-[11px] font-mono text-emerald-400">
          <span class="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
          Watching directory
        </span>
      {/if}
    </div>
    <div class="flex gap-2">
      <input
        type="text"
        placeholder="/Users/username/Documents/notes"
        bind:value={folder}
        class="flex-1 bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-xs text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-zinc-700 font-mono"
      />
      <button onclick={indexFolder} disabled={loading || !folder.trim()} class="px-4 py-2 bg-fuchsia-600 hover:bg-fuchsia-500 disabled:opacity-40 rounded-lg text-xs font-medium text-white transition-colors">
        {loading ? "Indexing…" : "Index Folder"}
      </button>
      <button onclick={startWatch} disabled={watching || !folder.trim()} class="px-3 py-2 bg-zinc-800 hover:bg-zinc-700 disabled:opacity-40 rounded-lg text-xs font-medium text-zinc-300 transition-colors">
        {watching ? "Watching" : "Watch"}
      </button>
    </div>
  </div>

  <div class="space-y-3">
    <div class="flex gap-2">
      <div class="relative flex-1">
        <input
          bind:this={searchInputEl}
          type="text"
          aria-label="Search documents"
          placeholder="Search documents with BM25 or semantic fusion (Enter to search, Esc to clear)…"
          bind:value={query}
          onkeydown={handleKeydown}
          class="w-full bg-zinc-900 border border-zinc-800 rounded-lg pl-3 pr-8 py-2 text-sm text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-zinc-700 font-mono"
        />
        {#if query}
          <button
            onclick={() => { query = ""; loadInitialLibrary(); }}
            class="absolute right-2.5 top-1/2 -translate-y-1/2 text-xs text-zinc-500 hover:text-zinc-300"
          >
            ✕
          </button>
        {/if}
      </div>
      <select bind:value={mode} class="bg-zinc-900 border border-zinc-800 rounded-lg px-3 py-2 text-xs text-zinc-300 font-mono focus:outline-none focus:border-zinc-700">
        <option value="hybrid">Hybrid (RRF)</option>
        <option value="semantic">Semantic</option>
        <option value="keyword">Keyword (BM25)</option>
      </select>
      <button onclick={search} disabled={loading} class="px-4 py-2 bg-zinc-800 hover:bg-zinc-700 rounded-lg text-xs font-medium text-zinc-200 transition-colors">
        Search
      </button>
    </div>

    <div class="flex items-center justify-between text-[11px] font-mono text-zinc-500 px-1">
      <span>{results.length} {results.length === 1 ? "document" : "documents"} {query ? "found" : "in library"}</span>
      {#if searchDurationMs !== null}
        <span class="text-emerald-400">Query completed in {searchDurationMs}ms</span>
      {/if}
    </div>

    {#if loading}
      <div class="py-12 text-center text-xs text-zinc-500 font-mono">Searching library index…</div>
    {:else if results.length === 0}
      <div class="py-12 text-center text-xs text-zinc-500 font-mono">
        {query ? "No matching documents found in library." : "Library is empty. Index a folder above to get started."}
      </div>
    {:else}
      <div class="space-y-2">
        {#each results as doc}
          <div class="border border-zinc-800 rounded-xl p-3.5 bg-zinc-900/30 hover:border-zinc-700 transition-colors flex justify-between items-start gap-4">
            <div class="space-y-1.5 flex-1 min-w-0">
              <div class="font-medium text-sm text-zinc-200 truncate">{doc.title}</div>
              <p class="text-xs text-zinc-400 line-clamp-2 font-mono leading-relaxed">{doc.snippet}</p>
              <div class="flex flex-wrap items-center gap-3 text-[10px] font-mono text-zinc-500">
                <span>Score: {doc.score.toFixed(3)}</span>
                <span class="truncate">Path: {doc.path}</span>
                <span>{(doc.size / 1024).toFixed(1)} KB</span>
              </div>
            </div>
            <button
              onclick={() => removeDoc(doc.path)}
              class="text-xs text-zinc-500 hover:text-rose-400 px-2.5 py-1 rounded bg-zinc-900 hover:bg-zinc-800 transition-colors flex-shrink-0"
              title="Remove from index"
            >
              Remove
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>
