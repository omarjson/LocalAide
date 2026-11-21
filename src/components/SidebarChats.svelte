<script lang="ts">
  // Saved chats in the sidebar, under Chat: chats that work in a project
  // folder are grouped by folder, plain chats come after them.
  import { slide } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import ConfirmDialog from "./ConfirmDialog.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import { chats, deleteChat, forgetFolder, groupChats, newChat, newChatInFolder, openChat } from "../lib/chats.svelte";
  import { dur, motion } from "../lib/motion";
  import { portal } from "../lib/portal";
  import { fileManagerName } from "../lib/platform";
  import { showToast } from "../lib/toast.svelte";
  import type { ChatThread } from "../lib/api/bindings";

  let { onOpen }: { onOpen?: () => void } = $props();

  let query = $state("");
  let closed = $state<Record<string, boolean>>({});
  let more = $state<Record<string, boolean>>({});
  let pendingDelete = $state<ChatThread | null>(null);

  const groups = $derived(groupChats(chats.threads, chats.folders, query));
  const LIMIT = 6;

  const open = (t: ChatThread) => {
    if (chats.busy) {
      showToast("Wait for the answer to finish, or stop it first.");
      return;
    }
    openChat(t);
    onOpen?.();
  };
  const start = (folder: string | null) => {
    if (chats.busy) {
      showToast("Wait for the answer to finish, or stop it first.");
      return;
    }
    newChat(folder);
    onOpen?.();
  };
  const startInFolder = async () => {
    try {
      if (await newChatInFolder()) onOpen?.();
    } catch (e: unknown) {
      showToast(`The folder could not be opened: ${errorMessage(e)}`, "error");
    }
  };
  const confirmDelete = async () => {
    const t = pendingDelete;
    pendingDelete = null;
    if (!t) return;
    try {
      await deleteChat(t.id);
    } catch (e: unknown) {
      showToast(`That chat could not be deleted: ${errorMessage(e)}`, "error");
    }
  };
</script>

