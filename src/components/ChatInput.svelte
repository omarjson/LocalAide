<script lang="ts">
  import { onMount, tick, type Snippet } from "svelte";
  import { fly } from "svelte/transition";
  import Icon, { type IconName } from "./Icon.svelte";
  import { MODE_ICONS } from "../lib/chatModes";
  import { popover } from "../lib/actions/popover";
  import { dur, motion } from "../lib/motion";
  import { flag } from "../lib/dev.svelte";
  import { slashItems, type SlashItem } from "../lib/slash";
  import type { ChatAttachment, ChatModeId, ChatModeInfo, Skill } from "../lib/api/bindings";

  /** An MCP server as the tools menu shows it. */
  export interface ToolServer {
    id: string;
    name: string;
    on: boolean;
  }

  interface Props {
    input: string;
    streaming?: boolean;
    toolPath: string;
    toolContent: string;
    showTools: boolean;
    onSend: () => void;
    onReadFile: () => void;
    onWriteFile?: () => void;
    onCancel?: () => void;
    pendingImages?: string[];
    onPickImages?: (files: FileList | null) => void;
    onRemoveImage?: (index: number) => void;
    attachments?: ChatAttachment[];
    attaching?: boolean;
    onAttachFiles?: () => void;
    onRemoveAttachment?: (index: number) => void;
    /** Saved prompt skills; typing / opens the picker. */
    skills?: Skill[];
    onPickSkill?: (body: string) => void;
    onSlashCommand?: (command: string) => void;
    mode?: ChatModeId;
    modes?: ChatModeInfo[];
    onModeChange?: (mode: ChatModeId) => void;
    useWeb?: boolean;
    webAvailable?: boolean;
    onToggleWeb?: () => void;
    useRecall?: boolean;
    recallAvailable?: boolean;
    onToggleRecall?: () => void;
    useFiles?: boolean;
    onToggleFiles?: () => void;
    useSkills?: boolean;
    skillsAvailable?: boolean;
    onToggleSkills?: () => void;
    mcpServers?: ToolServer[];
    mcpAvailable?: boolean;
    onToggleMcp?: (id: string) => void;
    onManageTools?: () => void;
    /** The model and thinking pickers, at the right of the box. */
    trailing?: Snippet;
  }

  let {
    input = $bindable(""),
    streaming = false,
    toolPath = $bindable(""),
    toolContent = $bindable(""),
    showTools = $bindable(false),
    onSend,
    onReadFile,
    onWriteFile,
    onCancel,
    pendingImages = [],
    onPickImages,
    onRemoveImage,
    attachments = [],
    attaching = false,
    onAttachFiles,
    onRemoveAttachment,
    skills = [],
    onPickSkill,
    onSlashCommand,
    mode = "standard",
    modes = [],
    onModeChange,
    useWeb = false,
    webAvailable = false,
    onToggleWeb,
    useRecall = true,
    recallAvailable = true,
    onToggleRecall,
    useFiles = true,
    onToggleFiles,
    useSkills = true,
    skillsAvailable = true,
    onToggleSkills,
    mcpServers = [],
    mcpAvailable = true,
    onToggleMcp,
    onManageTools,
    trailing,
  }: Props = $props();

  let textareaEl = $state<HTMLTextAreaElement>();
  let fileInputEl = $state<HTMLInputElement>();
  let skillCursor = $state(0);
  let modeMenuOpen = $state(false);
  let attachMenuOpen = $state(false);
  let toolsMenuOpen = $state(false);
  // Experimental: Ctrl+Enter (Cmd+Enter on a Mac) sends, Enter adds a line.
  const ctrlEnterSends = $derived(flag("send_ctrl_enter"));
  onMount(() => textareaEl?.focus());

  const currentMode = $derived(modes.find((m) => m.id === mode));

  // The box stays focusable while an answer streams (read-only, not disabled),
  // so Escape still stops it. Focus comes back when the answer ends.
  let wasStreaming = false;
  $effect(() => {
    if (wasStreaming && !streaming) textareaEl?.focus();
    wasStreaming = streaming;
  });

  // Grow with the text, up to about ten lines.
  const resize = () => {
    if (!textareaEl) return;
    textareaEl.style.height = "auto";
    textareaEl.style.height = `${Math.min(textareaEl.scrollHeight, 240)}px`;
  };
  $effect(() => {
    void input;
    void tick().then(resize);
  });

  const slashList = $derived(slashItems(input, skills));

  const pickSkill = (skill: Skill) => {
    input = skill.body;
    onPickSkill?.(skill.body);
    skillCursor = 0;
    textareaEl?.focus();
  };

  // Clear the "/word" first, so a command that writes a prompt prefix (or a
  // skill's body) is what stays in the box.
  const pickSlashItem = (item: SlashItem) => {
    input = "";
    skillCursor = 0;
    if (item.kind === "command" && item.value.id === "tools") {
      toolsMenuOpen = true;
      return;
    }
    if (item.kind === "command") onSlashCommand?.(item.value.id);
    else pickSkill(item.value);
    textareaEl?.focus();
  };

  // A narrower match list must not leave the cursor past its end.
  $effect(() => {
    if (skillCursor >= slashList.length) skillCursor = 0;
  });

  const handleKeydown = (e: KeyboardEvent) => {
    if (slashList.length > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        skillCursor = (skillCursor + 1) % slashList.length;
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        skillCursor = (skillCursor - 1 + slashList.length) % slashList.length;
        return;
      }
      if (e.key === "Tab" || (e.key === "Enter" && !e.shiftKey)) {
        e.preventDefault();
        pickSlashItem(slashList[skillCursor]);
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        skillCursor = 0;
        input = "";
        return;
      }
    }
    // With "Send with Ctrl+Enter" on, plain Enter adds a new line instead.
    const sendKey = ctrlEnterSends
      ? e.key === "Enter" && (e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey
      : e.key === "Enter" && !e.shiftKey && !e.ctrlKey && !e.metaKey && !e.altKey;
    if (sendKey) {
      e.preventDefault();
      // Enter that confirms an IME composition (Chinese, Japanese, Korean)
      // must not send the message.
      if (e.isComposing || e.keyCode === 229 || e.repeat) return;
      if (!streaming) onSend();
    } else if (e.key === "Escape" && streaming) {
      e.preventDefault();
      onCancel?.();
    }
  };

  // Pasted images attach like picked ones.
  const handlePaste = (e: ClipboardEvent) => {
    const files = e.clipboardData?.files;
    if (files && files.length > 0 && Array.from(files).some((f) => f.type.startsWith("image/"))) {
      e.preventDefault();
      onPickImages?.(files);
    }
  };

  const readyAttachments = $derived(attachments.filter((a) => !a.error));
  const canSend = $derived(!!input.trim() || pendingImages.length > 0 || readyAttachments.length > 0);

  type ToolRow = { id: string; label: string; hint: string; icon: IconName; on: boolean; toggle?: () => void };
  const toolRows = $derived.by<ToolRow[]>(() => {
    const rows: ToolRow[] = [];
    if (webAvailable) rows.push({ id: "web", label: "Web search", hint: "Search and read pages when it helps", icon: "globe", on: useWeb, toggle: onToggleWeb });
    if (recallAvailable) rows.push({ id: "notes", label: "Notes and documents", hint: "Search your notes and library, save notes", icon: "memory", on: useRecall, toggle: onToggleRecall });
    rows.push({ id: "files", label: "Workspace files", hint: "Read files; writing asks you first", icon: "folder", on: useFiles, toggle: onToggleFiles });
    if (skillsAvailable) rows.push({ id: "skills", label: "Skills", hint: "Use your saved skills", icon: "skills", on: useSkills, toggle: onToggleSkills });
    return rows;
  });
  const toolsOn = $derived(toolRows.filter((r) => r.on).length + (mcpAvailable ? mcpServers.filter((s) => s.on).length : 0));

  const fileSize = (chars: number) => (chars >= 1000 ? `${Math.round(chars / 1000)}K characters` : `${chars} characters`);
