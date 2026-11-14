<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import ErrorBanner from "../components/ErrorBanner.svelte";
  import { api } from "../lib/api/client";
  import { dur } from "../lib/motion";
  import type { MemoryNote } from "../lib/api/bindings";
  import { fade, scale, slide } from "svelte/transition";
  import { onMount } from "svelte";

  type NoteView = {
    id?: string;
    path: string;
    title: string;
    tags: string[];
    updated: string;
    preview: string;
    size: number;
  };

  let notes = $state<NoteView[]>([]);
  let query = $state("");
  let selectedTag = $state<string>("All");
  let loading = $state(true);
  let showNew = $state(false);
  let showForgetAllConfirm = $state(false);
  let newTitle = $state("");
  let newContent = $state("");
  let newTags = $state("");
  let searching = $state(false);
  let selectedNote = $state<NoteView | null>(null);
  let selectedContent = $state("");
  let selectedTagsEdit = $state("");
  let viewing = $state(false);
  let editing = $state(false);
  let errorMsg = $state<string | null>(null);
  // Set when search results replaced `notes`; the full list reloads when the
  // query narrows below the search threshold so stale results never linger.
  let notesDirty = $state(false);

  const fmtDate = (mod?: number) => {
    if (!mod) return "—";
    const d = new Date(mod * 1000);
    const now = Date.now();
    const diff = (now - d.getTime()) / 86400000;
    if (diff < 1) return "Today";
    if (diff < 2) return "Yesterday";
    return d.toLocaleDateString();
  };

  const load = async () => {
    loading = true;
    errorMsg = null;
    try {
      const raw = await api.listMemories();
      const notesData: NoteView[] = raw.map((r: MemoryNote) => {
        const name = r.title || (r.path.split(/[\\/]/).pop()?.replace(/\.md$/, "") ?? "Untitled");
        const contentStr = r.content || "";
        const preview = contentStr.slice(0, 160) || "Markdown memory note.";
        return {
          id: r.id,
          path: r.path,
          title: name,
          tags: r.tags || [],
          updated: fmtDate(r.modified),
          preview,
          size: r.size,
        };
      });
      notes = notesData;
    } catch (e: any) {
      errorMsg = e?.message || "Failed to load memory notes";
      notes = [];
    } finally {
      loading = false;
    }
  };

  const createNote = async () => {
    if (!newTitle.trim()) return;
    const tagList = newTags
      .split(",")
      .map(t => t.trim())
      .filter(t => t.length > 0);

    try {
      await api.createMemory({
        title: newTitle.trim(),
        content: newContent.trim(),
        tags: tagList,
      });
      showNew = false;
      newTitle = "";
      newContent = "";
      newTags = "";
      await load();
    } catch (e: any) {
      errorMsg = e?.message || "Failed to create memory note";
    }
  };

  const openNote = async (n: NoteView) => {
    selectedNote = n;
    selectedTagsEdit = n.tags.join(", ");
    try {
      selectedContent = await api.readMemory(n.path);
    } catch {
      selectedContent = "";
    }
    viewing = true;
    editing = false;
  };

  const saveNote = async () => {
    if (!selectedNote) return;
    const tagList = selectedTagsEdit
      .split(",")
      .map(t => t.trim())
      .filter(t => t.length > 0);

    try {
      await api.updateMemory({
        path: selectedNote.path,
        content: selectedContent,
        tags: tagList,
      });
      selectedNote.tags = tagList;
      editing = false;
      await load();
    } catch (e: any) {
      errorMsg = e?.message || "Failed to update note";
    }
  };

  const deleteNote = async (path: string) => {
    try {
      await api.deleteMemory(path);
      if (selectedNote?.path === path) {
        viewing = false;
        selectedNote = null;
      }
      notes = notes.filter(n => n.path !== path);
    } catch (e: any) {
      errorMsg = e?.message || "Failed to delete note";
    }
  };

  const executeForgetAll = async () => {
    try {
      await api.forgetAllMemories();
      showForgetAllConfirm = false;
      viewing = false;
      selectedNote = null;
      notes = [];
    } catch (e: any) {
      errorMsg = e?.message || "Failed to clear memory";
    }
  };

  onMount(load);

  $effect(() => {
    if (!query.trim() || query.length < 3) {
      if (notesDirty) {
        notesDirty = false;
        void load();
      }
      return;
    }
    let active = true;
    searching = true;
    const timer = setTimeout(async () => {
      try {
        const res = await api.searchMemories(query);
        if (!active) return;
        const mapped: NoteView[] = res.map((r) => {
          const path = r.path || "";
          const name = r.title || (path.split(/[\\/]/).pop()?.replace(/\.md$/, "") ?? "Untitled");
          return {
            path,
            title: name,
            tags: [],
            updated: fmtDate(r.modified || 0),
            preview: r.snippet || "",
            size: r.size,
          };
        });
        if (active) {
          notes = mapped;
          notesDirty = true;
        }
      } catch {
      } finally {
        if (active) searching = false;
      }
    }, 150);

    return () => {
      active = false;
      clearTimeout(timer);
    };
  });

  const allTags = $derived.by<string[]>(() => {
    const set = new Set<string>();
    for (const n of notes) {
      for (const t of n.tags) set.add(t);
    }
    return ["All", ...Array.from(set)];
  });

  const filteredNotes = $derived.by<NoteView[]>(() => {
    let result = notes;
    if (selectedTag !== "All") {
      result = result.filter(n => n.tags.includes(selectedTag));
    }
    if (query.trim().length > 0 && query.trim().length < 3) {
      result = result.filter(n =>
        n.title.toLowerCase().includes(query.toLowerCase()) ||
        n.tags.some(t => t.toLowerCase().includes(query.toLowerCase()))
      );
    }
    return result;
  });
