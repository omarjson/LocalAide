<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import ErrorBanner from "../components/ErrorBanner.svelte";
  import { api, errorMessage as errMsg } from "../lib/api/client";
  import type {
    EmbeddingStatusResponse,
    IndexerStatsResponse,
    SearchResultDoc,
  } from "../lib/api/bindings";
  import { onMount } from "svelte";

  let query = $state("");
  // Keyword is BM25-only. Semantic and hybrid rank with vectors from the
  // local embedding engine and quietly fall back to keyword when it is off.
  let mode = $state<"keyword" | "semantic" | "hybrid">("keyword");
  let results = $state<SearchResultDoc[]>([]);
  let stats = $state<IndexerStatsResponse | null>(null);
  let folder = $state("");
  let watching = $state(false);
  let loading = $state(false);
  let libraryLoaded = $state(false);
  let searchDurationMs = $state<number | null>(null);
  let errorMessage = $state<string | null>(null);
  let searchInputEl = $state<HTMLInputElement | null>(null);
  let singleFile = $state("");
  let embedding = $state<EmbeddingStatusResponse | null>(null);
  let embedBusy = $state(false);

  onMount(async () => {
    await refreshStats();
    await refreshEmbedding();
    await loadInitialLibrary();
    searchInputEl?.focus();
  });

  const refreshEmbedding = async () => {
    try {
      embedding = await api.getEmbeddingStatus();
    } catch (e) {
      console.error(e);
    }
  };

  const toggleEngine = async () => {
    embedBusy = true;
    errorMessage = null;
    try {
      if (embedding?.ready) {
        await api.stopEmbeddingEngine();
      } else {
        await api.startEmbeddingEngine();
      }
      await refreshEmbedding();
    } catch (e) {
      errorMessage = errMsg(e);
    } finally {
      embedBusy = false;
    }
  };

  const backfill = async () => {
    embedBusy = true;
    errorMessage = null;
    try {
      await api.backfillEmbeddings();
      await refreshEmbedding();
    } catch (e) {
      errorMessage = errMsg(e);
    } finally {
      embedBusy = false;
    }
  };

  const loadInitialLibrary = async () => {
    try {
      const docs = await api.listIndex();
      if (!query.trim()) {
        results = docs;
      }
    } catch (e) {
      console.error(e);
    } finally {
      libraryLoaded = true;
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

  const stopWatch = async () => {
    errorMessage = null;
    try {
      await api.stopWatchingFolder();
    } catch (e: any) {
      errorMessage = e?.message || "Failed to stop the folder watcher";
      return;
    }
    watching = false;
  };

  const indexSingleFile = async () => {
    if (!singleFile.trim()) return;
    loading = true;
    errorMessage = null;
    try {
      await api.indexFile(singleFile.trim());
      singleFile = "";
      await refreshStats();
      query = "";
      await loadInitialLibrary();
    } catch (e: any) {
      errorMessage = e?.message || "File indexing failed";
    } finally {
      loading = false;
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
      <h2 class="text-lg font-semibold tracking-tight text-ink-100">Document Library & Retrieval</h2>
      <p class="text-xs text-ink-400">Embedded SQLite WAL storage with BM25 keyword retrieval.</p>
    </div>
    {#if stats}
      <div class="flex items-center gap-4 text-xs font-mono text-ink-300 bg-ink-900 border border-ink-800 rounded-lg px-3 py-1.5">
        <span>Docs: {stats.doc_count}</span>
        <span>Size: {(stats.total_bytes / 1024).toFixed(1)} KB</span>
      </div>
    {/if}
  </div>

  {#if errorMessage}
    <ErrorBanner message={errorMessage} onDismiss={() => errorMessage = null} />
  {/if}

  <div class="border border-ink-800 rounded-2xl p-4 bg-ink-900/40 space-y-3">
    <div class="flex items-center justify-between gap-3 flex-wrap">
      <div>
        <div class="text-xs font-semibold text-ink-300 mb-1.5">Search mode</div>
        <div class="flex items-center gap-2">
          {#each ([["keyword", "Keyword"], ["hybrid", "Hybrid"], ["semantic", "Semantic"]] as const) as [value, label]}
            <button
              type="button"
              onclick={() => { mode = value; if (query.length > 2) search(); }}
              class="text-xs font-mono px-2.5 py-1 rounded-full border transition-colors {mode === value
                ? 'bg-ember-500/20 border-ember-500/40 text-ember-300'
                : 'border-ink-800 text-ink-400 hover:border-ink-700 hover:text-ink-300'}"
            >
              {label}
            </button>
          {/each}
        </div>
      </div>
      <div class="flex items-center gap-3 text-xs font-mono">
        {#if embedding?.ready}
          <span class="text-emerald-300 border border-emerald-500/30 bg-emerald-500/10 rounded-full px-2.5 py-1">
            Embeddings on · {embedding.model_id}{#if embedding.dimension} · {embedding.dimension}d{/if}
          </span>
          {#if embedding && embedding.embedded_chunks < embedding.total_chunks}
            <button onclick={backfill} disabled={embedBusy} class="text-ink-300 border border-ink-700 rounded-full px-2.5 py-1 hover:border-ink-600 disabled:opacity-40">
              {embedBusy ? "Backfilling…" : `Backfill ${embedding.total_chunks - embedding.embedded_chunks} chunks`}
            </button>
          {/if}
          <button onclick={toggleEngine} disabled={embedBusy} class="text-rose-300 border border-rose-500/30 rounded-full px-2.5 py-1 hover:border-rose-500/60 disabled:opacity-40">
            Stop
          </button>
        {:else}
          <span class="text-ink-400 border border-ink-800 rounded-full px-2.5 py-1">
            {#if embedding && embedding.total_chunks > 0}
              Keyword only · {embedding.embedded_chunks}/{embedding.total_chunks} chunks vectorized
            {:else}
              Keyword only
            {/if}
          </span>
          <button onclick={toggleEngine} disabled={embedBusy} class="text-ember-300 border border-ember-500/40 rounded-full px-2.5 py-1 hover:border-ember-500/70 disabled:opacity-40">
            {embedBusy ? "Starting…" : "Start embeddings"}
          </button>
        {/if}
      </div>
    </div>
    {#if mode !== "keyword" && !embedding?.ready}
      <p class="text-[11px] text-amber-300/90 font-mono">
        {mode} mode needs the embedding engine — results fall back to keyword ranking until it is running.
      </p>
    {/if}
  </div>

  <div class="border border-ink-800 rounded-2xl p-4 bg-ink-900/40 space-y-3">
    <div class="flex items-center justify-between">
      <div class="text-xs font-semibold text-ink-300">Index Local Directory</div>
      {#if watching}
        <button
          onclick={stopWatch}
          class="inline-flex items-center gap-1.5 text-[11px] font-mono text-emerald-400 hover:text-emerald-300 border border-emerald-500/30 hover:border-emerald-500/50 rounded-full px-2.5 py-1 transition-colors"
          title="Stop watching this directory"
        >
          <span class="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
          Watching — click to stop
        </button>
      {/if}
    </div>
    <div class="flex gap-2">
      <input
        type="text"
        placeholder="C:\Users\you\Documents\notes"
        id="folder-input"
        bind:value={folder}
        class="flex-1 bg-ink-950 border border-ink-800 rounded-lg px-3 py-2 text-xs text-ink-200 placeholder-ink-500 focus:outline-none focus:border-ink-700 font-mono"
      />
      <button onclick={indexFolder} disabled={loading || !folder.trim()} class="px-4 py-2 bg-ember-600 hover:bg-ember-500 disabled:opacity-40 rounded-lg text-xs font-medium text-ink-950 transition-colors">
        {loading ? "Indexing…" : "Index Folder"}
      </button>
      <button onclick={startWatch} disabled={watching || !folder.trim()} class="px-3 py-2 bg-ink-800 hover:bg-ink-700 disabled:opacity-40 rounded-lg text-xs font-medium text-ink-300 transition-colors">
        {watching ? "Watching" : "Watch"}
      </button>
    </div>
    <div class="flex gap-2">
      <input
        type="text"
        placeholder="…or index a single file: C:\Users\you\Documents\note.md"
        bind:value={singleFile}
        class="flex-1 bg-ink-950 border border-ink-800 rounded-lg px-3 py-2 text-xs text-ink-200 placeholder-ink-500 focus:outline-none focus:border-ink-700 font-mono"
      />
      <button onclick={indexSingleFile} disabled={loading || !singleFile.trim()} class="px-4 py-2 bg-ink-800 hover:bg-ink-700 disabled:opacity-40 rounded-lg text-xs font-medium text-ink-300 transition-colors">
        Index File
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
          placeholder="Search documents by keyword (Enter to search, Esc to clear)…"
          bind:value={query}
          onkeydown={handleKeydown}
          class="w-full bg-ink-900 border border-ink-800 rounded-lg pl-3 pr-8 py-2 text-sm text-ink-200 placeholder-ink-500 focus:outline-none focus:border-ink-700 font-mono"
        />
        {#if query}
          <button
            onclick={() => { query = ""; loadInitialLibrary(); }}
            class="absolute right-2.5 top-1/2 -translate-y-1/2 text-xs text-ink-400 hover:text-ink-300"
          >
            <Icon name="close" class="w-3.5 h-3.5" />
          </button>
        {/if}
      </div>
      <span class="px-3 py-2 text-xs text-ink-400 font-mono" title="Only keyword search runs in this version">Keyword (BM25)</span>
      <button onclick={search} disabled={loading} class="px-4 py-2 bg-ink-800 hover:bg-ink-700 rounded-lg text-xs font-medium text-ink-200 transition-colors">
        Search
      </button>
    </div>

    <div class="flex items-center justify-between text-[11px] font-mono text-ink-400 px-1">
      <span>{results.length} {results.length === 1 ? "document" : "documents"} {query ? "found" : "in library"}</span>
      {#if searchDurationMs !== null}
        <span class="text-emerald-400">Query completed in {searchDurationMs}ms</span>
      {/if}
    </div>

    {#if loading}
      <div class="py-12 text-center text-xs text-ink-400 font-mono">Searching library index…</div>
    {:else if !libraryLoaded}
      <div class="py-12 text-center text-xs text-ink-400 font-mono">Loading library…</div>
    {:else if results.length === 0}
      <div class="py-16 max-w-md mx-auto text-center space-y-2">
        {#if query}
          <h3 class="text-sm font-medium text-ink-200">No documents match “{query}”.</h3>
          <p class="text-xs text-ink-400">Try fewer or different words. Search looks for the words you type in indexed files.</p>
        {:else}
          <h3 class="text-sm font-medium text-ink-200">Your library is empty.</h3>
          <p class="text-xs text-ink-400">Index a folder of notes, documents, or code. Only the files in that folder are searched, and nothing leaves this computer.</p>
          <button
            type="button"
            onclick={() => document.getElementById("folder-input")?.focus()}
            class="mt-3 px-4 py-2 bg-ink-800 hover:bg-ink-700 rounded-lg text-xs font-medium text-ink-200"
          >
            Choose a folder to index
          </button>
        {/if}
      </div>
    {:else}
      <div class="space-y-2">
        {#each results as doc}
          <div class="border border-ink-800 rounded-xl p-3.5 bg-ink-900/30 hover:border-ink-700 transition-colors flex justify-between items-start gap-4">
            <div class="space-y-1.5 flex-1 min-w-0">
              <div class="font-medium text-sm text-ink-200 truncate">{doc.title}</div>
              <p class="text-xs text-ink-400 line-clamp-2 font-mono leading-relaxed">{doc.snippet}</p>
              <div class="flex flex-wrap items-center gap-3 text-xs font-mono text-ink-400">
                <span>Score: {doc.score.toFixed(3)}</span>
                <span class="truncate">Path: {doc.path}</span>
                <span>{(doc.size / 1024).toFixed(1)} KB</span>
              </div>
            </div>
            <button
              onclick={() => removeDoc(doc.path)}
              class="text-xs text-ink-400 hover:text-rose-400 px-2.5 py-1 rounded bg-ink-900 hover:bg-ink-800 transition-colors flex-shrink-0"
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
