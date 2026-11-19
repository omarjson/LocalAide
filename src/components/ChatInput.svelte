<script lang="ts">
  import { onMount, tick } from "svelte";
  import { fly } from "svelte/transition";
  import Icon from "./Icon.svelte";
  import { MODE_ICONS } from "../lib/chatModes";
  import { popover } from "../lib/actions/popover";
  import { dur, motion } from "../lib/motion";
  import type { ChatModeId, ChatModeInfo, Skill } from "../lib/api/bindings";

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
    /** Saved prompt skills; typing / opens the picker. */
    skills?: Skill[];
    onPickSkill?: (body: string) => void;
    mode?: ChatModeId;
    modes?: ChatModeInfo[];
    onModeChange?: (mode: ChatModeId) => void;
    useWeb?: boolean;
    webAvailable?: boolean;
    onToggleWeb?: () => void;
    useRecall?: boolean;
    recallAvailable?: boolean;
    onToggleRecall?: () => void;
    /** Shown under the box: which engine answers. */
    engineLabel?: string;
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
    skills = [],
    onPickSkill,
    mode = "standard",
    modes = [],
    onModeChange,
    useWeb = false,
    webAvailable = false,
    onToggleWeb,
    useRecall = true,
    recallAvailable = true,
    onToggleRecall,
    engineLabel = "",
  }: Props = $props();

  let textareaEl = $state<HTMLTextAreaElement>();
  let fileInputEl = $state<HTMLInputElement>();
  let skillCursor = $state(0);
  let modeMenuOpen = $state(false);
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

  // Slash menu: typing "/word" filters the skill list; Enter or click
  // inserts the skill body as the message text (the user can edit it).
  const slashMatch = $derived(
    input.length > 1 && input.startsWith("/") ? input.slice(1).toLowerCase() : null
  );
  const skillMatches = $derived(
    slashMatch === null
      ? []
      : skills.filter(
          (s) =>
            s.name.toLowerCase().includes(slashMatch) ||
            s.description.toLowerCase().includes(slashMatch)
        )
  );

  const pickSkill = (skill: Skill) => {
    input = skill.body;
    onPickSkill?.(skill.body);
    skillCursor = 0;
    textareaEl?.focus();
  };

  const handleKeydown = (e: KeyboardEvent) => {
    if (skillMatches.length > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        skillCursor = (skillCursor + 1) % skillMatches.length;
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        skillCursor = (skillCursor - 1 + skillMatches.length) % skillMatches.length;
        return;
      }
      if (e.key === "Tab" || (e.key === "Enter" && !e.shiftKey)) {
        e.preventDefault();
        pickSkill(skillMatches[skillCursor]);
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        skillCursor = 0;
        input = "";
        return;
      }
    }
    if (e.key === "Enter" && !e.shiftKey && !e.ctrlKey && !e.metaKey && !e.altKey) {
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

  const canSend = $derived(!!input.trim() || pendingImages.length > 0);
</script>

<div class="relative">
  {#if slashMatch !== null && skillMatches.length > 0}
    <div
      class="absolute bottom-full left-0 right-0 z-30 mb-2 overflow-hidden rounded-xl border border-edge bg-ink-950 shadow-2xl"
      role="listbox"
      aria-label="Skill picker"
      in:fly={{ y: 6, duration: dur(motion.base), easing: motion.enter }}
    >
      <div class="px-3 pb-1 pt-2 text-xs font-semibold text-ink-500">Skills</div>
      {#each skillMatches.slice(0, 6) as skill, i (skill.path)}
        <button
          type="button"
          role="option"
          aria-selected={i === skillCursor}
          onclick={() => pickSkill(skill)}
          onmouseenter={() => skillCursor = i}
          class={`flex w-full items-center gap-2.5 px-3 py-2 text-left text-sm transition-colors ${i === skillCursor ? 'bg-veil/10' : ''}`}
        >
          <Icon name="skills" class="h-4 w-4 shrink-0 text-ember-400" />
          <span class="shrink-0 font-mono text-ink-100">/{skill.name}</span>
          <span class="truncate text-ink-400">{skill.description || skill.body.slice(0, 80)}</span>
        </button>
      {/each}
    </div>
  {/if}

  <div class="composer field-shell rounded-2xl border border-ink-700 bg-ink-950 shadow-[0_8px_30px_-12px_rgb(0_0_0/0.45)]">
    {#if showTools}
      <div class="space-y-2 border-b border-ink-800 p-3" in:fly={{ y: -4, duration: dur(motion.base) }}>
        <div class="flex items-center justify-between text-xs text-ink-400">
          <span class="flex items-center gap-1.5 font-semibold text-ink-300"><Icon name="folder" class="h-3.5 w-3.5" />Workspace files</span>
          <span>LocalAide asks before it writes anything.</span>
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

    {#if pendingImages.length > 0}
      <div class="flex flex-wrap gap-2 px-3 pt-3">
        {#each pendingImages as src, i (i)}
          <div class="group relative" in:fly={{ y: 4, duration: dur(motion.base) }}>
            <img src={src} alt="Attachment {i + 1}" class="h-16 w-16 rounded-lg border border-ink-700 object-cover" />
            <button
              type="button"
              onclick={() => onRemoveImage?.(i)}
              aria-label="Remove image {i + 1}"
              class="absolute -right-1.5 -top-1.5 grid h-5 w-5 place-items-center rounded-full border border-ink-700 bg-ink-800 text-ink-200 shadow transition-colors hover:bg-rose-600 hover:text-white"
            ><Icon name="close" class="h-3 w-3" /></button>
          </div>
        {/each}
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
      oninput={resize}
      class="field-bare block max-h-60 w-full resize-none bg-transparent px-4 pb-1 pt-3.5 text-[15px] leading-6 text-ink-50 placeholder:text-ink-500"
    ></textarea>

    <div class="flex items-center gap-1 px-2 pb-2 pt-1">
      <input
        bind:this={fileInputEl}
        type="file"
        accept="image/png,image/jpeg,image/webp,image/gif"
        multiple
        class="sr-only"
        tabindex="-1"
        aria-hidden="true"
        disabled={streaming}
        onchange={(e) => { onPickImages?.((e.currentTarget as HTMLInputElement).files); (e.currentTarget as HTMLInputElement).value = ""; }}
      />
      <button
        type="button"
        onclick={() => fileInputEl?.click()}
        disabled={streaming}
        aria-label="Attach an image"
        title="Attach an image (needs a vision model)"
        class="composer-tool"
      ><Icon name="image" class="h-[18px] w-[18px]" /></button>
      <button
        type="button"
        onclick={() => showTools = !showTools}
        aria-pressed={showTools}
        aria-label="Workspace files"
        title="Read or write a file in the LocalAide workspace"
        class="composer-tool"
      ><Icon name="folder" class="h-[18px] w-[18px]" /></button>

      <span class="mx-1 h-5 w-px bg-ink-800" aria-hidden="true"></span>

      {#if modes.length > 0}
        <div class="relative" use:popover={{ open: modeMenuOpen, onClose: () => modeMenuOpen = false }}>
          <button
            type="button"
            aria-haspopup="menu"
            aria-expanded={modeMenuOpen}
            onclick={() => modeMenuOpen = !modeMenuOpen}
            class={`composer-chip ${mode !== "standard" ? "composer-chip-on" : ""}`}
            title={currentMode?.description}
          >
            <Icon name={MODE_ICONS[mode]} class="h-4 w-4" />
            <span>{currentMode?.label ?? "Standard"}</span>
            <Icon name="chevron-down" class={`h-3.5 w-3.5 transition-transform duration-200 ${modeMenuOpen ? "rotate-180" : ""}`} />
          </button>
          {#if modeMenuOpen}
            <div
              role="menu"
              aria-label="Chat mode"
              class="absolute bottom-full left-0 z-30 mb-2 w-80 overflow-hidden rounded-xl border border-edge bg-ink-950 p-1.5 shadow-2xl"
              in:fly={{ y: 6, duration: dur(motion.base), easing: motion.enter }}
            >
              <div class="px-2.5 pb-1 pt-1.5 text-xs font-semibold text-ink-500">How LocalAide answers</div>
              {#each modes as m (m.id)}
                <button
                  type="button"
                  role="menuitemradio"
                  aria-checked={m.id === mode}
                  data-popover-item
                  onclick={() => { onModeChange?.(m.id); modeMenuOpen = false; }}
                  class={`flex w-full items-start gap-3 rounded-lg px-2.5 py-2 text-left transition-colors hover:bg-veil/5 focus:bg-veil/5 focus:outline-none ${m.id === mode ? "bg-ember-500/10" : ""}`}
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

      {#if webAvailable}
        <button
          type="button"
          aria-pressed={useWeb}
          onclick={onToggleWeb}
          title={useWeb ? "Web search is on for your messages" : "Search the web for your messages"}
          class={`composer-chip ${useWeb ? "composer-chip-on" : ""}`}
        ><Icon name="globe" class="h-4 w-4" /><span class="hidden sm:inline">Web</span></button>
      {/if}
      {#if recallAvailable}
        <button
          type="button"
          aria-pressed={useRecall}
          onclick={onToggleRecall}
          title={useRecall ? "Using your notes and files" : "Not using your notes and files"}
          class={`composer-chip ${useRecall ? "composer-chip-on" : ""}`}
        ><Icon name="memory" class="h-4 w-4" /><span class="hidden sm:inline">Notes</span></button>
      {/if}

      <span class="ml-auto hidden truncate pr-2 text-xs text-ink-500 md:inline">{engineLabel}</span>

      {#if streaming}
        <button
          type="button"
          onclick={onCancel}
          aria-label="Stop the answer"
          title="Stop (Esc)"
          class="grid h-9 w-9 shrink-0 place-items-center rounded-full bg-ink-100 text-ink-950 transition-transform hover:scale-105 active:scale-95"
        ><Icon name="stop" class="h-4 w-4" /></button>
      {:else}
        <button
          type="button"
          onclick={onSend}
          disabled={!canSend}
          aria-label="Send"
          title="Send (Enter)"
          class={`grid h-9 w-9 shrink-0 place-items-center rounded-full transition-[transform,background-color,opacity] duration-150 active:scale-95 ${canSend ? "bg-ember-500 text-on-ember shadow-md shadow-ember-900/40 hover:bg-ember-400 hover:scale-105" : "bg-ink-800 text-ink-500"}`}
        ><Icon name="arrow-up" class="h-[18px] w-[18px]" /></button>
      {/if}
    </div>
  </div>
</div>

<style>
  .composer-tool {
    display: grid;
    height: 2.25rem;
    width: 2.25rem;
    place-items: center;
    border-radius: 0.6rem;
    color: var(--color-ink-400);
    transition: background-color 150ms ease-out, color 150ms ease-out;
  }
  .composer-tool:hover:not(:disabled),
  .composer-tool[aria-pressed="true"] {
    background: color-mix(in oklab, var(--color-veil) 6%, transparent);
    color: var(--color-ink-50);
  }
  .composer-tool:disabled {
    opacity: 0.4;
  }
  .composer-chip {
    display: inline-flex;
    height: 2rem;
    align-items: center;
    gap: 0.375rem;
    border-radius: 9999px;
    border: 1px solid var(--color-ink-700);
    padding: 0 0.7rem;
    font-size: 0.8125rem;
    color: var(--color-ink-300);
    transition: background-color 150ms ease-out, color 150ms ease-out, border-color 150ms ease-out;
  }
  .composer-chip:hover {
    border-color: var(--color-edge);
    color: var(--color-ink-50);
  }
  .composer-chip-on {
    border-color: color-mix(in oklab, var(--color-ember-500) 45%, transparent);
    background: color-mix(in oklab, var(--color-ember-500) 12%, transparent);
    color: var(--color-ember-200);
  }
</style>
