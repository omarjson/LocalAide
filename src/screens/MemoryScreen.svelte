<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  type Note = { path: string; title: string; updated: string; preview: string };

  let notes = $state<Note[]>([]);
  let query = $state("");
  let loading = $state(true);
  let showNew = $state(false);
  let newTitle = $state("");
  let newContent = $state("");
  let searching = $state(false);
  let selectedNote = $state<Note | null>(null);
  let selectedContent = $state("");
  let viewing = $state(false);
  let editing = $state(false);

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
    try {
      const raw = await invoke<any[]>("memory_list");
      const notesData = await Promise.all(raw.map(async r => {
        const name = r.path.split(/[\\/]/).pop()?.replace(/\.md$/, "") ?? "Untitled";
        let preview = "Markdown note with vector recall.";
        try {
          const content = await invoke<string>("memory_read", { path: r.path });
          const firstLine = content.split("\n").find(l => l.trim().length > 0) ?? "";
          const body = content.replace(firstLine, "").trim();
          preview = body.slice(0, 160) || firstLine.slice(0, 160);
        } catch {}
        return {
          path: r.path,
          title: name,
          updated: fmtDate(r.modified),
          preview
        };
      }));
      notes = notesData;
    } catch {
      notes = [];
    } finally {
      loading = false;
    }
  };

  const createNote = async () => {
    if (!newTitle.trim()) return;
    await invoke("memory_create", { title: newTitle.trim(), content: newContent.trim() });
    showNew = false;
    newTitle = "";
    newContent = "";
    await load();
  };

  const openNote = async (n: Note) => {
    selectedNote = n;
    try {
      selectedContent = await invoke<string>("memory_read", { path: n.path });
    } catch {
      selectedContent = "";
    }
    viewing = true;
    editing = false;
  };

  const saveNote = async () => {
    if (!selectedNote) return;
    try {
      await invoke("memory_update", { path: selectedNote.path, content: selectedContent });
      editing = false;
      await load();
    } catch {}
  };

  onMount(load);

  $effect(async () => {
    if (!query.trim() || query.length < 3) return;
    searching = true;
    try {
      const res = await invoke<any[]>("memory_search", { query });
      const mapped = await Promise.all(res.map(async r => {
        const path = r.path as string;
        const name = path.split(/[\\/]/).pop()?.replace(/\.md$/, "") ?? "Untitled";
        let preview = "";
        try {
          const content = await invoke<string>("memory_read", { path });
          preview = content.slice(0, 160);
        } catch {}
        return {
          path,
          title: name,
          updated: fmtDate(r.modified),
          preview
        };
      }));
      notes = mapped;
    } catch {
      // fallback to local filter
    } finally {
      searching = false;
    }
  });

  const filtered = $derived(query.trim().length >= 3 ? notes : notes.filter(n => n.title.toLowerCase().includes(query.toLowerCase())));
</script>

