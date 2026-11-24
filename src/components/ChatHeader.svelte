<script lang="ts">
  // The chat's top bar: title (click opens the menu, double-click
  // renames), the folder chip, the online/offline chip, and the chat
  // actions. Moved out of ChatScreen unchanged; the screen owns the
  // title state and the actions, this is only the markup.
  import { fly } from "svelte/transition";
  import { popover } from "../lib/actions/popover";
  import { dur, motion } from "../lib/motion";
  import { fileManagerName } from "../lib/platform";
  import { folderName } from "../lib/chats.svelte";
  import Icon from "./Icon.svelte";

  interface Props {
    /** The chat's title; "New Conversation" shows as "New chat". */
    title: string;
    editingTitle: boolean;
    titleDraft: string;
    /** Rename/regenerate buttons show: a loaded chat, not streaming. */
    canRename: boolean;
    titleBusy: boolean;
    folder: string | null;
    /** The online provider's label, or null for a local-only chat. */
    onlineLabel: string | null;
    settingsOpen: boolean;
    railOpen: boolean;
    moreOpen: boolean;
    hasMessages: boolean;
    hasTwoMessages: boolean;
    streaming: boolean;
    onTitleClick: () => void;
    onTitleDblClick: () => void;
    onStartTitleEdit: () => void;
    onSaveTitleEdit: () => void;
    onGenerateTitle: () => void;
    onToggleSettings: () => void;
    onOpenSearch: () => void;
    onToggleRail: () => void;
    onToggleMore: () => void;
    onCloseMore: () => void;
    onExportMarkdown: () => void;
    onCopy: () => void;
    onExportJson: () => void;
    onSummarize: () => void;
    onRevealFolder: () => void;
    onNewChat: () => void;
  }
  let {
    title,
    editingTitle = $bindable(),
    titleDraft = $bindable(),
    canRename,
    titleBusy,
    folder,
    onlineLabel,
    settingsOpen,
    railOpen,
    moreOpen,
    hasMessages,
    hasTwoMessages,
    streaming,
    onTitleClick,
    onTitleDblClick,
    onStartTitleEdit,
    onSaveTitleEdit,
    onGenerateTitle,
    onToggleSettings,
    onOpenSearch,
    onToggleRail,
    onToggleMore,
    onCloseMore,
    onExportMarkdown,
    onCopy,
    onExportJson,
    onSummarize,
    onRevealFolder,
    onNewChat,
  }: Props = $props();
</script>