{#snippet row(t: ChatThread)}
  <div class={`chat-row group ${t.id === chats.activeId ? "chat-row-on" : ""}`}>
    <button type="button" class="min-w-0 flex-1 truncate py-1.5 pl-2 text-left" aria-current={t.id === chats.activeId ? "true" : undefined} onclick={() => open(t)} title={t.title}>{t.title}</button>
    <button type="button" class="chat-row-del" aria-label={`Delete chat ${t.title}`} title="Delete chat" onclick={() => (pendingDelete = t)}><Icon name="trash" class="h-3.5 w-3.5" /></button>
  </div>
{/snippet}

<div class="mt-2 space-y-0.5 pb-2" aria-label="Saved chats" role="group">
  <div class="flex items-center gap-1 px-2 pb-1 pt-2">
    <span class="flex-1 text-xs font-semibold text-ink-500">Chats</span>
    <button type="button" class="side-icon" aria-label="New chat" title="New chat (Ctrl+N)" onclick={() => start(null)}><Icon name="plus" class="h-3.5 w-3.5" /></button>
    <button type="button" class="side-icon" aria-label="New chat in a folder" title="New chat in a project folder" onclick={() => void startInFolder()}><Icon name="folder-plus" class="h-3.5 w-3.5" /></button>
  </div>
  {#if chats.threads.length > 8}
    <div class="px-1.5 pb-1">
      <input type="search" bind:value={query} aria-label="Find a chat" placeholder="Find a chat" class="h-7 w-full rounded-md border border-ink-800 bg-ink-900 px-2 text-[13px] text-ink-100 placeholder:text-ink-500" />
    </div>
  {/if}

  {#each groups.folders as g (g.path)}
    <div>
      <div class="folder-row group">
        <button type="button" class="flex min-w-0 flex-1 items-center gap-1.5 py-1.5 pl-1.5 text-left" aria-expanded={!closed[g.path]} onclick={() => (closed[g.path] = !closed[g.path])} title={g.path}>
          <Icon name={closed[g.path] ? "folder" : "folder-open"} class="h-3.5 w-3.5 shrink-0 text-ink-400" />
          <span class="truncate font-medium">{g.name}</span>
          <span class="text-xs tabular-nums text-ink-600">{g.threads.length || ""}</span>
        </button>
        <button type="button" class="chat-row-del" aria-label={`New chat in ${g.name}`} title={`New chat in ${g.name}`} onclick={() => start(g.path)}><Icon name="plus" class="h-3.5 w-3.5" /></button>
        <button type="button" class="chat-row-del" aria-label={`Show ${g.name} in ${fileManagerName()}`} title={`Show in ${fileManagerName()}`} onclick={() => void api.revealFile("", g.path).catch((e: unknown) => showToast(errorMessage(e), "error"))}><Icon name="external" class="h-3.5 w-3.5" /></button>
        {#if g.threads.length === 0}
          <button type="button" class="chat-row-del" aria-label={`Remove ${g.name} from the list`} title="Remove from the list" onclick={() => forgetFolder(g.path)}><Icon name="close" class="h-3.5 w-3.5" /></button>
        {/if}
      </div>
      {#if !closed[g.path]}
        <div class="ml-3 border-l border-ink-800 pl-1" transition:slide={{ duration: dur(motion.fast) }}>
          {#each g.threads.slice(0, more[g.path] ? undefined : LIMIT) as t (t.id)}{@render row(t)}{/each}
          {#if g.threads.length > LIMIT && !more[g.path]}
            <button type="button" class="px-2 py-1 text-xs text-ink-500 hover:text-ink-200" onclick={() => (more[g.path] = true)}>Show {g.threads.length - LIMIT} more</button>
          {/if}
          {#if g.threads.length === 0}<p class="px-2 py-1 text-xs text-ink-600">No chats yet</p>{/if}
        </div>
      {/if}
    </div>
  {/each}

  {#if groups.folders.length > 0 && groups.plain.length > 0}
    <div class="px-2 pb-0.5 pt-2 text-xs font-semibold text-ink-600">Plain chats</div>
  {/if}
  {#each groups.plain.slice(0, more[""] ? undefined : 12) as t (t.id)}{@render row(t)}{/each}
  {#if groups.plain.length > 12 && !more[""]}
    <button type="button" class="px-2 py-1 text-xs text-ink-500 hover:text-ink-200" onclick={() => (more[""] = true)}>Show {groups.plain.length - 12} more</button>
  {/if}
  {#if chats.threads.length === 0 && groups.folders.length === 0}
    <p class="px-2 py-1 text-xs leading-5 text-ink-600">Chats save themselves after the first answer.</p>
  {:else if query && groups.plain.length === 0 && groups.folders.length === 0}
    <p class="px-2 py-1 text-xs text-ink-600">No chat matches.</p>
  {/if}
</div>

{#if pendingDelete}
  <!-- The sidebar animates with a transform, which traps fixed-position
       children; the dialog renders at the document root instead. -->
  <div use:portal>
    <ConfirmDialog
      title="Delete this chat?"
      message={`“${pendingDelete.title}” is removed from this computer. You cannot undo it.`}
      confirmLabel="Delete chat"
      onConfirm={() => void confirmDelete()}
      onCancel={() => (pendingDelete = null)}
    />
  </div>
{/if}

<style>
  .side-icon {
    display: grid;
    height: 1.5rem;
    width: 1.5rem;
    place-items: center;
    border-radius: 0.4rem;
    color: var(--color-ink-500);
    transition: background-color 120ms ease-out, color 120ms ease-out;
  }
  .side-icon:hover {
    background: color-mix(in oklab, var(--color-veil) 7%, transparent);
    color: var(--color-ink-100);
  }
  .chat-row,
  .folder-row {
    display: flex;
    align-items: center;
    border-radius: 0.45rem;
    font-size: 0.8125rem;
    color: var(--color-ink-300);
    transition: background-color 120ms ease-out, color 120ms ease-out;
  }
  .folder-row {
    color: var(--color-ink-200);
  }
  .chat-row:hover,
  .folder-row:hover {
    background: color-mix(in oklab, var(--color-veil) 5%, transparent);
    color: var(--color-ink-50);
  }
  .chat-row-on {
    background: var(--color-ink-800);
    color: var(--color-ink-50);
  }
  .chat-row-del {
    display: grid;
    height: 1.5rem;
    width: 1.5rem;
    flex-shrink: 0;
    place-items: center;
    border-radius: 0.35rem;
    color: var(--color-ink-500);
    opacity: 0;
    transition: opacity 120ms ease-out, color 120ms ease-out;
  }
  .chat-row:hover .chat-row-del,
  .folder-row:hover .chat-row-del,
  .chat-row-del:focus-visible {
    opacity: 1;
  }
  .chat-row-del:hover {
    color: var(--color-ink-50);
  }
</style>