</script>

<div class="relative">
  {#if slashList.length > 0}
    <div
      class="absolute bottom-full left-0 right-0 z-30 mb-2 max-h-[min(22rem,60vh)] overflow-y-auto rounded-xl border border-edge bg-ink-950 shadow-2xl"
      role="listbox"
      aria-label="Slash commands and skills"
      in:fly={{ y: 6, duration: dur(motion.base), easing: motion.enter }}
    >
      {#each slashList as item, i (`${item.kind}:${item.kind === "command" ? item.value.id : item.value.path}`)}
        {#if i === 0 && item.kind === "command"}<div class="px-3 pb-1 pt-2 text-xs font-semibold text-ink-500">Commands</div>{/if}
        {#if item.kind === "skill" && (i === 0 || slashList[i - 1].kind === "command")}<div class={`px-3 pb-1 pt-2 text-xs font-semibold text-ink-500 ${i > 0 ? "border-t border-ink-800" : ""}`}>Skills</div>{/if}
        <button
          type="button"
          role="option"
          aria-selected={i === skillCursor}
          onclick={() => pickSlashItem(item)}
          onmouseenter={() => (skillCursor = i)}
          class={`flex w-full items-center gap-2.5 px-3 py-2 text-left text-sm transition-colors ${i === skillCursor ? "bg-veil/10" : ""}`}
        >
          {#if item.kind === "command"}
            <Icon name={item.value.icon} class="h-4 w-4 shrink-0 text-ember-300" />
            <span class="shrink-0 font-mono text-ink-100">/{item.value.id}</span>
            <span class="truncate text-ink-400">{item.value.description}</span>
          {:else}
            <Icon name="skills" class="h-4 w-4 shrink-0 text-ember-400" />
            <span class="shrink-0 font-mono text-ink-100">/{item.value.name}</span>
            <span class="truncate text-ink-400">{item.value.description || item.value.body.slice(0, 80)}</span>
          {/if}
        </button>
      {/each}
    </div>
  {/if}

  <div class="composer field-shell rounded-2xl border border-ink-700 bg-ink-950 shadow-[0_8px_30px_-12px_rgb(0_0_0/0.45)]">
    {#if showTools}
      <div class="space-y-2 border-b border-ink-800 p-3" in:fly={{ y: -4, duration: dur(motion.base) }}>
        <div class="flex items-center justify-between text-xs text-ink-400">
          <span class="flex items-center gap-1.5 font-semibold text-ink-300"><Icon name="folder" class="h-3.5 w-3.5" />Workspace files</span>
          <span class="flex items-center gap-2">LocalAide asks before it writes anything.<button type="button" aria-label="Close the file panel" onclick={() => (showTools = false)} class="rounded p-0.5 hover:bg-veil/5 hover:text-ink-100"><Icon name="close" class="h-3.5 w-3.5" /></button></span>
        </div>
        <div class="flex gap-2">
          <input
            aria-label="File name in the workspace folder"
            bind:value={toolPath}
            placeholder="notes.txt"
            class="h-9 min-w-0 flex-1 rounded-lg border border-ink-700 bg-ink-900 px-3 font-mono text-sm text-ink-100 placeholder:text-ink-500"
          />
          <button type="button" onclick={onReadFile} disabled={!toolPath.trim()} class="h-9 rounded-lg border border-ink-700 px-3 text-sm text-ink-200 transition-colors hover:bg-veil/5 disabled:opacity-40">Read</button>
          <button type="button" onclick={onWriteFile} disabled={!toolPath.trim() || !toolContent.trim()} class="h-9 rounded-lg border border-ink-700 px-3 text-sm text-ink-200 transition-colors hover:bg-veil/5 disabled:opacity-40">Write</button>
        </div>
        <textarea
          aria-label="File content to write"
          bind:value={toolContent}
          rows={2}
          placeholder="Text to write into the file above"
          class="w-full resize-none rounded-lg border border-ink-700 bg-ink-900 px-3 py-2 text-sm text-ink-100 placeholder:text-ink-500"
        ></textarea>
      </div>
    {/if}

    {#if pendingImages.length > 0 || attachments.length > 0 || attaching}
      <div class="flex flex-wrap gap-2 px-3 pt-3">
        {#each pendingImages as src, i (i)}
          <div class="group relative" in:fly={{ y: 4, duration: dur(motion.base) }}>
            <img {src} alt="Attachment {i + 1}" class="h-16 w-16 rounded-lg border border-ink-700 object-cover" />
            <button
              type="button"
              onclick={() => onRemoveImage?.(i)}
              aria-label="Remove image {i + 1}"
              class="absolute -right-1.5 -top-1.5 grid h-5 w-5 place-items-center rounded-full border border-ink-700 bg-ink-800 text-ink-200 shadow transition-colors hover:bg-rose-600 hover:text-white"
            ><Icon name="close" class="h-3 w-3" /></button>
          </div>
        {/each}
        {#each attachments as file, i (file.name + i)}
          <div
            class={`flex h-16 max-w-[16rem] items-center gap-2.5 rounded-lg border px-2.5 ${file.error ? "border-rose-500/40 bg-rose-500/10" : "border-ink-700 bg-ink-900"}`}
            in:fly={{ y: 4, duration: dur(motion.base) }}
            title={file.error ?? file.name}
          >
            <span class={`grid h-9 w-9 shrink-0 place-items-center rounded-md ${file.error ? "bg-rose-500/15 text-rose-300" : "bg-ink-800 text-ink-300"}`}>
              <Icon name={file.error ? "alert" : "file"} class="h-4 w-4" />
            </span>
            <span class="min-w-0">
              <span class="block truncate text-sm font-medium text-ink-100">{file.name}</span>
              <span class={`block truncate text-xs ${file.error ? "text-rose-300" : "text-ink-500"}`}>{file.error ?? `${fileSize(file.chars)}${file.truncated ? ", shortened" : ""}`}</span>
            </span>
            <button type="button" onclick={() => onRemoveAttachment?.(i)} aria-label={`Remove ${file.name}`} class="ml-auto grid h-6 w-6 shrink-0 place-items-center rounded-md text-ink-400 transition-colors hover:bg-veil/10 hover:text-ink-100"><Icon name="close" class="h-3.5 w-3.5" /></button>
          </div>
        {/each}
        {#if attaching}
          <div class="flex h-16 items-center gap-2 rounded-lg border border-ink-700 px-3 text-sm text-ink-400" role="status">
            <span class="h-3.5 w-3.5 animate-spin rounded-full border-2 border-ink-700 border-t-ember-500" aria-hidden="true"></span>Reading files…
          </div>
        {/if}
      </div>
    {/if}

    <textarea
      aria-label="Chat message"
      bind:this={textareaEl}
      bind:value={input}
      placeholder={streaming ? "Answering… press Esc to stop" : skills.length ? "Ask anything. Type / for skills" : "Ask anything"}
      readonly={streaming}
      aria-busy={streaming}
      rows={1}
      onkeydown={handleKeydown}
      onpaste={handlePaste}
      oninput={resize}
      class="field-bare block max-h-60 w-full resize-none bg-transparent px-4 pb-1 pt-3.5 text-[15px] leading-6 text-ink-50 placeholder:text-ink-500"
    ></textarea>

    <div class="flex flex-wrap items-center gap-1 px-2 pb-2 pt-1">
      <input
        bind:this={fileInputEl}
        type="file"
        accept="image/png,image/jpeg,image/webp,image/gif"
        multiple
        class="sr-only"
        tabindex="-1"
        aria-hidden="true"
        disabled={streaming}
        onchange={(e) => {
          onPickImages?.((e.currentTarget as HTMLInputElement).files);
          (e.currentTarget as HTMLInputElement).value = "";
        }}
      />

      <div class="relative" use:popover={{ open: attachMenuOpen, onClose: () => (attachMenuOpen = false) }}>
        <button type="button" aria-haspopup="menu" aria-expanded={attachMenuOpen} onclick={() => (attachMenuOpen = !attachMenuOpen)} disabled={streaming} aria-label="Attach" title="Attach files or images" class="composer-tool"><Icon name="plus" class="h-4 w-4" /></button>
        {#if attachMenuOpen}
          <div role="menu" aria-label="Attach" class="menu bottom-full left-0 mb-2 w-72" in:fly={{ y: 6, duration: dur(motion.base), easing: motion.enter }}>
            <button type="button" role="menuitem" data-popover-item onclick={() => { attachMenuOpen = false; onAttachFiles?.(); }} class="menu-row">
              <Icon name="paperclip" class="h-4 w-4 text-ink-300" /><span><span class="block text-sm text-ink-50">Attach files</span><span class="block text-xs text-ink-400">Text, code, Markdown, CSV, JSON or PDF. You can also drop files here.</span></span>
            </button>
            <button type="button" role="menuitem" data-popover-item onclick={() => { attachMenuOpen = false; fileInputEl?.click(); }} class="menu-row">
              <Icon name="image" class="h-4 w-4 text-ink-300" /><span><span class="block text-sm text-ink-50">Add an image</span><span class="block text-xs text-ink-400">For models that read images. Pasting works too.</span></span>
            </button>
            <button type="button" role="menuitem" data-popover-item onclick={() => { attachMenuOpen = false; showTools = true; }} class="menu-row">
              <Icon name="folder" class="h-4 w-4 text-ink-300" /><span><span class="block text-sm text-ink-50">Read or write a workspace file</span><span class="block text-xs text-ink-400">Do it yourself, without the model.</span></span>
            </button>
          </div>
        {/if}
      </div>

      <div class="relative" use:popover={{ open: toolsMenuOpen, onClose: () => (toolsMenuOpen = false) }}>
        <button type="button" aria-haspopup="menu" aria-expanded={toolsMenuOpen} onclick={() => (toolsMenuOpen = !toolsMenuOpen)} class={`composer-chip ${toolsOn > 0 ? "" : "opacity-80"}`} title="Tools the model may use">
          <Icon name="wrench" class="h-3.5 w-3.5" /><span class="hidden sm:inline">Tools</span>{#if toolsOn > 0}<span class="rounded-full bg-ink-800 px-1.5 text-[11px] leading-4 tabular-nums text-ink-200">{toolsOn}</span>{/if}
        </button>
        {#if toolsMenuOpen}
          <div role="menu" aria-label="Tools the model may use" class="menu bottom-full left-0 mb-2 w-80" in:fly={{ y: 6, duration: dur(motion.base), easing: motion.enter }}>
            <div class="px-2.5 pb-1 pt-1.5 text-xs text-ink-500">The model decides when to use these.</div>
            {#each toolRows as row (row.id)}
              <button type="button" role="menuitemcheckbox" aria-checked={row.on} data-popover-item onclick={() => row.toggle?.()} class="menu-row items-center">
                <Icon name={row.icon} class="h-4 w-4 text-ink-300" />
                <span class="min-w-0 flex-1"><span class="block text-sm text-ink-50">{row.label}</span><span class="block truncate text-xs text-ink-400">{row.hint}</span></span>
                <span class={`switch ${row.on ? "switch-on" : ""}`} aria-hidden="true"><span></span></span>
              </button>
            {/each}
            {#if mcpAvailable}
              <div class="mt-1 border-t border-ink-800 px-2.5 pb-1 pt-2 text-xs font-semibold text-ink-500">MCP servers</div>
              {#each mcpServers as server (server.id)}
                <button type="button" role="menuitemcheckbox" aria-checked={server.on} data-popover-item onclick={() => onToggleMcp?.(server.id)} class="menu-row items-center">
                  <Icon name="plug" class="h-4 w-4 text-ink-300" />
                  <span class="min-w-0 flex-1 truncate text-sm text-ink-50">{server.name}</span>
                  <span class={`switch ${server.on ? "switch-on" : ""}`} aria-hidden="true"><span></span></span>
                </button>
              {:else}
                <p class="px-2.5 pb-1 text-xs leading-5 text-ink-400">None yet. MCP servers add tools such as library docs, GitHub or a browser.</p>
              {/each}
            {/if}
            {#if onManageTools}
              <button type="button" role="menuitem" data-popover-item onclick={() => { toolsMenuOpen = false; onManageTools?.(); }} class="menu-row mt-1 items-center border-t border-ink-800 text-ink-400">
                <Icon name="settings" class="h-4 w-4" /><span class="text-sm">Manage tools and MCP servers</span>
              </button>
            {/if}
          </div>
        {/if}
      </div>


      {#if modes.length > 0}
        <div class="relative" use:popover={{ open: modeMenuOpen, onClose: () => (modeMenuOpen = false) }}>
          <button
            type="button"
            aria-haspopup="menu"
            aria-expanded={modeMenuOpen}
            onclick={() => (modeMenuOpen = !modeMenuOpen)}
            class={`composer-chip ${mode !== "standard" ? "composer-chip-on" : ""}`}
            title={currentMode?.description}
          >
            <Icon name={MODE_ICONS[mode]} class="h-3.5 w-3.5" />
            <span class="hidden md:inline">{currentMode?.label ?? "Standard"}</span>
            <Icon name="chevron-down" class={`h-3 w-3 transition-transform duration-200 ${modeMenuOpen ? "rotate-180" : ""}`} />
          </button>
          {#if modeMenuOpen}
            <div role="menu" aria-label="Chat mode" class="menu bottom-full left-0 mb-2 w-80" in:fly={{ y: 6, duration: dur(motion.base), easing: motion.enter }}>
              <div class="px-2.5 pb-1 pt-1.5 text-xs font-semibold text-ink-500">How LocalAide answers</div>
              {#each modes as m (m.id)}
                <button
                  type="button"
                  role="menuitemradio"
                  aria-checked={m.id === mode}
                  data-popover-item
                  onclick={() => { onModeChange?.(m.id); modeMenuOpen = false; }}
                  class={`menu-row ${m.id === mode ? "bg-ember-500/10" : ""}`}
                >
                  <span class={`mt-0.5 grid h-7 w-7 shrink-0 place-items-center rounded-md ${m.id === mode ? "bg-ember-500/20 text-ember-300" : "bg-ink-800 text-ink-300"}`}><Icon name={MODE_ICONS[m.id]} class="h-4 w-4" /></span>
                  <span class="min-w-0 flex-1">
                    <span class="block text-sm font-semibold text-ink-50">{m.label}</span>
                    <span class="block text-xs leading-5 text-ink-400">{m.description}</span>
                  </span>
                  {#if m.id === mode}<Icon name="check" class="mt-1 h-4 w-4 shrink-0 text-ember-300" />{/if}
                </button>
              {/each}
            </div>
          {/if}
        </div>
      {/if}

      <span class="ml-auto"></span>
      {#if trailing}{@render trailing()}{/if}

      {#if streaming}
        <button
          type="button"
          onclick={onCancel}
          aria-label="Stop the answer"
          title="Stop (Esc)"
          class="ml-1 grid h-8 w-8 shrink-0 place-items-center rounded-full bg-ink-100 text-ink-950 transition-transform hover:scale-105 active:scale-95"
        ><Icon name="stop" class="h-3.5 w-3.5" /></button>
      {:else}
        <button
          type="button"
          onclick={onSend}
          disabled={!canSend}
          aria-label="Send"
          title={ctrlEnterSends ? "Send (Ctrl+Enter)" : "Send (Enter)"}
          class={`ml-1 grid h-8 w-8 shrink-0 place-items-center rounded-full transition-[transform,background-color,opacity] duration-150 active:scale-95 ${canSend ? "bg-ember-500 text-on-ember shadow-md shadow-ember-900/40 hover:scale-105 hover:bg-ember-400" : "bg-ink-800 text-ink-500"}`}
        ><Icon name="arrow-up" class="h-4 w-4" /></button>
      {/if}
    </div>
  </div>
</div>

<style>
  .composer-tool {
    display: grid;
    height: 2rem;
    width: 2rem;
    place-items: center;
    border-radius: 0.6rem;
    color: var(--color-ink-400);
    transition: background-color 150ms ease-out, color 150ms ease-out;
  }
  .composer-tool:hover:not(:disabled),
  .composer-tool[aria-expanded="true"] {
    background: color-mix(in oklab, var(--color-veil) 6%, transparent);
    color: var(--color-ink-50);
  }
  .composer-tool:disabled {
    opacity: 0.4;
  }
  .composer-chip {
    display: inline-flex;
    height: 1.75rem;
    align-items: center;
    gap: 0.3rem;
    border-radius: 9999px;
    border: 1px solid var(--color-ink-700);
    padding: 0 0.6rem;
    font-size: 0.75rem;
    color: var(--color-ink-300);
    transition: background-color 150ms ease-out, color 150ms ease-out, border-color 150ms ease-out;
  }
  .composer-chip:hover,
  .composer-chip[aria-expanded="true"] {
    border-color: var(--color-edge);
    color: var(--color-ink-50);
  }
  .composer-chip-on {
    border-color: color-mix(in oklab, var(--color-ember-500) 45%, transparent);
    background: color-mix(in oklab, var(--color-ember-500) 12%, transparent);
    color: var(--color-ember-200);
  }
  .menu {
    position: absolute;
    z-index: 40;
    overflow: hidden;
    border-radius: 0.75rem;
    border: 1px solid var(--color-edge);
    background: var(--color-ink-950);
    padding: 0.375rem;
    box-shadow: 0 18px 40px -16px rgb(0 0 0 / 0.55);
  }
  .menu-row {
    display: flex;
    width: 100%;
    align-items: flex-start;
    gap: 0.75rem;
    border-radius: 0.5rem;
    padding: 0.5rem 0.625rem;
    text-align: left;
    transition: background-color 150ms ease-out;
  }
  .menu-row:hover,
  .menu-row:focus {
    background: color-mix(in oklab, var(--color-veil) 6%, transparent);
    outline: none;
  }
  .switch {
    position: relative;
    height: 1.125rem;
    width: 2rem;
    flex-shrink: 0;
    border-radius: 9999px;
    border: 1px solid var(--color-edge);
    background: var(--color-ink-800);
    transition: background-color 150ms ease-out, border-color 150ms ease-out;
  }
  .switch span {
    position: absolute;
    left: 0.15rem;
    top: 50%;
    height: 0.7rem;
    width: 0.7rem;
    transform: translateY(-50%);
    border-radius: 9999px;
    background: var(--color-ink-300);
    transition: transform 180ms ease-out, background-color 150ms ease-out;
  }
  .switch-on {
    border-color: var(--color-ember-600);
    background: var(--color-ember-600);
  }
  .switch-on span {
    transform: translate(0.85rem, -50%);
    background: var(--color-on-ember);
  }
</style>
