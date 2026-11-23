<script lang="ts">
  // Session picker: saved chats and the chats already open in tabs, in one
  // place. FTS search is the same local index the search modal uses; the
  // listing below it comes from the already-loaded thread/tab state, so it
  // costs no extra IPC. Nothing leaves this computer.
  import Modal from "./Modal.svelte";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import { chats } from "../lib/chats.svelte";
  import type { ChatThread, SessionHit } from "../lib/api/bindings";

  let {
    onClose,
    onOpen,
  }: {
    onClose: () => void;
    /** Parent navigates to chat and opens/focuses the real tab for this hit. */
    onOpen: (hit: SessionHit) => void;
  } = $props();

  let query = $state("");
  let hits = $state<SessionHit[]>([]);
  let busy = $state(false);
  let searched = $state(false);
  let error = $state<string | null>(null);

  const tabFor = (threadId: string) => chats.tabs.find((t) => t.id === threadId);
  const busyFor = (threadId: string) => tabFor(threadId)?.busy === true;

  const openThread = (thread: ChatThread) => {
    onOpen({
      thread_id: thread.id,
      title: thread.title,
      snippet: `${thread.message_count} ${thread.message_count === 1 ? "message" : "messages"}`,
      path: thread.folder ?? "",
    });
  };

  // The local FTS index. Rebuild when the index is empty (older conversations).
  const search = async () => {
    const q = query.trim();
    if (!q || busy) return;
    busy = true;
    error = null;
    try {
      hits = await api.sessionSearch(q, 50);
      searched = true;
    } catch (e: unknown) {
      error = errorMessage(e);
      hits = [];
    } finally {
      busy = false;
    }
  };

  const reindex = async () => {
    if (busy) return;
    busy = true;
    error = null;
    try {
      await api.sessionReindex();
      if (query.trim()) await search();
    } catch (e: unknown) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  };

  const saved = $derived(
    [...chats.threads].sort((a, b) => b.updated_at - a.updated_at).slice(0, 40),
  );
</script>

<Modal title="Pick a session" onClose={onClose} size="lg">
  <form class="flex items-center gap-2" onsubmit={(e) => { e.preventDefault(); void search(); }}>
    <label class="sr-only" for="session-picker-query">Search saved chats</label>
    <input
      id="session-picker-query"
      bind:value={query}
      type="search"
      autocomplete="off"
      placeholder="Search chat titles and messages…"
      class="h-9 flex-1 rounded-md border border-ink-700 bg-ink-950 px-2 text-sm text-ink-100 placeholder:text-ink-400"
    />
    <Button type="submit" variant="primary" disabled={busy || !query.trim()}>
      {busy ? "Searching…" : "Search"}
    </Button>
    <Button variant="subtle" disabled={busy} onclick={() => void reindex()} title="Rebuild the local chat index">
      <Icon name="refresh" class="h-4 w-4" />Reindex
    </Button>
  </form>
  {#if error}<p class="mt-3 rounded-md bg-rose-500/10 p-2 text-sm text-rose-300" role="alert">{error}</p>{/if}

  {#if chats.tabs.length > 0}
    <h3 class="mt-5 text-xs font-semibold uppercase tracking-wide text-ink-500">Open in tabs</h3>
    <ul class="mt-2 space-y-1.5" aria-label="Open chat tabs">
      {#each chats.tabs as tab (tab.key)}
        <li class="flex items-center gap-2 rounded-lg border border-ember-500/25 bg-ember-500/5 px-3 py-2">
          <Icon name="chat" class="h-4 w-4 shrink-0 text-ember-300" />
          <span class="min-w-0 flex-1 truncate text-sm text-ink-100">{tab.title || "New chat"}</span>
          {#if tab.busy}
            <span class="session-busy" title="This chat is working" aria-label="Busy"></span>
          {/if}
          <span class="shrink-0 text-xs font-medium text-ember-200">Open in tab</span>
          {#if tab.id}<Button variant="subtle" onclick={() => onOpen({ thread_id: String(tab.id ?? ""), title: tab.title, snippet: "", path: "" })}>Focus</Button>{/if}
        </li>
      {/each}
    </ul>
  {/if}

  {#if searched}
    {#if hits.length > 0}
      <h3 class="mt-5 text-xs font-semibold uppercase tracking-wide text-ink-500">Search results</h3>
      <ul class="mt-2 space-y-1.5" aria-label="Session search results">
        {#each hits as hit (hit.thread_id + hit.path)}
          <li class="flex items-center gap-2 rounded-lg border border-ink-700 bg-ink-950 px-3 py-2">
            <Icon name="chat" class="h-4 w-4 shrink-0 text-ember-300" />
            <span class="min-w-0 flex-1">
              <span class="block truncate text-sm font-medium text-ink-100">{hit.title || "Untitled"}</span>
              <span class="block truncate text-xs text-ink-500">{hit.snippet}</span>
            </span>
            {#if busyFor(hit.thread_id)}<span class="session-busy" title="This chat is working" aria-label="Busy"></span>{/if}
            <span class="shrink-0 text-xs text-ink-500">{tabFor(hit.thread_id) ? "Open in tab" : "Saved"}</span>
            <Button variant="subtle" onclick={() => onOpen(hit)}>Open</Button>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="mt-5 text-sm text-ink-400">No chats matched that search.</p>
    {/if}
  {/if}

  <h3 class="mt-5 text-xs font-semibold uppercase tracking-wide text-ink-500">Saved chats</h3>
  {#if saved.length === 0}
    <p class="mt-2 text-sm text-ink-400">No saved chats yet. Chats appear here after their first answer.</p>
  {:else}
    <ul class="mt-2 max-h-72 space-y-1.5 overflow-auto" aria-label="Saved chats">
      {#each saved as thread (thread.id)}
        <li class="flex items-center gap-2 rounded-lg border border-ink-700 bg-ink-950 px-3 py-2">
          <Icon name="chat" class="h-4 w-4 shrink-0 text-ink-300" />
          <span class="min-w-0 flex-1 truncate text-sm text-ink-100">{thread.title || "Untitled"}</span>
          {#if busyFor(thread.id)}<span class="session-busy" title="This chat is working" aria-label="Busy"></span>{/if}
          <span class="shrink-0 text-xs text-ink-500">{tabFor(thread.id) ? "Open in tab" : `${thread.message_count} msgs`}</span>
          <Button variant="subtle" onclick={() => openThread(thread)}>{tabFor(thread.id) ? "Open in tab" : "Open"}</Button>
        </li>
      {/each}
    </ul>
  {/if}
</Modal>

<style>
  .session-busy {
    display: inline-block;
    height: 8px;
    width: 8px;
    flex: none;
    border-radius: 9999px;
    background: var(--color-ember-400, #fb923c);
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--color-ember-500) 22%, transparent);
    animation: session-pulse 1.4s ease-in-out infinite;
  }
  @keyframes session-pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.35; }
  }
  @media (prefers-reduced-motion: reduce) {
    .session-busy { animation: none; }
  }
</style>
