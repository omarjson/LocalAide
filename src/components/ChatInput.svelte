<script lang="ts">
  interface Props {
    input: string;
    streaming?: boolean;
    toolPath: string;
    showTools: boolean;
    onSend: () => void;
    onReadFile: () => void;
    onCancel?: () => void;
  }

  let {
    input = $bindable(""),
    streaming = false,
    toolPath = $bindable(""),
    showTools = $bindable(false),
    onSend,
    onReadFile,
    onCancel,
  }: Props = $props();
</script>

<div class="border-t border-zinc-800 p-4">
  <div class="flex items-center justify-between mb-2">
    <button
      type="button"
      onclick={() => showTools = !showTools}
      class="text-[11px] px-2 py-1 rounded-md bg-white/5 border border-zinc-800 text-zinc-400 hover:text-zinc-200"
    >
      {showTools ? 'Hide tool panel' : 'File tool'}
    </button>
    <div class="text-xs text-zinc-400">Enter to send • Shift+Enter for newline • Esc to cancel</div>
  </div>

  {#if showTools}
    <div class="flex gap-2 mb-3">
      <input
        aria-label="File path to read"
        bind:value={toolPath}
        placeholder="Workspace file path to inspect…"
        class="flex-1 bg-zinc-950 border border-zinc-800 rounded-xl px-3 py-2 text-xs text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-fuchsia-500"
      />
      <button
        type="button"
        onclick={onReadFile}
        class="px-3 rounded-xl bg-white/5 border border-zinc-800 hover:bg-white/10 text-xs text-zinc-300"
      >
        Read
      </button>
    </div>
  {/if}

  <form onsubmit={(e) => { e.preventDefault(); if (streaming) onCancel?.(); else onSend(); }} class="flex gap-2">
    <textarea
      aria-label="Chat message"
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
      class="flex-1 bg-zinc-900 border border-zinc-800 rounded-xl px-4 py-2.5 text-sm text-zinc-100 placeholder-zinc-500 focus:outline-none focus:border-fuchsia-500 resize-none"
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
        disabled={!input.trim()}
        class="px-5 rounded-xl bg-fuchsia-600 text-white text-sm font-medium disabled:opacity-40 transition-opacity"
      >
        Send
      </button>
    {/if}
  </form>
</div>