<!-- Top bar: saved chats, the engine that answers, and chat actions. -->
<header class="relative z-20 flex h-14 shrink-0 items-center gap-2 border-b border-ink-800 bg-ink-900 px-3 md:px-5">
  <div class="flex min-w-0 items-center gap-2 px-1">
    {#if editingTitle}
      <!-- svelte-ignore a11y_autofocus -->
      <input
        bind:value={titleDraft}
        onkeydown={(e) => {
          if (e.key === "Enter") onSaveTitleEdit();
          if (e.key === "Escape") editingTitle = false;
        }}
        onblur={onSaveTitleEdit}
        aria-label="Chat title"
        placeholder="Name this chat"
        class="h-7 w-56 rounded-md border border-ember-500/50 bg-ink-950 px-2 text-sm text-ink-50 outline-none"
        autofocus
      />
    {:else}
      <div class="group/title flex min-w-0 items-center gap-1">
        <!-- The title is a button: click opens the chat menu,
             double-click renames. Hover says the full title when it is
             cut off; "New chat" stays until the model titles it. -->
        <button
          type="button"
          class="max-w-[18rem] truncate rounded-sm text-sm font-semibold text-ink-100 focus-visible:outline focus-visible:outline-2 focus-visible:outline-ember-400"
          title={title === "New Conversation" ? "New chat" : title}
          aria-label={`Chat title: ${title === "New Conversation" ? "New chat" : title}. Open the chat menu, or double-click to rename.`}
          onclick={onTitleClick}
          ondblclick={onTitleDblClick}
        >{title === "New Conversation" ? "New chat" : title}</button>
        {#if canRename}
          <button type="button" class="chat-icon-btn opacity-0 transition-opacity group-hover/title:opacity-100 focus-visible:opacity-100" aria-label="Rename this chat" title="Rename" onclick={onStartTitleEdit}><Icon name="pencil" class="h-3.5 w-3.5" /></button>
          <button type="button" class="chat-icon-btn opacity-0 transition-opacity group-hover/title:opacity-100 focus-visible:opacity-100" aria-label="Generate a title with the model" title="Generate title with the model" disabled={titleBusy} onclick={onGenerateTitle}><Icon name="refresh" class={`h-3.5 w-3.5 ${titleBusy ? "animate-spin" : ""}`} /></button>
        {/if}
      </div>
    {/if}
    {#if folder}
      <button type="button" onclick={onRevealFolder} class="folder-chip" title={`This chat works in ${folder}. Show it in ${fileManagerName()}.`}>
        <Icon name="folder" class="h-3.5 w-3.5 shrink-0" /><span class="truncate">{folderName(folder)}</span>
      </button>
    {/if}
  </div>

  <div class="ml-auto flex items-center gap-1.5">
    {#if onlineLabel}
      <span class="inline-flex min-w-0 max-w-[15rem] items-center gap-1.5 rounded-full border border-ember-500/30 bg-ember-500/10 px-2.5 py-1 text-xs font-medium text-ember-200" title={`Messages go to ${onlineLabel}.`}>
        <Icon name="cloud" class="h-3.5 w-3.5 shrink-0" /><span class="truncate">Online · {onlineLabel}</span>
      </span>
    {:else}
      <span class="inline-flex min-w-0 max-w-[15rem] items-center gap-2 rounded-full border border-ink-700 bg-veil/5 px-2.5 py-1 text-xs font-medium text-ink-300" title="Messages stay on this computer.">
        <span class="h-2 w-2 shrink-0 rounded-full bg-ember-500 shadow-[0_0_0_3px_rgba(255,122,26,0.18)]" aria-hidden="true"></span><span class="truncate">Offline · this computer</span>
      </span>
    {/if}

    <button type="button" onclick={onToggleSettings} aria-pressed={settingsOpen} aria-label="Chat settings" title="Chat settings" class="chat-icon-btn"><Icon name="sliders" class="h-[18px] w-[18px]" /></button>
    <button type="button" onclick={onOpenSearch} aria-label="Search in this chat" title="Search in this chat (Ctrl+F)" class="chat-icon-btn"><Icon name="search" class="h-[18px] w-[18px]" /></button>
    <button type="button" onclick={onToggleRail} aria-pressed={railOpen} aria-label="Chat panels" title="Chat panels — sources, work, skills (on wide screens)" class="hidden lg:inline-flex chat-icon-btn"><Icon name="list-check" class="h-[18px] w-[18px]" /></button>
    <div class="relative" use:popover={{ open: moreOpen, onClose: onCloseMore }}>
      <button type="button" aria-label="More chat actions" aria-expanded={moreOpen} aria-haspopup="menu" onclick={onToggleMore} class="chat-icon-btn"><Icon name="more" class="h-[18px] w-[18px]" /></button>
      {#if moreOpen}
        <div role="menu" aria-label="More chat actions" class="absolute right-0 top-10 z-30 min-w-52 rounded-xl border border-ink-700 bg-ink-900 p-1.5 shadow-2xl" in:fly={{ y: -4, duration: dur(motion.base), easing: motion.enter }}>
          <button type="button" role="menuitem" data-popover-item onclick={() => { onCloseMore(); onExportMarkdown(); }} disabled={!hasMessages || streaming} class="menu-item"><Icon name="download" class="h-4 w-4" />Export conversation as Markdown</button>
          <button type="button" role="menuitem" data-popover-item onclick={() => { onCloseMore(); onCopy(); }} disabled={!hasMessages || streaming} class="menu-item"><Icon name="copy" class="h-4 w-4" />Copy full conversation</button>
          <button type="button" role="menuitem" data-popover-item onclick={() => { onCloseMore(); onExportJson(); }} disabled={!hasMessages || streaming} class="menu-item"><Icon name="file" class="h-4 w-4" />Export conversation as JSON</button>
          <button type="button" role="menuitem" data-popover-item onclick={() => { onCloseMore(); onSummarize(); }} disabled={streaming || !hasTwoMessages} class="menu-item"><Icon name="file" class="h-4 w-4" />Summarize last answer</button>
          <div class="my-1 h-px bg-ink-800" role="separator"></div>
          <button type="button" role="menuitem" data-popover-item onclick={() => { onCloseMore(); onRevealFolder(); }} class="menu-item"><Icon name="folder-open" class="h-4 w-4" />Open {folder ? "the chat's folder" : "the workspace folder"} in {fileManagerName()}</button>
        </div>
      {/if}
    </div>
    <button type="button" onclick={onNewChat} class="ml-1 flex items-center gap-1.5 rounded-lg bg-ember-500 px-3 py-1.5 text-sm font-semibold text-on-ember shadow-[0_6px_18px_-8px_rgba(255,122,26,0.55)] transition-[background-color,transform] hover:bg-ember-400 active:scale-[0.97]" title="New chat (Ctrl+N)"><Icon name="plus" class="h-4 w-4" /><span class="hidden sm:inline">New chat</span></button>
  </div>
</header>

<style>
  .chat-icon-btn {
    display: grid;
    height: 2.25rem;
    width: 2.25rem;
    place-items: center;
    border-radius: 0.6rem;
    color: var(--color-ink-400);
    transition: background-color 150ms ease-out, color 150ms ease-out;
  }
  .chat-icon-btn:hover,
  .chat-icon-btn[aria-pressed="true"] {
    background: color-mix(in oklab, var(--color-veil) 6%, transparent);
    color: var(--color-ink-50);
  }
  .folder-chip {
    display: inline-flex;
    max-width: 14rem;
    align-items: center;
    gap: 0.35rem;
    border-radius: 9999px;
    border: 1px solid var(--color-ink-700);
    padding: 0.1rem 0.55rem;
    font-size: 0.75rem;
    color: var(--color-ink-300);
    transition: border-color 150ms ease-out, color 150ms ease-out;
  }
  .folder-chip:hover {
    border-color: var(--color-edge);
    color: var(--color-ink-50);
  }
  .menu-item {
    display: flex;
    width: 100%;
    align-items: center;
    gap: 0.6rem;
    border-radius: 0.5rem;
    padding: 0.5rem 0.65rem;
    text-align: left;
    font-size: 0.875rem;
    color: var(--color-ink-200);
  }
  .menu-item:hover,
  .menu-item:focus {
    background: color-mix(in oklab, var(--color-veil) 6%, transparent);
    outline: none;
  }
  .menu-item:disabled {
    opacity: 0.4;
  }
</style>
