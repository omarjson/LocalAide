<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";

  interface Props {
    input: string;
    streaming?: boolean;
    toolPath: string;
    showTools: boolean;
    onSend: () => void;
    onReadFile: () => void;
    onCancel?: () => void;
    pendingImages?: string[];
    onPickImages?: (files: FileList | null) => void;
    onRemoveImage?: (index: number) => void;
  }

  let {
    input = $bindable(""),
    streaming = false,
    toolPath = $bindable(""),
    showTools = $bindable(false),
    onSend,
    onReadFile,
    onCancel,
    pendingImages = [],
    onPickImages,
    onRemoveImage,
  }: Props = $props();

  let textareaEl = $state<HTMLTextAreaElement>();
  onMount(() => textareaEl?.focus());
</script>

<div class="border-t border-ink-800 p-4">
  <div class="flex items-center justify-between mb-2">
    <button
      type="button"
      onclick={() => showTools = !showTools}
      class="text-[11px] px-2 py-1 rounded-md bg-white/5 border border-ink-800 text-ink-400 hover:text-ink-200"
    >
      {showTools ? 'Hide tool panel' : 'File tool'}
    </button>
    <div class="text-xs text-ink-400">Enter to send • Shift+Enter for newline • Esc to cancel</div>
  </div>

  {#if showTools}
    <div class="flex gap-2 mb-3">
      <input
        aria-label="File path to read"
        bind:value={toolPath}
        placeholder="Workspace file path to inspect…"
        class="flex-1 bg-ink-950 border border-ink-800 rounded-xl px-3 py-2 text-xs text-ink-200 placeholder-ink-600 focus:outline-none focus:border-ember-500"
      />
      <button
        type="button"
        onclick={onReadFile}
        class="px-3 rounded-xl bg-white/5 border border-ink-800 hover:bg-white/10 text-xs text-ink-300"
      >
        Read
      </button>
    </div>
  {/if}

  {#if pendingImages.length > 0}
    <div class="flex flex-wrap gap-2 mb-3">
      {#each pendingImages as src, i (i)}
        <div class="relative">
          <img src={src} alt="Attached image {i + 1}" class="h-16 rounded-lg border border-ink-700" />
          <button
            type="button"
            onclick={() => onRemoveImage?.(i)}
            aria-label="Remove image {i + 1}"
            class="absolute -top-2 -right-2 w-5 h-5 rounded-full bg-ink-800 text-ink-200 leading-none grid place-items-center"
          ><Icon name="close" class="w-3 h-3" /></button>
        </div>
      {/each}
    </div>
  {/if}

  <form onsubmit={(e) => { e.preventDefault(); if (streaming) onCancel?.(); else onSend(); }} class="flex gap-2">
    <label
      class="px-3 rounded-xl bg-white/5 border border-ink-800 hover:bg-white/10 text-xs text-ink-300 flex items-center cursor-pointer {streaming ? 'opacity-40 pointer-events-none' : ''}"
      title="Attach an image. Needs a vision model."
    >
      Image
      <input
        type="file"
        accept="image/png,image/jpeg,image/webp,image/gif"
        multiple
        class="sr-only"
        disabled={streaming}
        onchange={(e) => onPickImages?.((e.currentTarget as HTMLInputElement).files)}
      />
    </label>
    <textarea
      aria-label="Chat message"
      bind:this={textareaEl}
      bind:value={input}
      placeholder={streaming ? "Generating response…" : "Type a message…"}
      disabled={streaming}
      rows={1}
      onkeydown={(e) => {
        if (e.key === 'Enter' && !e.shiftKey && !streaming) {
          e.preventDefault();
          onSend();
        } else if (e.key === 'Escape' && streaming) {
          e.preventDefault();
          onCancel?.();
        }
      }}
      class="flex-1 bg-ink-900 border border-ink-800 rounded-xl px-4 py-2.5 text-sm text-ink-100 placeholder-ink-500 focus:outline-none focus:border-ember-500 resize-none"
    ></textarea>

    {#if streaming}
      <button
        type="button"
        onclick={onCancel}
        class="px-5 rounded-xl bg-rose-600/90 hover:bg-rose-500 text-white text-sm font-medium transition-colors flex items-center gap-1.5"
      >
        <span class="w-2 h-2 rounded-sm bg-white"></span>
        Stop
      </button>
    {:else}
      <button
        type="submit"
        disabled={!input.trim() && pendingImages.length === 0}
        class="px-5 rounded-xl bg-ember-600 text-ink-950 text-sm font-medium disabled:opacity-40 transition-opacity"
      >
        Send
      </button>
    {/if}
  </form>
</div>
