<script lang="ts">
  let {
    input = $bindable(""),
    streaming = false,
    toolPath = $bindable(""),
    imagePath = $bindable(""),
    showTools = $bindable(true),
    onSend,
    onReadFile,
    onIdentifyImage,
    onVoice
  } = $props<{
    input: string;
    streaming?: boolean;
    toolPath: string;
    imagePath: string;
    showTools: boolean;
    onSend: () => void;
    onReadFile: () => void;
    onIdentifyImage: () => void;
    onVoice: () => void;
  }>();
</script>

<div class="border-t border-zinc-800 p-4">
  <div class="flex items-center justify-between mb-2">
    <button type="button" onclick={() => showTools = !showTools} class="text-[11px] px-2 py-1 rounded-md bg-white/5 border border-zinc-800 text-zinc-400 hover:text-zinc-200">{showTools ? 'Hide tools' : 'Show tools'}</button>
    <div class="text-[10px] text-zinc-600">Enter to send • Shift+Enter for new line</div>
  </div>
  {#if showTools}
    <div class="grid grid-cols-1 md:grid-cols-2 gap-2 mb-3">
      <div class="flex gap-2">
        <input bind:value={toolPath} placeholder="File path to read…" class="flex-1 bg-zinc-950 border border-zinc-800 rounded-xl px-3 py-2 text-xs text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-fuchsia-500"/>
        <button type="button" onclick={onReadFile} class="px-3 rounded-xl bg-white/5 border border-zinc-800 hover:bg-white/10 text-xs text-zinc-300">Read</button>
      </div>
      <div class="flex gap-2">
        <input bind:value={imagePath} placeholder="Image path to identify…" class="flex-1 bg-zinc-950 border border-zinc-800 rounded-xl px-3 py-2 text-xs text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-fuchsia-500"/>
        <button type="button" onclick={onIdentifyImage} class="px-3 rounded-xl bg-white/5 border border-zinc-800 hover:bg-white/10 text-xs text-zinc-300">Identify</button>
      </div>
    </div>
  {/if}
  <form onsubmit={(e) => { e.preventDefault(); onSend(); }} class="flex gap-2">
    <textarea bind:value={input} placeholder="Message…" disabled={streaming} rows="1" onkeydown={(e) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); onSend(); } }} class="flex-1 bg-zinc-900 border border-zinc-800 rounded-xl px-4 py-2.5 text-sm text-zinc-100 placeholder-zinc-500 focus:outline-none focus:border-fuchsia-500 resize-none"></textarea>
    <button type="button" disabled={streaming} onclick={onVoice} class="size-11 grid place-items-center rounded-xl bg-white/5 border border-zinc-800 hover:bg-white/10 text-zinc-300" title="Push-to-talk">🎙️</button>
    <button type="submit" disabled={streaming || !input.trim()} class="px-5 rounded-xl bg-gradient-to-r from-fuchsia-600 to-cyan-500 text-white text-sm font-medium disabled:opacity-40">Send</button>
  </form>
</div>
