<script lang="ts">
  import { formatBytes } from "../lib/format";
  import LibraryStatus from "../components/LibraryStatus.svelte";
  import { onMount } from "svelte";
  import Button from "../components/Button.svelte";
  import ConfirmDialog from "../components/ConfirmDialog.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import ErrorBanner from "../components/ErrorBanner.svelte";
  import Icon from "../components/Icon.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import StatusBadge from "../components/StatusBadge.svelte";
  import Tabs from "../components/Tabs.svelte";
  import { api, errorMessage as errMsg } from "../lib/api/client";
  import { popover } from "../lib/actions/popover";
  import { openInBrowser } from "../lib/links";
  import { showToast } from "../lib/toast.svelte";
  import type {
    EmbeddingStatusResponse,
    IndexerStatsResponse,
    RerankStatusResponse,
    SearchResultDoc,
    WebHit,
    WebPage,
  } from "../lib/api/bindings";

  type SearchMode = "keyword" | "semantic" | "hybrid" | "web";

  let query = $state("");
  let mode = $state<SearchMode>("keyword");
  let results = $state<SearchResultDoc[]>([]);
  let webResults = $state<WebHit[]>([]);
  let webPages = $state<Record<string, WebPage>>({});
  let readingUrl = $state<string | null>(null);
  let stats = $state<IndexerStatsResponse | null>(null);
  let folder = $state("");
  let watching = $state(false);
  let loading = $state(false);
  let libraryLoaded = $state(false);
  let searchDurationMs = $state<number | null>(null);
  let errorMessage = $state<string | null>(null);
  let searchInputEl = $state<HTMLInputElement | null>(null);
  let folderInputEl = $state<HTMLInputElement | null>(null);
  let singleFileInputEl = $state<HTMLInputElement | null>(null);
  let singleFile = $state("");
  let embedding = $state<EmbeddingStatusResponse | null>(null);
  let embedBusy = $state(false);
  let rerank = $state<RerankStatusResponse | null>(null);
  let rerankBusy = $state(false);
  let showSearchSettings = $state(false);
  let showIndexing = $state(false);
  let openMenuPath = $state<string | null>(null);
  let pendingDeletePath = $state<string | null>(null);

  const localSearchTabs = [
    { value: "keyword", label: "Exact words" },
    { value: "hybrid", label: "Words + meaning" },
    { value: "semantic", label: "Meaning" },
  ];

  onMount(async () => {
    await refreshStats();
    await refreshEmbedding();
    await refreshRerank();
    await loadInitialLibrary();
    searchInputEl?.focus();
  });

  const refreshRerank = async () => {
    try {
      rerank = await api.getRerankStatus();
    } catch (e: unknown) {
      console.error(e);
    }
  };

  const toggleRerank = async () => {
    rerankBusy = true;
    errorMessage = null;
    try {
      if (rerank?.ready) {
        await api.stopRerankEngine();
      } else {
        await api.startRerankEngine();
      }
      await refreshRerank();
    } catch (e: unknown) {
      errorMessage = errMsg(e);
    } finally {
      rerankBusy = false;
    }
  };

  const refreshEmbedding = async () => {
    try {
      embedding = await api.getEmbeddingStatus();
    } catch (e: unknown) {
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
    } catch (e: unknown) {
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
    } catch (e: unknown) {
      errorMessage = errMsg(e);
    } finally {
      embedBusy = false;
    }
  };

  // Every search and library load takes a number. A reply that arrives after
  // a newer request started is dropped, so old results never replace new ones.
  let requestSeq = 0;
  const LIBRARY_PAGE = 200;
  let libraryHasMore = $state(false);
  let loadingMore = $state(false);

  const loadInitialLibrary = async () => {
    const seq = ++requestSeq;
    try {
      const docs = await api.listIndex(LIBRARY_PAGE, 0);
      if (seq === requestSeq && !query.trim()) {
        results = docs;
        libraryHasMore = docs.length === LIBRARY_PAGE;
      }
    } catch (e: unknown) {
      if (seq === requestSeq) errorMessage = errMsg(e);
    } finally {
      libraryLoaded = true;
    }
  };

  const loadMoreLibrary = async () => {
    loadingMore = true;
    const seq = requestSeq;
    try {
      const more = await api.listIndex(LIBRARY_PAGE, results.length);
      if (seq === requestSeq && !query.trim()) {
        results = [...results, ...more];
        libraryHasMore = more.length === LIBRARY_PAGE;
      }
    } catch (e: unknown) {
      errorMessage = errMsg(e);
    } finally {
      loadingMore = false;
    }
  };

  const search = async () => {
    if (!query.trim()) {
      await loadInitialLibrary();
      webResults = [];
      searchDurationMs = null;
      return;
    }
    const seq = ++requestSeq;
    loading = true;
    errorMessage = null;
    const start = performance.now();
    try {
      if (mode === "web") {
        const hits = await api.webSearch(query);
        if (seq !== requestSeq) return;
        results = [];
        webResults = hits;
      } else {
        const docs = await api.searchIndex({ query, mode, limit: 50 });
        if (seq !== requestSeq) return;
        webResults = [];
        results = docs;
      }
      libraryHasMore = false;
      searchDurationMs = Math.round(performance.now() - start);
    } catch (e: unknown) {
      if (seq === requestSeq) errorMessage = errMsg(e);
    } finally {
      if (seq === requestSeq) loading = false;
    }
  };

  const readPage = async (url: string) => {
    if (webPages[url]) return;
    readingUrl = url;
    errorMessage = null;
    try {
      const page = await api.webCrawl(url);
      webPages = { ...webPages, [url]: page };
    } catch (e: unknown) {
      errorMessage = errMsg(e);
    } finally {
      readingUrl = null;
    }
  };

  // Folders go through the Library status card, which shows live progress
  // and the report of what was added and skipped.
  let libraryEl = $state<{ addFolder: (path: string) => Promise<void> } | null>(null);
  const indexFolder = async () => {
    if (!folder.trim()) return;
    loading = true;
    errorMessage = null;
    try {
      await libraryEl?.addFolder(folder.trim());
    } finally {
      loading = false;
    }
  };
  const libraryChanged = async () => {
    await refreshStats();
    await refreshEmbedding();
    if (!query.trim()) await loadInitialLibrary();
  };

  const refreshStats = async () => {
    try {
      stats = await api.getIndexerStats();
    } catch (e: unknown) {
      console.error(e);
    }
  };

  const startWatch = async () => {
    if (!folder.trim()) return;
    errorMessage = null;
    try {
      await api.watchFolder(folder);
      watching = true;
    } catch (e: unknown) {
      errorMessage = errMsg(e);
    }
  };

  const stopWatch = async () => {
    errorMessage = null;
    try {
      await api.stopWatchingFolder();
    } catch (e: unknown) {
      errorMessage = errMsg(e);
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
    } catch (e: unknown) {
      errorMessage = errMsg(e);
    } finally {
      loading = false;
    }
  };

  const removeDoc = async (path: string) => {
    errorMessage = null;
    try {
      await api.removeIndexedDoc(path);
      results = results.filter((result) => result.path !== path);
      await refreshStats();
    } catch (e: unknown) {
      errorMessage = errMsg(e);
    }
  };

  const askRemoveDoc = (path: string) => {
    openMenuPath = null;
    pendingDeletePath = path;
  };

  const confirmRemoveDoc = async () => {
    const path = pendingDeletePath;
    pendingDeletePath = null;
    if (path) await removeDoc(path);
  };

  // The header buttons open the system picker and add the choice right away.
  // The panel below stays open for typing a path or watching the folder.
  const pickAndAddFolder = async () => {
    showIndexing = true;
    errorMessage = null;
    try {
      const picked = await api.pickFolder();
      if (!picked) return;
      folder = picked;
      await indexFolder();
    } catch (e: unknown) {
      errorMessage = errMsg(e);
    }
  };

  const pickAndAddFile = async () => {
    showIndexing = true;
    errorMessage = null;
    try {
      const picked = await api.pickDocument();
      if (!picked) return;
      singleFile = picked;
      await indexSingleFile();
    } catch (e: unknown) {
      errorMessage = errMsg(e);
    }
  };

  const browseFolder = async () => {
    errorMessage = null;
    try {
      const picked = await api.pickFolder();
      if (picked) folder = picked;
    } catch (e: unknown) {
      errorMessage = errMsg(e);
    }
  };

  const browseFile = async () => {
    errorMessage = null;
    try {
      const picked = await api.pickDocument();
      if (picked) singleFile = picked;
    } catch (e: unknown) {
      errorMessage = errMsg(e);
    }
  };

  const setSearchTab = (value: string) => {
    if (value === "web") {
      // A web query leaves this computer, so it waits for Enter or the button.
      mode = "web";
      return;
    }
    mode = mode === "web" ? "keyword" : mode;
    if (query.length > 2) void search();
  };

  const setLocalMode = (value: string) => {
    if (value === "keyword" || value === "semantic" || value === "hybrid") {
      mode = value;
      if (query.length > 2) void search();
    }
  };

  const handleKeydown = (event: KeyboardEvent) => {
    if (event.key === "Enter") {
      void search();
    } else if (event.key === "Escape") {
      query = "";
      void loadInitialLibrary();
    }
  };

  const sourceLabel = (id: string): string =>
    ({ duckduckgo: "DuckDuckGo", exa: "Exa", firecrawl: "Firecrawl", wikipedia: "Wikipedia" })[id] ?? id;

  const fileType = (title: string) => {
    const extension = title.split(".").pop()?.toUpperCase();
    return extension && extension.length <= 4 ? extension : "FILE";
  };

  $effect(() => {
    // Typing never sends a web query. Only Enter or the button does.
    if (mode === "web") return;
    if (query.length > 2) {
      const timer = setTimeout(() => void search(), 150);
      return () => clearTimeout(timer);
    } else if (query.length === 0) {
      void loadInitialLibrary();
      searchDurationMs = null;
    }
  });
