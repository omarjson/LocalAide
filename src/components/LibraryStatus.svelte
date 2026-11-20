<script lang="ts">
  // Search → Library status: what is indexed, how search works right now,
  // folder presets, live indexing progress and the report afterwards.
  // Nothing here uses the network except downloading the meaning model,
  // which the user starts.
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import { formatGb } from "../lib/format";
  import { dur, motion } from "../lib/motion";
  import type { FolderPreset, IndexProgress, IndexReport, LibraryStatus } from "../lib/api/bindings";

  interface Props {
    /** After a folder is added, removed or rescanned. */
    onChanged?: () => void;
  }
  let { onChanged }: Props = $props();

  let status = $state<LibraryStatus | null>(null);
  let presets = $state<FolderPreset[]>([]);
  let progress = $state<IndexProgress | null>(null);
  let report = $state<IndexReport | null>(null);
  let showSkipped = $state(false);
  let error = $state<string | null>(null);
  let busyModel = $state<string | null>(null);
  let downloadPct = $state<number | null>(null);
  let showFolders = $state(false);

  const refresh = async () => {
    try {
      [status, presets] = await Promise.all([api.libraryStatus(), api.libraryFolderPresets()]);
    } catch (e: unknown) {
      error = errorMessage(e);
    }
  };
  onMount(() => {
    void refresh();
    // The meaning model fills in vectors in the background; show it.
    const t = setInterval(() => {
      if (status?.embedding_ready && status.embedded_chunks < status.total_chunks) void refresh();
    }, 4000);
    return () => clearInterval(t);
  });

  /** Add (or rescan) a folder with live progress. */
  export async function addFolder(path: string) {
    if (progress) return;
    error = null;
    report = null;
    showSkipped = false;
    progress = { folder: path, seen: 0, indexed: 0, skipped: 0, current: "" };
    try {
      report = await api.libraryIndex(path, (p) => (progress = p));
      onChanged?.();
    } catch (e: unknown) {
      error = errorMessage(e);
    } finally {
      progress = null;
      await refresh();
    }
  }

  const pickOther = async () => {
    try {
      const picked = await api.pickFolder();
      if (picked) await addFolder(picked);
    } catch (e: unknown) {
      error = errorMessage(e);
    }
  };

  const removeFolder = async (path: string) => {
    error = null;
    try {
      await api.libraryFolderRemove(path);
      onChanged?.();
      await refresh();
    } catch (e: unknown) {
      error = errorMessage(e);
    }
  };

  const smallest = $derived(status?.choices[0] ?? null);
  const installed = $derived(status?.choices.filter((c) => c.installed) ?? []);

  /** Download (if needed) and choose a meaning model. */
  const useModel = async (id: string) => {
    error = null;
    busyModel = id;
    try {
      const choice = status?.choices.find((c) => c.id === id);
      if (choice && !choice.installed) {
        downloadPct = 0;
        await api.downloadModel(id, (e) => {
          if (e.status === "downloading") downloadPct = Math.round(e.percent);
        });
      }
      downloadPct = null;
      status = await api.libraryEmbeddingSelect(id);
    } catch (e: unknown) {
      error = errorMessage(e);
    } finally {
      busyModel = null;
      downloadPct = null;
      await refresh();
    }
  };

  const ago = (secs: number | null) => {
    if (!secs) return "never";
    const s = Math.max(0, Date.now() / 1000 - secs);
    if (s < 60) return "just now";
    if (s < 3600) return `${Math.round(s / 60)} min ago`;
    if (s < 86400) return `${Math.round(s / 3600)} h ago`;
    return new Date(secs * 1000).toLocaleDateString();
  };
  const name = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() ?? path;
  const semantic = $derived(status?.search_mode === "semantic");
  const embedPct = $derived(status && status.total_chunks > 0 ? Math.round((status.embedded_chunks / status.total_chunks) * 100) : 100);
</script>

