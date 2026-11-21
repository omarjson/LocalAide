<script lang="ts">
  import { onMount } from "svelte";
  import { fade, slide } from "svelte/transition";
  import Button from "../components/Button.svelte";
  import ConfirmDialog from "../components/ConfirmDialog.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import ErrorBanner from "../components/ErrorBanner.svelte";
  import Icon from "../components/Icon.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import TextField from "../components/TextField.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import { showToast } from "../lib/toast.svelte";
  import { formatBytes } from "../lib/format";
  import type { MemoryNote } from "../lib/api/bindings";

  type NoteView = {
    id?: string;
    path: string;
    title: string;
    tags: string[];
    updated: string;
    preview: string;
    size: number;
    modified: number;
    pinned: boolean;
  };

  type SuggestionView = {
    file: string;
    title: string;
    content: string;
  };

  let notes = $state<NoteView[]>([]);
  let query = $state("");
  let selectedTag = $state<string>("All");
  let loading = $state(true);
  let showNew = $state(false);
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
  let pendingDeletePath = $state<string | null>(null);
  let showForgetAllConfirm = $state(false);
  let notesDirty = $state(false);
  let suggestions = $state<SuggestionView[]>([]);
  let exporting = $state(false);

  const fmtDate = (mod?: number) => {
    if (!mod) return "—";
    const date = new Date(mod * 1000);
    const diff = (Date.now() - date.getTime()) / 86400000;
    if (diff < 1) return "Today";
    if (diff < 2) return "Yesterday";
    return date.toLocaleDateString();
  };

  const load = async () => {
    loading = true;
    errorMsg = null;
    try {
      const raw = await api.listMemories();
      notes = raw.map((note: MemoryNote): NoteView => {
        const title = note.title || (note.path.split(/[\\/]/).pop()?.replace(/\.md$/, "") ?? "Untitled");
        const content = note.content || "";
        return {
          id: note.id,
          path: note.path,
          title,
          tags: note.tags || [],
          updated: fmtDate(note.modified),
          preview: content.slice(0, 160) || "A saved note.",
          size: note.size,
          modified: note.modified,
          pinned: note.pinned === true,
        };
      });
      const waiting = await api.pendingSuggestions().catch(() => []);
      suggestions = waiting.map((suggestion) => ({
        file: suggestion.file,
        title: suggestion.title,
        content: suggestion.content,
      }));
    } catch (e: unknown) {
      errorMsg = errorMessage(e);
      notes = [];
    } finally {
      loading = false;
    }
  };

  const createNote = async () => {
    if (!newTitle.trim()) return;
    const tagList = newTags.split(",").map((tag) => tag.trim()).filter(Boolean);
    try {
      await api.createMemory({ title: newTitle.trim(), content: newContent.trim(), tags: tagList });
      showNew = false;
      newTitle = "";
      newContent = "";
      newTags = "";
      await load();
    } catch (e: unknown) {
      errorMsg = errorMessage(e);
    }
  };

  const openNote = async (note: NoteView) => {
    selectedNote = note;
    selectedTagsEdit = note.tags.join(", ");
    errorMsg = null;
    try {
      selectedContent = await api.readMemory(note.path);
      viewing = true;
      editing = false;
    } catch (e: unknown) {
      errorMsg = errorMessage(e);
      viewing = false;
    }
  };

  const saveNote = async () => {
    if (!selectedNote) return;
    const tagList = selectedTagsEdit.split(",").map((tag) => tag.trim()).filter(Boolean);
    try {
      await api.updateMemory({ path: selectedNote.path, content: selectedContent, tags: tagList });
      selectedNote.tags = tagList;
      editing = false;
      await load();
    } catch (e: unknown) {
      errorMsg = errorMessage(e);
    }
  };

  const deleteNote = async (path: string) => {
    errorMsg = null;
    try {
      await api.deleteMemory(path);
      if (selectedNote?.path === path) {
        viewing = false;
        selectedNote = null;
      }
      notes = notes.filter((note) => note.path !== path);
    } catch (e: unknown) {
      errorMsg = errorMessage(e);
    }
  };

  const askDelete = (path: string) => {
    pendingDeletePath = path;
  };

  const togglePin = async (note: NoteView) => {
    errorMsg = null;
    try {
      const updated = await api.setMemoryPinned(note.path, !note.pinned);
      notes = notes.map((n) => (n.path === note.path ? { ...n, pinned: updated.pinned === true } : n));
    } catch (e: unknown) {
      errorMsg = errorMessage(e);
    }
  };

  const acceptSuggestion = async (file: string) => {
    errorMsg = null;
    try {
      await api.acceptSuggestion(file);
      suggestions = suggestions.filter((s) => s.file !== file);
      await load();
      showToast("Note saved.");
    } catch (e: unknown) {
      errorMsg = errorMessage(e);
    }
  };

  const dismissSuggestion = async (file: string) => {
    errorMsg = null;
    try {
      await api.dismissSuggestion(file);
      suggestions = suggestions.filter((s) => s.file !== file);
    } catch (e: unknown) {
      errorMsg = errorMessage(e);
    }
  };

  const exportAll = async () => {
    if (notes.length === 0 || exporting) return;
    exporting = true;
    try {
      const body = notes
        .map((note) => `# ${note.title}\n\n${note.preview}${note.preview.length >= 160 ? "…" : ""}\n`)
        .join("\n---\n\n");
      const header = `# LocalAide notes\n\nExported ${new Date().toLocaleString()} · ${notes.length} notes\n\n`;
      const saved = await api.saveChatMarkdown("localaide-notes.md", header + body);
      if (saved) showToast("Notes exported.");
    } catch (e: unknown) {
      errorMsg = errorMessage(e);
    } finally {
      exporting = false;
    }
  };

  const confirmDelete = async () => {
    const path = pendingDeletePath;
    pendingDeletePath = null;
    if (path) await deleteNote(path);
  };

  const executeForgetAll = async () => {
    errorMsg = null;
    try {
      await api.forgetAllMemories();
      showForgetAllConfirm = false;
      viewing = false;
      selectedNote = null;
      notes = [];
    } catch (e: unknown) {
      errorMsg = errorMessage(e);
    }
  };

  onMount(load);

  $effect(() => {
    if (!query.trim() || query.length < 3) {
      // A search cut short by editing the query must not stay "searching".
      searching = false;
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
        const found = await api.searchMemories(query);
        if (!active) return;
        notes = found.map((result) => {
          const path = result.path || "";
          const title = result.title || (path.split(/[\\/]/).pop()?.replace(/\.md$/, "") ?? "Untitled");
          return {
            path,
            title,
            tags: [],
            updated: fmtDate(result.modified || 0),
            preview: result.snippet || "",
            size: result.size,
            modified: result.modified || 0,
            pinned: false,
          };
        });
        notesDirty = true;
      } catch (e: unknown) {
        if (active) errorMsg = errorMessage(e);
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
    const tagSet = new Set<string>();
    for (const note of notes) for (const tag of note.tags) tagSet.add(tag);
    return ["All", ...Array.from(tagSet)];
  });

  // One glance: how many notes there are and how much room they take.
  const totals = $derived.by(() => {
    const count = searching ? notes.length : filteredNotes.length;
    const bytes = (searching ? notes : filteredNotes).reduce((sum, note) => sum + (note.size || 0), 0);
    return { count, bytes };
  });

  const filteredNotes = $derived.by<NoteView[]>(() => {
    let result = notes;
    if (selectedTag !== "All") result = result.filter((note) => note.tags.includes(selectedTag));
    if (query.trim().length > 0 && query.trim().length < 3) {
      result = result.filter((note) => note.title.toLowerCase().includes(query.toLowerCase()) || note.tags.some((tag) => tag.toLowerCase().includes(query.toLowerCase())));
    }
    return result;
  });

  const pinnedNotes = $derived(filteredNotes.filter((note) => note.pinned));
  const unpinnedNotes = $derived(filteredNotes.filter((note) => !note.pinned));

  // The timeline: unpinned notes grouped by month, newest first.
  const timeline = $derived.by<{ label: string; notes: NoteView[] }[]>(() => {
    const months = new Map<string, NoteView[]>();
    for (const note of unpinnedNotes) {
      const date = new Date(note.modified * 1000);
      const label = date.toLocaleDateString(undefined, { month: "long", year: "numeric" });
      const bucket = months.get(label);
      if (bucket) bucket.push(note);
      else months.set(label, [note]);
    }
    return Array.from(months, ([label, list]) => ({ label, notes: list }));
  });
</script>

<div class="mx-auto max-w-5xl pb-10">
  <PageHeader title="Memory" description="Save notes that LocalAide can recall during future chats.">
    {#snippet actions()}
      <Button variant="secondary" disabled={notes.length === 0 || exporting} onclick={() => void exportAll()}>Export all</Button>
      <Button variant="danger" disabled={notes.length === 0} onclick={() => showForgetAllConfirm = true}>Forget all</Button>
      <Button variant="primary" onclick={() => showNew = !showNew}><Icon name="plus" class="h-[18px] w-[18px]" />New note</Button>
    {/snippet}
  </PageHeader>

  {#if errorMsg}
    <div class="mb-4"><ErrorBanner message={errorMsg} onRetry={() => void load()} onDismiss={() => errorMsg = null} /></div>
  {/if}

  {#if suggestions.length > 0}
    <div class="mb-5 rounded-lg border border-ember-500/40 bg-ember-500/5 p-4" transition:slide={{ duration: 150 }}>
      <div class="flex items-center justify-between">
        <h2 class="flex items-center gap-2 font-semibold text-ink-50">
          Review suggestions
          <span class="rounded-full border border-ember-500/40 bg-ember-500/10 px-2 py-0.5 text-sm text-ember-200">{suggestions.length}</span>
        </h2>
        <p class="text-sm text-ink-400">The model noticed these while chatting.</p>
      </div>
      <div class="mt-3 space-y-2">
        {#each suggestions as suggestion (suggestion.file)}
          <div class="rounded-md border border-ink-700 bg-ink-900 p-3">
            <div class="flex items-start justify-between gap-3">
              <div class="min-w-0">
                <p class="truncate font-medium text-ink-100">{suggestion.title}</p>
                <p class="mt-1 line-clamp-2 text-sm leading-6 text-ink-400">{suggestion.content}</p>
              </div>
              <div class="flex shrink-0 items-center gap-2">
                <Button variant="primary" onclick={() => void acceptSuggestion(suggestion.file)}>Save</Button>
                <Button variant="subtle" onclick={() => void dismissSuggestion(suggestion.file)}>Dismiss</Button>
              </div>
            </div>
          </div>
        {/each}
      </div>
    </div>
  {/if}

  {#if showNew}
    <div class="mb-5 rounded-lg border border-ink-700 bg-ink-900 p-4" transition:slide={{ duration: 150 }}>
      <div class="flex items-center justify-between">
        <h2 class="font-semibold text-ink-50">New note</h2>
        <Button variant="subtle" ariaLabel="Close new note" onclick={() => showNew = false}><Icon name="close" class="h-4 w-4" /></Button>
      </div>
      <div class="mt-4 space-y-3">
        <TextField bind:value={newTitle} label="Title" placeholder="Example: Project meeting notes" />
        <TextField bind:value={newTags} label="Tags" placeholder="Work, home, plans" />
        <label class="block text-sm text-ink-400">Note<textarea rows={6} placeholder="Write the note here…" bind:value={newContent} class="mt-1 w-full resize-y rounded-md border border-ink-700 bg-ink-950 px-3 py-2 text-sm leading-6 text-ink-100 outline-none placeholder:text-ink-400 focus:border-edge"></textarea></label>
        <div class="flex justify-end gap-2"><Button variant="subtle" onclick={() => showNew = false}>Cancel</Button><Button variant="primary" disabled={!newTitle.trim()} onclick={() => void createNote()}>Save note</Button></div>
      </div>
    </div>
  {/if}

  {#if viewing && selectedNote}
    <div class="mb-5 rounded-lg border border-ink-700 bg-ink-900 p-5" transition:slide={{ duration: 150 }}>
      <div class="flex flex-col gap-3 border-b border-ink-800 pb-3 sm:flex-row sm:items-center sm:justify-between">
        <div class="min-w-0"><h2 class="font-semibold text-ink-50">{selectedNote.title}</h2><p class="truncate text-sm text-ink-400 font-mono">{selectedNote.path}</p></div>
        <div class="flex items-center gap-2">
          {#if editing}
            <Button variant="primary" onclick={() => void saveNote()}>Save changes</Button>
            <Button variant="subtle" onclick={() => editing = false}>Cancel</Button>
          {:else}
            <Button variant="secondary" onclick={() => editing = true}>Edit</Button>
            <Button variant="danger" onclick={() => askDelete(selectedNote!.path)}>Delete</Button>
            <Button variant="subtle" onclick={() => viewing = false}>Close</Button>
          {/if}
        </div>
      </div>
      {#if editing}
        <div class="mt-4 space-y-3">
          <TextField bind:value={selectedTagsEdit} label="Tags" placeholder="Work, home, plans" />
          <textarea rows={12} bind:value={selectedContent} class="w-full resize-y rounded-md border border-ink-700 bg-ink-950 px-3 py-2 text-sm leading-6 text-ink-100 outline-none focus:border-edge"></textarea>
        </div>
      {:else}
        {#if selectedNote.tags.length > 0}<div class="mt-4 flex flex-wrap gap-2">{#each selectedNote.tags as tag}<span class="rounded-full border border-ink-700 bg-ink-800 px-2.5 py-1 text-sm text-ember-300">#{tag}</span>{/each}</div>{/if}
        <div class="mt-4 max-h-96 overflow-y-auto rounded-md border border-ink-700 bg-ink-950 p-4"><pre class="whitespace-pre-wrap text-sm leading-6 text-ink-200">{selectedContent}</pre></div>
      {/if}
    </div>
  {/if}

  <div class="mb-5 space-y-3">
    <TextField bind:value={query} type="search" icon="search" ariaLabel="Search notes" placeholder="Search notes" />
    {#if !loading && notes.length > 0}
      <div class="flex flex-wrap items-center gap-2">
        <span class="rounded-full border border-ink-700 bg-ink-900 px-2.5 py-1 text-sm text-ink-300">{totals.count} {totals.count === 1 ? "note" : "notes"}</span>
        {#if totals.bytes > 0}
          <span class="rounded-full border border-ink-700 bg-ink-900 px-2.5 py-1 text-sm text-ink-300">{formatBytes(totals.bytes)}</span>
        {/if}
        {#if searching}
          <span class="rounded-full border border-ember-500/40 bg-ember-500/10 px-2.5 py-1 text-sm text-ember-200" role="status">Searching…</span>
        {/if}
      </div>
    {/if}
    {#if query}<Button variant="subtle" onclick={() => { query = ""; void load(); }}>Clear search</Button>{/if}
    {#if allTags.length > 1}
      <div class="flex flex-wrap items-center gap-2">{#each allTags as tag}<button type="button" onclick={() => selectedTag = tag} class={`rounded-full px-3 py-1 text-sm transition-colors ${selectedTag === tag ? "bg-ember-600 font-semibold text-on-ember" : "border border-ink-700 bg-ink-900 text-ink-400 hover:text-ink-50"}`}>{tag === "All" ? "All notes" : `#${tag}`}</button>{/each}</div>
    {/if}
  </div>

  {#if loading}
    <div class="rounded-lg border border-ink-700 bg-ink-900 px-4 py-12 text-center text-sm text-ink-400">Loading notes…</div>
  {:else if searching}
    <div class="rounded-lg border border-ink-700 bg-ink-900 px-4 py-12 text-center text-sm text-ink-400">Searching notes…</div>
  {:else if filteredNotes.length === 0}
    <EmptyState icon="memory" title={query.trim() ? "No notes match" : "No notes yet"} description={query.trim() ? "Try different words, or clear the search." : "Create a note and LocalAide can recall it in future chats."}>
      {#snippet action()}<Button variant="primary" onclick={() => showNew = true}>New note</Button>{/snippet}
    </EmptyState>
  {:else}
    <div class="space-y-3">
      {#if pinnedNotes.length > 0}
        <div class="rounded-lg border border-ember-500/40 bg-ember-500/5 p-4">
          <h2 class="flex items-center gap-2 text-sm font-semibold text-ember-200"><Icon name="pin" class="h-4 w-4" />Pinned — always in context while Recall is on</h2>
          <div class="mt-3 space-y-3">
            {#each pinnedNotes as note (note.path)}
              {#snippet pinnedRow()}
                <div class="flex items-start justify-between gap-4 rounded-md border border-ink-700 bg-ink-900 p-3">
                  <div class="min-w-0 flex-1 space-y-1.5">
                    <div class="flex flex-wrap items-center gap-2"><button type="button" onclick={() => void openNote(note)} class="truncate text-left font-semibold text-ink-50 hover:text-ember-300">{note.title}</button><span class="text-sm text-ink-400">{note.updated}</span></div>
                    <p class="line-clamp-2 text-sm leading-6 text-ink-400">{note.preview}</p>
                  </div>
                  <div class="flex shrink-0 items-center gap-2">
                    <Button variant="subtle" class="h-8 w-8 px-0" ariaLabel={`Unpin ${note.title}`} onclick={() => void togglePin(note)}><Icon name="pin" class="h-4 w-4 text-ember-300" /></Button>
                    <Button variant="danger" class="h-8 w-8 px-0" ariaLabel={`Delete ${note.title}`} onclick={() => askDelete(note.path)}><Icon name="trash" class="h-4 w-4" /></Button>
                  </div>
                </div>
              {/snippet}
              {@render pinnedRow()}
            {/each}
          </div>
        </div>
      {/if}
      {#if pinnedNotes.length > 0 && unpinnedNotes.length > 0}
        <p class="px-1 pt-2 text-sm font-medium text-ink-500">All notes</p>
      {/if}
      {#each timeline as month (month.label)}
        <div class="pt-1">
          <p class="px-1 pb-2 text-sm font-medium text-ink-500">{month.label}</p>
          <div class="space-y-3 border-l border-ink-800 pl-4">
            {#each month.notes as note (note.path)}
              <div class="relative flex items-start justify-between gap-4 rounded-lg border border-ink-700 bg-ink-900 p-4 transition-colors hover:bg-veil/5">
                <span class="absolute -left-[21px] top-6 h-2 w-2 rounded-full bg-ink-600" aria-hidden="true"></span>
                <div class="min-w-0 flex-1 space-y-2">
                  <div class="flex flex-wrap items-center gap-2"><button type="button" onclick={() => void openNote(note)} class="truncate text-left font-semibold text-ink-50 hover:text-ember-300">{note.title}</button><span class="text-sm text-ink-400">{note.updated}</span></div>
                  <p class="line-clamp-2 text-sm leading-6 text-ink-400">{note.preview}</p>
                  {#if note.tags.length > 0}<div class="flex flex-wrap gap-2">{#each note.tags as tag}<button type="button" onclick={() => selectedTag = tag} class="rounded-full bg-ink-800 px-2 py-0.5 text-sm text-ember-300 hover:bg-ink-700">#{tag}</button>{/each}</div>{/if}
                </div>
                <div class="flex shrink-0 items-center gap-2">
                  <Button variant="subtle" class="h-8 w-8 px-0" ariaLabel={`Pin ${note.title}`} onclick={() => void togglePin(note)}><Icon name="pin" class="h-4 w-4" /></Button>
                  <Button variant="secondary" onclick={() => void openNote(note)}>Open</Button>
                  <Button variant="danger" class="h-8 w-8 px-0" ariaLabel={`Delete ${note.title}`} onclick={() => askDelete(note.path)}><Icon name="trash" class="h-4 w-4" /></Button>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {/each}
      {#if timeline.length === 0 && pinnedNotes.length === 0}
        <div class="rounded-lg border border-ink-700 bg-ink-900 px-4 py-8 text-center text-sm text-ink-400">No notes match the current filter.</div>
      {/if}
    </div>
  {/if}
</div>

{#if pendingDeletePath}
  <ConfirmDialog title="Delete this note?" message="This removes the note from LocalAide and from this computer. You cannot undo it." confirmLabel="Delete note" onConfirm={() => void confirmDelete()} onCancel={() => pendingDeletePath = null} />
{/if}
{#if showForgetAllConfirm}
  <ConfirmDialog title="Forget all notes?" message={`This deletes all ${notes.length} notes from LocalAide and from this computer. You cannot undo it.`} confirmLabel="Forget all" onConfirm={() => void executeForgetAll()} onCancel={() => showForgetAllConfirm = false} />
{/if}
