<script lang="ts">
  // Search saved chats with the local FTS index. Nothing leaves this computer.
  import Modal from "./Modal.svelte";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import type { SessionHit } from "../lib/api/bindings";

  let {
    onClose,
    onOpen,
  }: {
    onClose: () => void;
    /** Called with the hit the user chose; the parent navigates and opens its tab. */
    onOpen: (hit: SessionHit) => void;
  } = $props();

  let query = $state("");
  let hits = $state<SessionHit[]>([]);
  let busy = $state(false);
  let searched = $state(false);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);

  const search = async () => {
    const q = query.trim();
    if (!q || busy) return;
    busy = true;
    error = null;
    notice = null;
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

  // The index starts empty until a full rebuild; offer it next to the results.
  const reindex = async () => {
    if (busy) return;
    busy = true;
    error = null;
    notice = null;
    try {
      const count = await api.sessionReindex();
      notice = `Indexed ${count} ${count === 1 ? "chat" : "chats"}.`;
      if (query.trim()) await search();
    } catch (e: unknown) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  };
</script>

<Modal title="Search chats" onClose={onClose} size="lg">
  <form class="flex items-center gap-2" onsubmit={(e) => { e.preventDefault(); void search(); }}>
    <label class="sr-only" for="chat-search-query">Search saved chats</label>
    <input
      id="chat-search-query"
      bind:value={query}
      type="search"
      autocomplete="off"
      placeholder="Words from an old conversation…"
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
  {#if notice}<p class="mt-3 text-sm text-emerald-300" role="status">{notice}</p>{/if}

  {#if !searched}
    <p class="mt-5 text-sm text-ink-400">Search looks through the titles and messages of your saved chats. Rebuild the index first if you have older conversations.</p>
  {:else if hits.length === 0}
    <p class="mt-5 text-sm text-ink-400">No chats matched that search.</p>
  {:else}
    <ul class="mt-4 space-y-2" aria-label="Chat search results">
      {#each hits as hit (hit.thread_id + hit.path)}
        <li class="rounded-lg border border-ink-700 bg-ink-950 p-3">
          <div class="flex flex-wrap items-start justify-between gap-3">
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-2 font-medium text-ink-100">
                <Icon name="chat" class="h-4 w-4 shrink-0 text-ember-300" />
                <span class="truncate">{hit.title || "Untitled"}</span>
              </div>
              <p class="mt-1 break-words text-sm text-ink-300">{hit.snippet}</p>
              <p class="mt-1 truncate text-xs text-ink-500" title={hit.path}>{hit.path}</p>
            </div>
            <Button variant="secondary" onclick={() => onOpen(hit)}>Open</Button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</Modal>