<section class="lib" aria-label="Library status">
  <div class="grid gap-px overflow-hidden rounded-t-[11px] bg-ink-800 sm:grid-cols-4">
    <div class="stat">
      <span class="stat-label">Search</span>
      {#if status}
        <span class={`stat-value flex items-center gap-1.5 ${semantic ? "text-emerald-300" : "text-ember-200"}`}>
          <span class={`h-2 w-2 rounded-full ${semantic ? "bg-emerald-400" : "bg-ember-500"}`} aria-hidden="true"></span>
          {semantic ? "By meaning" : "Exact words only"}
        </span>
      {:else}<span class="skeleton h-5 w-24"></span>{/if}
    </div>
    <div class="stat">
      <span class="stat-label">Folders</span>
      <span class="stat-value">{status?.folders.length ?? "–"}</span>
    </div>
    <div class="stat">
      <span class="stat-label">Files</span>
      <span class="stat-value tabular-nums">{status ? status.documents.toLocaleString() : "–"}</span>
    </div>
    <div class="stat">
      <span class="stat-label">Last scan</span>
      <span class="stat-value">{status ? ago(status.last_scan_at) : "–"}</span>
    </div>
  </div>

  <div class="space-y-4 p-4">
    {#if status && !semantic}
      <div class="flex flex-col gap-3 rounded-lg border border-ember-500/30 bg-ember-500/[0.06] p-3 sm:flex-row sm:items-center">
        <Icon name="info" class="hidden h-4 w-4 shrink-0 text-ember-300 sm:block" />
        <p class="flex-1 text-sm leading-6 text-ink-200">{status.reason}</p>
        {#if installed.length > 0}
          <Button variant="primary" disabled={!!busyModel} onclick={() => void useModel((installed.find((c) => c.id === status?.embedding_model) ?? installed[0]).id)}>
            {busyModel ? "Starting…" : `Use ${(installed.find((c) => c.id === status?.embedding_model) ?? installed[0]).name}`}
          </Button>
        {:else if smallest}
          <Button variant="primary" disabled={!!busyModel} onclick={() => void useModel(smallest.id)}>
            <Icon name="download" class="h-4 w-4" />
            {downloadPct !== null ? `Downloading ${downloadPct}%` : `Download ${smallest.name} (${formatGb(smallest.size_bytes)})`}
          </Button>
        {/if}
      </div>
    {:else if status && semantic}
      <div class="flex flex-wrap items-center gap-x-3 gap-y-2 text-sm text-ink-400">
        <span>Meaning model: <b class="font-medium text-ink-200">{status.choices.find((c) => c.id === status?.embedding_model)?.name ?? status.embedding_model}</b></span>
        {#if status.embedded_chunks < status.total_chunks}
          <span class="flex items-center gap-2">
            <span class="h-1.5 w-28 overflow-hidden rounded-full bg-ink-800"><span class="block h-full bg-ember-500 transition-[width] duration-500" style={`width:${embedPct}%`}></span></span>
            Reading for meaning · {embedPct}%
          </span>
        {/if}
        {#if status.choices.length > 1}
          <label class="ml-auto flex items-center gap-2">
            <span class="text-ink-500">Change</span>
            <select class="h-8 rounded-md border border-ink-700 bg-ink-950 px-2 text-sm text-ink-100" value={status.embedding_model ?? ""} disabled={!!busyModel} onchange={(e) => void useModel((e.currentTarget as HTMLSelectElement).value)}>
              {#each status.choices as c (c.id)}<option value={c.id}>{c.name}{c.installed ? "" : ` (download ${formatGb(c.size_bytes)})`}</option>{/each}
            </select>
          </label>
        {/if}
      </div>
    {/if}

    <div>
      <div class="mb-2 flex items-center justify-between">
        <h3 class="text-sm font-semibold text-ink-100">Add a folder</h3>
        <span class="text-xs text-ink-500">Indexing and search stay on this computer.</span>
      </div>
      <div class="flex flex-wrap gap-2">
        {#each presets as p (p.id)}
          <button type="button" class={`preset ${p.added ? "preset-on" : ""}`} disabled={!p.exists || !!progress} title={p.path ?? "Not found on this computer"} onclick={() => p.path && void addFolder(p.path)}>
            <Icon name={p.added ? "check" : "folder"} class="h-4 w-4" />
            <span>{p.label}</span>
            {#if p.added}<span class="text-xs text-ink-400">Scan again</span>{/if}
          </button>
        {/each}
        <button type="button" class="preset" disabled={!!progress} onclick={() => void pickOther()}><Icon name="folder-plus" class="h-4 w-4" /><span>Another folder…</span></button>
      </div>
    </div>

    {#if progress}
      <div class="rounded-lg border border-ink-700 bg-ink-950 p-3" transition:slide={{ duration: dur(motion.base) }} aria-live="polite">
        <div class="flex items-center gap-3">
          <span class="h-4 w-4 shrink-0 animate-spin rounded-full border-2 border-ink-700 border-t-ember-500" aria-hidden="true"></span>
          <span class="min-w-0 flex-1 truncate text-sm text-ink-100">Adding <b class="font-semibold">{name(progress.folder)}</b></span>
          <Button variant="subtle" onclick={() => void api.libraryIndexCancel()}>Cancel</Button>
        </div>
        <div class="bar mt-2.5"><span></span></div>
        <div class="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs tabular-nums text-ink-400">
          <span><b class="text-ink-100">{progress.indexed.toLocaleString()}</b> added</span>
          <span><b class="text-ink-200">{progress.skipped.toLocaleString()}</b> skipped</span>
          <span>{progress.seen.toLocaleString()} looked at</span>
          {#if progress.current}<span class="min-w-0 flex-1 truncate font-mono text-ink-500" title={progress.current}>{progress.current}</span>{/if}
        </div>
      </div>
    {/if}

    {#if report}
      <div class="rounded-lg border border-ink-700 bg-ink-950 p-3" transition:slide={{ duration: dur(motion.base) }}>
        <div class="flex flex-wrap items-baseline gap-x-2">
          <Icon name={report.cancelled ? "x-circle" : "check"} class={`h-4 w-4 self-center ${report.cancelled ? "text-ink-400" : "text-emerald-400"}`} />
          <span class="text-sm font-semibold text-ink-50">{report.cancelled ? "Stopped" : "Added"} {name(report.folder)}</span>
          <span class="text-sm text-ink-400">{report.indexed.toLocaleString()} {report.indexed === 1 ? "file" : "files"} in the library · {report.skipped.toLocaleString()} skipped · {(report.ms / 1000).toFixed(1)} s</span>
          <button type="button" class="ml-auto text-xs text-ink-500 hover:text-ink-200" onclick={() => (report = null)}>Dismiss</button>
        </div>
        {#if report.limited}<p class="mt-1.5 text-xs text-ember-200">The folder is very large, so the scan stopped at 20,000 files or 32 folder levels.</p>{/if}
        {#if report.reasons.length > 0}
          <div class="mt-2 flex flex-wrap gap-1.5">
            {#each report.reasons as [reason, count] (reason)}<span class="rounded-md bg-ink-800 px-2 py-0.5 text-xs text-ink-300">{reason}: {count.toLocaleString()}</span>{/each}
          </div>
          {#if report.skipped_files.length > 0}
            <button type="button" class="mt-2 flex items-center gap-1 text-xs text-ink-400 hover:text-ink-100" aria-expanded={showSkipped} onclick={() => (showSkipped = !showSkipped)}>
              <Icon name="chevron-right" class={`h-3.5 w-3.5 transition-transform ${showSkipped ? "rotate-90" : ""}`} />Show skipped files{report.skipped > report.skipped_files.length ? ` (first ${report.skipped_files.length})` : ""}
            </button>
            {#if showSkipped}
              <ul class="mt-1.5 max-h-48 overflow-y-auto rounded-md border border-ink-800 font-mono text-[12px]" transition:slide={{ duration: dur(motion.base) }}>
                {#each report.skipped_files as f (f.path)}<li class="flex gap-3 border-b border-ink-800 px-2.5 py-1 last:border-b-0"><span class="min-w-0 flex-1 truncate text-ink-300" title={f.path}>{f.path}</span><span class="shrink-0 text-ink-500">{f.reason}</span></li>{/each}
              </ul>
            {/if}
          {/if}
        {/if}
      </div>
    {/if}

    {#if status && status.folders.length > 0}
      <div>
        <button type="button" class="flex items-center gap-1.5 text-sm text-ink-300 hover:text-ink-50" aria-expanded={showFolders} onclick={() => (showFolders = !showFolders)}>
          <Icon name="chevron-right" class={`h-4 w-4 transition-transform ${showFolders ? "rotate-90" : ""}`} />
          {status.folders.length} {status.folders.length === 1 ? "folder" : "folders"} in the library
        </button>
        {#if showFolders}
          <ul class="mt-2 divide-y divide-ink-800 rounded-lg border border-ink-800" transition:slide={{ duration: dur(motion.base) }}>
            {#each status.folders as f (f.path)}
              <li class="flex flex-wrap items-center gap-x-3 gap-y-1 px-3 py-2">
                <Icon name="folder" class="h-4 w-4 shrink-0 text-ink-400" />
                <span class="min-w-0 flex-1">
                  <span class="block truncate text-sm text-ink-100" title={f.path}>{f.path}</span>
                  <span class="block text-xs text-ink-500">{f.documents.toLocaleString()} files · scanned {ago(f.last_scan_at)}{f.skipped ? ` · ${f.skipped.toLocaleString()} skipped` : ""}</span>
                </span>
                <Button variant="subtle" disabled={!!progress} onclick={() => void addFolder(f.path)}>Scan again</Button>
                <Button variant="danger" disabled={!!progress} onclick={() => void removeFolder(f.path)}>Remove</Button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {/if}

    {#if error}<p class="rounded-md bg-rose-500/10 px-3 py-2 text-sm text-rose-300" role="alert">{error}</p>{/if}
  </div>
</section>

<style>
  .lib {
    border-radius: 0.75rem;
    border: 1px solid var(--color-ink-800);
    background: var(--color-ink-900);
  }
  .stat {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    background: var(--color-ink-900);
    padding: 0.7rem 1rem;
  }
  .stat-label {
    font-size: 0.75rem;
    color: var(--color-ink-500);
  }
  .stat-value {
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--color-ink-50);
  }
  .preset {
    display: inline-flex;
    height: 2.25rem;
    align-items: center;
    gap: 0.5rem;
    border-radius: 0.6rem;
    border: 1px solid var(--color-ink-700);
    background: var(--color-ink-950);
    padding: 0 0.8rem;
    font-size: 0.875rem;
    color: var(--color-ink-100);
    transition: border-color 120ms ease-out, background-color 120ms ease-out;
  }
  .preset:not(:disabled):hover {
    border-color: var(--color-edge);
    background: var(--color-ink-800);
  }
  .preset:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .preset-on {
    border-color: color-mix(in oklab, var(--color-ember-500) 40%, transparent);
  }
  .preset-on :global(svg) {
    color: var(--color-ember-300);
  }
  .bar {
    position: relative;
    height: 4px;
    overflow: hidden;
    border-radius: 9999px;
    background: var(--color-ink-800);
  }
  .bar span {
    position: absolute;
    inset: 0;
    width: 35%;
    border-radius: 9999px;
    background: var(--color-ember-500);
    animation: sweep 1.3s ease-in-out infinite;
  }
  @keyframes sweep {
    from { transform: translateX(-100%); }
    to { transform: translateX(290%); }
  }
  :global([data-motion="off"]) .bar span {
    animation: none;
    width: 100%;
    opacity: 0.6;
  }
</style>