<div class="max-w-3xl mx-auto">
  <div class="flex items-center justify-between pb-4 mb-6 border-b border-zinc-800">
    <div>
      <h2 class="text-lg font-semibold tracking-tight">Memory</h2>
      <p class="text-xs text-zinc-500">Markdown notes with vector recall</p>
    </div>
    <button onclick={() => showNew = true} class="px-4 py-2 rounded-xl bg-gradient-to-r from-fuchsia-600 to-cyan-500 text-white text-sm font-medium">New note</button>
  </div>

  <div class="mb-4">
    <input bind:value={query} placeholder="Search notes…" class="w-full bg-zinc-950 border border-zinc-800 rounded-xl px-4 py-2.5 text-sm text-zinc-100 placeholder-zinc-500 focus:outline-none focus:border-fuchsia-500"/>
  </div>

  {#if loading}
    <div class="space-y-3">
      {#each Array(3) as _,i}
        <div class="h-24 rounded-2xl bg-zinc-900/40 animate-pulse"/>
      {/each}
    </div>
  {:else if filtered.length === 0}
    <div class="border border-dashed border-zinc-800 rounded-2xl p-12 text-center">
      <div class="text-4xl mb-3">🧠</div>
      <div class="text-zinc-300 font-medium">No notes found</div>
      <div class="text-sm text-zinc-500 mt-1">Create your first memory note to start building vector recall.</div>
    </div>
  {:else}
    <div class="space-y-3">
      {#each filtered as n}
        <div onclick={() => openNote(n)} class="group border border-zinc-800 rounded-2xl p-4 bg-zinc-900/60 hover:border-zinc-700 transition cursor-pointer">
          <div class="flex justify-between items-start">
            <div class="font-medium text-zinc-100">{n.title}</div>
            <span class="text-xs text-zinc-500">{n.updated}</span>
          </div>
          <p class="mt-2 text-sm text-zinc-400 line-clamp-2">{n.preview}</p>
          <div class="mt-3 flex gap-2 opacity-0 group-hover:opacity-100 transition">
            <span class="text-[10px] px-2 py-0.5 rounded-full border border-zinc-700 text-zinc-400">vector</span>
            <span class="text-[10px] px-2 py-0.5 rounded-full border border-zinc-700 text-zinc-400">recall</span>
            <span class="text-[10px] px-2 py-0.5 rounded-full border border-zinc-700 text-zinc-400 ml-auto">Open</span>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

{#if showNew}
<div class="fixed inset-0 bg-black/60 backdrop-blur-sm grid place-items-center p-4">
  <div class="w-full max-w-xl rounded-2xl border border-zinc-800 bg-zinc-950 p-6 shadow-2xl">
    <h3 class="text-lg font-semibold mb-4">New memory note</h3>
    <input bind:value={newTitle} placeholder="Title" class="w-full bg-zinc-900 border border-zinc-800 rounded-xl px-4 py-2.5 mb-3 text-sm focus:outline-none focus:border-fuchsia-500"/>
    <textarea bind:value={newContent} placeholder="Content (markdown)" rows="8" class="w-full bg-zinc-900 border border-zinc-800 rounded-xl px-4 py-2.5 text-sm focus:outline-none focus:border-fuchsia-500"/>
    <div class="flex justify-end gap-2 mt-4">
      <button onclick={() => showNew = false} class="px-4 py-2 rounded-xl bg-white/5 border border-zinc-800 text-sm">Cancel</button>
      <button onclick={createNote} class="px-4 py-2 rounded-xl bg-gradient-to-r from-fuchsia-600 to-cyan-500 text-white text-sm font-medium">Save</button>
    </div>
  </div>
</div>
{/if}

{#if viewing && selectedNote}
<div class="fixed inset-0 bg-black/70 backdrop-blur-sm grid place-items-center p-4 z-50">
  <div class="w-full max-w-2xl rounded-2xl border border-zinc-800 bg-zinc-950 shadow-2xl flex flex-col max-h-[80vh]">
    <div class="flex items-center justify-between p-5 border-b border-zinc-800">
      <div>
        <h3 class="text-lg font-semibold">{selectedNote.title}</h3>
        <p class="text-xs text-zinc-500 mt-0.5">Updated {selectedNote.updated} • Vector recall</p>
      </div>
      <div class="flex items-center gap-2">
        {#if !editing}
          <button onclick={() => editing = true} class="px-3 py-1.5 rounded-lg bg-white/5 border border-zinc-800 text-xs text-zinc-300 hover:bg-white/10">Edit</button>
        {:else}
          <button onclick={saveNote} class="px-3 py-1.5 rounded-lg bg-emerald-600/20 border border-emerald-500/30 text-xs text-emerald-200 hover:bg-emerald-600/30">Save</button>
          <button onclick={() => editing = false} class="px-3 py-1.5 rounded-lg bg-white/5 border border-zinc-800 text-xs text-zinc-300 hover:bg-white/10">Cancel</button>
        {/if}
        <button onclick={() => viewing = false} class="px-3 py-1.5 rounded-lg bg-white/5 border border-zinc-800 text-xs text-zinc-300 hover:bg-white/10">Close</button>
      </div>
    </div>
    <div class="p-5 overflow-auto flex-1">
      {#if editing}
        <textarea bind:value={selectedContent} rows="20" class="w-full h-full bg-zinc-900 border border-zinc-800 rounded-xl p-3 text-sm text-zinc-100 focus:outline-none focus:border-fuchsia-500 resize-none"/>
      {:else}
        <div class="text-sm text-zinc-200 whitespace-pre-wrap leading-relaxed">{selectedContent}</div>
      {/if}
    </div>
  </div>
</div>
{/if}