</script>

<div class="max-w-4xl mx-auto space-y-6">
  <!-- Header -->
  <div class="flex items-center justify-between pb-4 border-b border-edge">
    <div>
      <h2 class="text-lg font-semibold tracking-tight text-ink-100">Knowledge & Memory</h2>
      <p class="text-xs text-ink-400">Curated notes stored in SQLite WAL and on disk. Automatically recalled with citations in chat.</p>
    </div>
    <div class="flex items-center gap-2">
      <button
        onclick={() => showForgetAllConfirm = true}
        disabled={notes.length === 0}
        class="px-2.5 py-1.5 text-xs text-rose-400 hover:text-rose-300 border border-rose-500/30 hover:border-rose-500/50 rounded-lg transition-colors disabled:opacity-30"
      >
        Forget all
      </button>
      <button
        onclick={() => showNew = !showNew}
        class="px-3 py-1.5 bg-ember-600 hover:bg-ember-500 rounded-lg text-xs font-medium text-on-ember transition-colors"
      >
        + New note
      </button>
    </div>
  </div>

  {#if errorMsg}
    <ErrorBanner message={errorMsg} onDismiss={() => errorMsg = null} />
  {/if}

  <!-- Forget All Confirmation Modal -->
  {#if showForgetAllConfirm}
    <div
      class="fixed inset-0 bg-black/70 backdrop-blur-sm z-50 flex items-center justify-center p-4"
      role="presentation"
      in:fade={{ duration: dur(100) }}
      out:fade={{ duration: dur(80) }}
      onclick={() => showForgetAllConfirm = false}
      onkeydown={(e) => { if (e.key === 'Escape') showForgetAllConfirm = false; }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Forget all memory confirmation"
        tabindex="-1"
        in:scale={{ start: 0.97, duration: dur(130) }}
        out:scale={{ start: 0.97, duration: dur(80) }}
        class="bg-ink-900 border border-rose-500/40 rounded-2xl max-w-md w-full p-6 space-y-4 shadow-2xl"
        onclick={(e) => e.stopPropagation()}
        onkeydown={(e) => e.stopPropagation()}
      >
        <div class="flex items-center gap-3 text-rose-400">
          <Icon name="alert" class="w-5 h-5" />
          <h3 class="font-semibold text-base text-ink-100">Forget All Memory?</h3>
        </div>
        <p class="text-xs text-ink-300 leading-relaxed">
          This will permanently delete all {notes.length} curated notes from SQLite and unlink all files from disk. Zero traces will remain. This action cannot be undone.
        </p>
        <div class="flex justify-end gap-2.5 pt-2">
          <button
            onclick={() => showForgetAllConfirm = false}
            class="px-3 py-1.5 rounded-lg text-xs text-ink-400 hover:text-ink-200"
          >
            Cancel
          </button>
          <button
            onclick={executeForgetAll}
            class="px-4 py-1.5 rounded-lg text-xs font-medium bg-rose-600 hover:bg-rose-500 text-white transition-colors"
          >
            Yes, forget everything
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Create Note Drawer -->
  {#if showNew}
    <div class="border border-edge rounded-2xl p-5 bg-ink-900/60 space-y-3" transition:slide={{ duration: dur(150) }}>
      <div class="font-medium text-sm text-ink-100">Create New Knowledge Note</div>
      <input
        type="text"
        placeholder="Title (e.g. Project Venus Architecture)…"
        bind:value={newTitle}
        class="w-full bg-ink-950 border border-edge rounded-lg p-2.5 text-xs text-ink-200 focus:outline-none focus:border-edge font-sans"
      />
      <input
        type="text"
        placeholder="Tags (comma separated, e.g. architecture, database)…"
        bind:value={newTags}
        class="w-full bg-ink-950 border border-edge rounded-lg p-2 text-xs text-ink-200 focus:outline-none focus:border-edge font-mono text-[11px]"
      />
      <textarea
        rows={6}
        placeholder="Content in Markdown format…"
        bind:value={newContent}
        class="w-full bg-ink-950 border border-edge rounded-lg p-2.5 text-xs text-ink-200 font-mono focus:outline-none focus:border-edge leading-relaxed"
      ></textarea>
      <div class="flex justify-end gap-2">
        <button onclick={() => showNew = false} class="px-3 py-1.5 text-xs text-ink-400 hover:text-ink-200">
          Cancel
        </button>
        <button onclick={createNote} class="px-4 py-1.5 bg-ember-600 hover:bg-ember-500 rounded-lg text-xs font-medium text-on-ember transition-colors">
          Save Note
        </button>
      </div>
    </div>
  {/if}

  <!-- View / Edit Modal/Card -->
  {#if viewing && selectedNote}
    <div class="border border-edge rounded-2xl p-6 bg-ink-900/70 space-y-4" transition:slide={{ duration: dur(150) }}>
      <div class="flex items-center justify-between pb-3 border-b border-edge">
        <div>
          <h3 class="font-semibold text-ink-100 text-sm">{selectedNote.title}</h3>
          <p class="text-[11px] font-mono text-ink-400 truncate max-w-md">{selectedNote.path}</p>
        </div>
        <div class="flex items-center gap-2">
          {#if editing}
            <button onclick={saveNote} class="px-3 py-1 bg-ember-600 hover:bg-ember-500 text-on-ember rounded text-xs font-medium">Save</button>
            <button onclick={() => editing = false} class="px-2 py-1 text-ink-400 hover:text-ink-200 text-xs">Cancel</button>
          {:else}
            <button onclick={() => editing = true} class="px-2.5 py-1 text-xs border border-edge hover:bg-ink-800 rounded text-ink-300">Edit</button>
            <button onclick={() => deleteNote(selectedNote!.path)} class="px-2.5 py-1 text-xs border border-rose-500/30 text-rose-400 hover:bg-rose-500/10 rounded">Delete</button>
            <button onclick={() => viewing = false} class="px-2.5 py-1 text-xs text-ink-400 hover:text-ink-200">Close</button>
          {/if}
        </div>
      </div>

      {#if editing}
        <input
          type="text"
          placeholder="Tags (comma separated)…"
          bind:value={selectedTagsEdit}
          class="w-full bg-ink-950 border border-edge rounded-lg p-2 text-xs text-ink-200 font-mono text-[11px]"
        />
        <textarea
          rows={12}
          bind:value={selectedContent}
          class="w-full bg-ink-950 border border-edge rounded-lg p-3 text-xs text-ink-200 font-mono focus:outline-none focus:border-edge leading-relaxed"
        ></textarea>
      {:else}
        {#if selectedNote.tags.length > 0}
          <div class="flex flex-wrap gap-1.5">
            {#each selectedNote.tags as tag}
              <span class="px-2 py-0.5 rounded bg-ink-800 text-xs font-mono text-ember-300 border border-edge">
                #{tag}
              </span>
            {/each}
          </div>
        {/if}
        <div class="bg-ink-950 border border-edge/80 rounded-xl p-4 max-h-96 overflow-y-auto">
          <pre class="text-xs text-ink-300 font-mono whitespace-pre-wrap leading-relaxed">{selectedContent}</pre>
        </div>
      {/if}
    </div>
  {/if}

  <!-- Controls & Search -->
  <div class="space-y-3">
    <div class="flex gap-2">
      <input
        type="text"
        placeholder="Search notes (Enter to search)…"
        bind:value={query}
        class="flex-1 bg-ink-900 border border-edge rounded-lg px-3 py-2 text-xs text-ink-200 placeholder-ink-500 focus:outline-none focus:border-edge font-mono"
      />
      {#if query}
        <button onclick={() => { query = ""; load(); }} class="px-3 py-2 bg-ink-800 hover:bg-ink-700 rounded-lg text-xs text-ink-300">
          Clear
        </button>
      {/if}
    </div>

    <!-- Tag Filter Pills -->
    {#if allTags.length > 1}
      <div class="flex flex-wrap items-center gap-1.5 pt-1">
        {#each allTags as tag}
          <button
            onclick={() => selectedTag = tag}
            class={`px-2.5 py-0.5 rounded-full text-[11px] font-mono transition-colors ${selectedTag === tag ? 'bg-ember-600 text-on-ember font-medium' : 'bg-ink-900 border border-edge text-ink-400 hover:text-ink-200'}`}
          >
            {tag === "All" ? "All notes" : `#${tag}`}
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <!-- Note List -->
  {#if loading}
    <div class="py-12 text-center text-xs text-ink-400 font-mono">Loading notes…</div>
  {:else if searching}
    <div class="py-12 text-center text-xs text-ink-400 font-mono">Searching knowledge notes…</div>
  {:else if filteredNotes.length === 0}
    <div class="border border-dashed border-edge rounded-2xl p-12 text-center space-y-2">
      <div class="text-xs text-ink-400">No memory notes found</div>
      <div class="text-[11px] text-ink-400">Create one above to enable automatic context recall and citations in chat.</div>
    </div>
  {:else}
    <div class="space-y-3">
      {#each filteredNotes as n}
        <div class="border border-edge rounded-xl p-4 bg-ink-900/40 hover:border-edge transition-colors flex justify-between items-start gap-4">
          <div class="space-y-1.5 flex-1 min-w-0">
            <div class="flex items-center gap-2">
              <button onclick={() => openNote(n)} class="font-medium text-sm text-ink-100 hover:text-ember-400 transition-colors text-left truncate">
                {n.title}
              </button>
              <span class="text-xs font-mono text-ink-400 flex-shrink-0">{n.updated}</span>
            </div>
            <p class="text-xs text-ink-400 line-clamp-2 leading-relaxed">{n.preview}</p>
            {#if n.tags.length > 0}
              <div class="flex flex-wrap gap-1 pt-1">
                {#each n.tags as tag}
                  <button
                    onclick={() => selectedTag = tag}
                    class="text-xs font-mono px-1.5 py-0.5 rounded bg-ink-800/80 text-ember-300 hover:bg-ink-700"
                  >
                    #{tag}
                  </button>
                {/each}
              </div>
            {/if}
          </div>
          <div class="flex items-center gap-2 flex-shrink-0">
            <button onclick={() => openNote(n)} class="px-2.5 py-1 text-xs font-mono bg-ink-800 hover:bg-ink-700 text-ink-300 rounded transition-colors">
              Open
            </button>
            <button onclick={() => deleteNote(n.path)} class="px-2 py-1 text-xs text-ink-400 hover:text-rose-400 transition-colors" title="Delete note">
              <Icon name="close" class="w-3.5 h-3.5" />
            </button>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