</script>

<div class="mx-auto max-w-5xl pb-10">
  <PageHeader
    title="Documents"
    description="Find things in the folders you add. Your files stay on this computer."
  >
    {#snippet actions()}
      <Button variant="secondary" disabled={loading} onclick={() => void pickAndAddFile()}>
        <Icon name="file-plus" class="h-[18px] w-[18px]" />
        Add a file
      </Button>
      <Button variant="primary" disabled={loading} onclick={() => void pickAndAddFolder()}>
        <Icon name="folder-plus" class="h-[18px] w-[18px]" />
        Add a folder
      </Button>
    {/snippet}
  </PageHeader>

  {#if errorMessage}
    <div class="mb-4">
      <ErrorBanner
        message={errorMessage}
        onRetry={() => void (query.trim() ? search() : loadInitialLibrary())}
        onDismiss={() => errorMessage = null}
      />
    </div>
  {/if}

  <div class="mb-5">
    <LibraryStatus bind:this={libraryEl} onChanged={() => void libraryChanged()} />
  </div>

  <div class="flex flex-col gap-3 lg:flex-row lg:items-center">
    <div class="field-shell flex h-[46px] min-w-0 flex-1 items-center gap-2 rounded-lg border border-ink-700 bg-ink-950 px-3 shadow-sm">
      <Icon name="search" class="h-[18px] w-[18px] shrink-0 text-ink-400" />
      <input
        bind:this={searchInputEl}
        type="search"
        aria-label={mode === "web" ? "Search the web" : "Search documents"}
        placeholder={mode === "web" ? "Search the web" : "Search your documents"}
        bind:value={query}
        onkeydown={handleKeydown}
        class="field-bare min-w-0 flex-1 bg-transparent text-base text-ink-100 placeholder:text-ink-400"
      />
      {#if query}
        <button
          type="button"
          aria-label="Clear search"
          onclick={() => { query = ""; void loadInitialLibrary(); }}
          class="grid h-7 w-7 place-items-center rounded-md text-ink-400 hover:bg-veil/5 hover:text-ink-50"
        >
          <Icon name="close" class="h-4 w-4" />
        </button>
      {/if}
    </div>
    <Tabs
      items={[{ value: "local", label: "My documents" }, { value: "web", label: "The web" }]}
      value={mode === "web" ? "web" : "local"}
      onChange={setSearchTab}
      ariaLabel="Search source"
    />
    {#if mode === "web"}
      <Button variant="primary" disabled={loading || !query.trim()} onclick={() => void search()}>
        Search the web
      </Button>
    {/if}
  </div>

  <div class="mt-3 flex flex-wrap items-center gap-x-4 gap-y-2 text-sm text-ink-400">
    <span>{stats?.doc_count ?? results.length} {stats?.doc_count === 1 ? "document" : "documents"}</span>
    {#if stats}
      <span>{formatBytes(stats.total_bytes)} stored</span>
    {/if}
    {#if watching}
      <StatusBadge label="Watching for new files" tone="ok" dot />
      <Button variant="subtle" class="px-2" onclick={stopWatch}>Stop watching</Button>
    {/if}
    <Button
      variant="subtle"
      class="ml-auto px-2"
      ariaLabel="Search settings"
      onclick={() => showSearchSettings = !showSearchSettings}
    >
      <Icon name="sliders" class="h-[18px] w-[18px]" />
      Search settings
    </Button>
  </div>

  {#if showSearchSettings}
    <div class="mt-4 rounded-lg border border-ink-700 bg-ink-900 p-4">
      <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
        <div>
          <h2 class="font-semibold text-ink-50">How search works</h2>
          <p class="mt-1 text-sm text-ink-400">Choose exact words or let a small local model find related ideas.</p>
        </div>
        {#if mode !== "web"}
          <Tabs items={localSearchTabs} value={mode} onChange={setLocalMode} ariaLabel="Document search mode" />
        {/if}
      </div>

      <div class="mt-4 flex flex-wrap items-center gap-2 border-t border-ink-800 pt-4">
        {#if embedding?.ready}
          <StatusBadge label="Meaning search is on" tone="ok" dot />
          {#if embedding.embedded_chunks < embedding.total_chunks}
            <Button variant="secondary" disabled={embedBusy} onclick={backfill}>
              {embedBusy ? "Updating…" : `Update ${embedding.total_chunks - embedding.embedded_chunks} files`}
            </Button>
          {/if}
          <Button variant="subtle" disabled={embedBusy} onclick={toggleEngine}>Stop meaning search</Button>
        {:else}
          <StatusBadge label="Exact words only" tone="idle" />
          <Button variant="secondary" disabled={embedBusy} onclick={toggleEngine}>
            {embedBusy ? "Starting…" : "Turn on meaning search"}
          </Button>
        {/if}
        {#if rerank?.ready}
          <StatusBadge label="Extra ranking on" tone="ok" dot />
          <Button variant="subtle" disabled={rerankBusy} onclick={toggleRerank}>Stop extra ranking</Button>
        {:else}
          <Button variant="subtle" disabled={rerankBusy} onclick={toggleRerank}>
            {rerankBusy ? "Starting…" : "Turn on extra ranking"}
          </Button>
        {/if}
      </div>

      {#if mode !== "keyword" && mode !== "web" && !embedding?.ready}
        <p class="mt-3 text-sm text-amber-300">Meaning search needs the local meaning model. Results use exact words until it starts.</p>
      {/if}
      {#if mode === "web"}
        <p class="mt-3 text-sm text-ink-400">Web search uses the provider in Settings. Nothing leaves this computer until you search.</p>
      {/if}
    </div>
  {/if}

  {#if showIndexing}
    <div class="mt-4 rounded-lg border border-ink-700 bg-ink-900 p-4">
      <div class="flex items-center justify-between gap-3">
        <div>
          <h2 class="font-semibold text-ink-50">Add documents</h2>
          <p class="mt-1 text-sm text-ink-400">Add a folder to keep it up to date, or add one file.</p>
        </div>
        <Button variant="subtle" ariaLabel="Close add documents" onclick={() => showIndexing = false}>
          <Icon name="close" class="h-4 w-4" />
        </Button>
      </div>
      <div class="mt-4 grid gap-3 lg:grid-cols-[1fr_auto_auto_auto]">
        <input
          bind:this={folderInputEl}
          type="text"
          placeholder="C:\\Users\\you\\Documents"
          aria-label="Folder path"
          bind:value={folder}
          class="h-10 min-w-0 rounded-md border border-ink-700 bg-ink-950 px-3 text-sm text-ink-100 outline-none placeholder:text-ink-400 focus:border-edge"
        />
        <Button variant="secondary" disabled={loading} onclick={() => void browseFolder()}>Browse…</Button>
        <Button variant="primary" disabled={loading || !folder.trim()} onclick={indexFolder}>
          <Icon name="folder-plus" class="h-[18px] w-[18px]" />
          {loading ? "Adding…" : "Add folder"}
        </Button>
        <Button variant="secondary" disabled={watching || !folder.trim()} onclick={startWatch}>
          {watching ? "Watching" : "Watch folder"}
        </Button>
      </div>
      <div class="mt-3 grid gap-3 lg:grid-cols-[1fr_auto_auto]">
        <input
          bind:this={singleFileInputEl}
          type="text"
          placeholder="C:\\Users\\you\\Documents\\note.md"
          aria-label="File path"
          bind:value={singleFile}
          class="h-10 min-w-0 rounded-md border border-ink-700 bg-ink-950 px-3 text-sm text-ink-100 outline-none placeholder:text-ink-400 focus:border-edge"
        />
        <Button variant="secondary" disabled={loading} onclick={() => void browseFile()}>Browse…</Button>
        <Button variant="secondary" disabled={loading || !singleFile.trim()} onclick={indexSingleFile}>
          <Icon name="file-plus" class="h-[18px] w-[18px]" />
          Add file
        </Button>
      </div>
    </div>
  {/if}

  <div class="mt-5">
    <p class="mb-1 text-sm text-ink-400">
      {#if mode === "web"}
        {webResults.length} {webResults.length === 1 ? "result" : "results"}{query ? ` for “${query}”` : ""}
      {:else if query}
        {results.length} {results.length === 1 ? "result" : "results"} for “{query}”
      {:else}
        Your document library
      {/if}
    </p>

    {#if searchDurationMs !== null}
      <p class="mb-3 text-sm text-ink-400">Search finished in {searchDurationMs} ms.</p>
    {/if}

    {#if loading}
      <div class="rounded-lg border border-ink-700 bg-ink-900 px-4 py-14 text-center text-sm text-ink-400">
        {mode === "web" ? "Searching the web…" : "Searching your documents…"}
      </div>
    {:else if mode === "web"}
      {#if webResults.length === 0}
        <EmptyState
          icon="globe"
          title="No web results yet"
          description="Type a question or a few words above, then search."
        />
      {:else}
        <div class="overflow-hidden rounded-lg border border-ink-700 bg-ink-900">
          {#each webResults as hit (hit.url)}
            <article class="p-4 transition-colors hover:bg-veil/5 {hit !== webResults[0] ? 'border-t border-ink-800' : ''}">
              <div class="flex items-start gap-3">
                <div class="grid h-9 w-9 shrink-0 place-items-center rounded-md bg-ink-800 text-ink-200">
                  <Icon name="globe" class="h-[18px] w-[18px]" />
                </div>
                <div class="min-w-0 flex-1">
                  <a href={hit.url} rel="noreferrer" onclick={(e) => openInBrowser(e, hit.url)} onauxclick={(e) => openInBrowser(e, hit.url)} class="font-semibold text-ink-50 hover:text-ember-300 hover:underline">{hit.title}</a>
                  <div class="mt-1 flex min-w-0 items-center gap-2 text-sm text-ink-400"><span class="truncate">{hit.url}</span>{#if hit.source}<span class="shrink-0 rounded-full border border-ink-700 px-2 text-xs text-ink-500">via {sourceLabel(hit.source)}</span>{/if}</div>
                  <p class="mt-2 text-sm leading-6 text-ink-200">{hit.snippet}</p>
                  <div class="mt-3 flex flex-wrap items-center gap-2">
                    {#if webPages[hit.url]}
                      <Button variant="subtle" onclick={() => { const rest = { ...webPages }; delete rest[hit.url]; webPages = rest; }}>Hide page text</Button>
                    {:else}
                      <Button variant="secondary" disabled={readingUrl !== null} onclick={() => void readPage(hit.url)}>
                        {readingUrl === hit.url ? "Reading…" : "Read page"}
                      </Button>
                    {/if}
                  </div>
                  {#if webPages[hit.url]}
                    <pre class="mt-3 max-h-80 overflow-auto rounded-md border border-ink-700 bg-ink-950 p-3 text-sm leading-6 text-ink-300 whitespace-pre-wrap">{webPages[hit.url].content}</pre>
                  {/if}
                </div>
              </div>
            </article>
          {/each}
        </div>
      {/if}
    {:else if !libraryLoaded}
      <div class="rounded-lg border border-ink-700 bg-ink-900 px-4 py-14 text-center text-sm text-ink-400">Loading your library…</div>
    {:else if results.length === 0}
      <EmptyState
        icon="file"
        title={query ? `No documents match “${query}”.` : "Your library is empty."}
        description={query ? "Try fewer or different words." : "Add a folder of notes, documents, or code. Your files stay on this computer."}
      >
        {#snippet action()}
          {#if !query}<Button variant="primary" disabled={loading} onclick={() => void pickAndAddFolder()}>Add a folder</Button>{/if}
        {/snippet}
      </EmptyState>
    {:else}
      <div class="overflow-hidden rounded-lg border border-ink-700 bg-ink-900">
        {#each results as doc, index (doc.path)}
          <article class={`group relative grid gap-x-3 gap-y-1 p-4 transition-colors hover:bg-veil/5 sm:grid-cols-[36px_minmax(0,1fr)_auto] ${index > 0 ? 'border-t border-ink-800' : ''}`}>
            <div class="row-span-3 grid h-9 w-9 place-items-center rounded-md bg-ink-800 text-xs font-semibold text-ink-200">{fileType(doc.title)}</div>
            <div class="min-w-0">
              <div class="truncate font-semibold text-ink-50">{doc.title}</div>
              <div class="truncate text-sm text-ink-400">{doc.path}</div>
              <p class="mt-1 text-sm leading-6 text-ink-200">{doc.snippet}</p>
              <div class="mt-2 flex items-center gap-2 text-sm text-ink-400">
                {#if query.trim()}
                  <span>Relevance {doc.score.toFixed(2)}</span>
                  <span class="h-3 w-px bg-ink-700" aria-hidden="true"></span>
                {/if}
                <span>{formatBytes(doc.size)}</span>
              </div>
            </div>
            <div class="relative self-start justify-self-end" use:popover={{ open: openMenuPath === doc.path, onClose: () => openMenuPath = null }}>
              <Button
                variant="subtle"
                class="h-8 w-8 px-0"
                ariaLabel={`More actions for ${doc.title}`}
                ariaExpanded={openMenuPath === doc.path}
                ariaHaspopup="menu"
                onclick={() => openMenuPath = openMenuPath === doc.path ? null : doc.path}
              >
                <Icon name="more" class="h-[18px] w-[18px]" />
              </Button>
              {#if openMenuPath === doc.path}
                <div role="menu" aria-label={`Actions for ${doc.title}`} class="absolute right-0 top-9 z-10 min-w-44 rounded-md border border-edge bg-ink-900 p-1 shadow-xl">
                  <button type="button" role="menuitem" onclick={() => askRemoveDoc(doc.path)} class="flex w-full items-center gap-2 rounded px-3 py-2 text-left text-sm text-rose-400 hover:bg-rose-500/10 focus:bg-rose-500/10 focus:outline-none">
                    <Icon name="trash" class="h-4 w-4" />
                    Remove from library
                  </button>
                </div>
              {/if}
            </div>
          </article>
        {/each}
      </div>
      {#if !query && libraryHasMore}
        <div class="mt-3 flex justify-center">
          <Button variant="secondary" disabled={loadingMore} onclick={() => void loadMoreLibrary()}>
            {loadingMore ? "Loading…" : "Show more documents"}
          </Button>
        </div>
      {/if}
    {/if}
  </div>
</div>

{#if pendingDeletePath}
  <ConfirmDialog
    title="Remove this document?"
    message="The file stays on your computer. LocalAide will only remove it from the library."
    confirmLabel="Remove"
    onConfirm={() => void confirmRemoveDoc()}
    onCancel={() => pendingDeletePath = null}
  />
{/if}
