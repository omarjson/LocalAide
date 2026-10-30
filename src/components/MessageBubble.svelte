<script lang="ts">
  import type { Citation } from "../lib/api/bindings";

  interface Props {
    role: string;
    content: string;
    ts?: number;
    streaming?: boolean;
    citations?: Citation[];
    onRetry?: () => void;
    onEdit?: (newContent: string) => void;
  }

  let { role, content, ts, streaming = false, citations = [], onRetry, onEdit }: Props = $props();

  let copied = $state(false);
  let copiedBlock = $state<number | null>(null);
  let isEditing = $state(false);
  let editDraft = $state("");
  let activeCitation = $state<Citation | null>(null);

  const copyMessage = async () => {
    try {
      await navigator.clipboard.writeText(content);
      copied = true;
      setTimeout(() => { copied = false; }, 2000);
    } catch {}
  };

  const copyCode = async (code: string, idx: number) => {
    try {
      await navigator.clipboard.writeText(code);
      copiedBlock = idx;
      setTimeout(() => { copiedBlock = null; }, 2000);
    } catch {}
  };

  const startEdit = () => {
    editDraft = content;
    isEditing = true;
  };

  const submitEdit = () => {
    if (editDraft.trim() && onEdit) {
      onEdit(editDraft);
    }
    isEditing = false;
  };

  // Parse markdown code blocks
  type Segment = { type: "text"; text: string } | { type: "code"; lang: string; code: string };

  const segments = $derived.by<Segment[]>(() => {
    if (!content) return [];
    const parts: Segment[] = [];
    const codeBlockRegex = /```(\w+)?\n([\s\S]*?)```/g;
    let lastIndex = 0;
    let match: RegExpExecArray | null;

    while ((match = codeBlockRegex.exec(content)) !== null) {
      if (match.index > lastIndex) {
        parts.push({
          type: "text",
          text: content.slice(lastIndex, match.index),
        });
      }
      parts.push({
        type: "code",
        lang: match[1] || "code",
        code: match[2].trimEnd(),
      });
      lastIndex = match.index + match[0].length;
    }

    if (lastIndex < content.length) {
      parts.push({
        type: "text",
        text: content.slice(lastIndex),
      });
    }

    return parts.length > 0 ? parts : [{ type: "text", text: content }];
  });
</script>

<div class={role === "user" ? "flex justify-end group mb-4" : "flex justify-start group mb-4"}>
  <div class="max-w-[85%] flex flex-col {role === 'user' ? 'items-end' : 'items-start'}">
    <div class="flex items-center gap-2 mb-1 px-1">
      <span class="text-[10px] font-mono uppercase tracking-wider text-zinc-500 font-medium">
        {role === "user" ? "You" : "LocalAide"}
      </span>
      {#if ts}
        <span class="text-[10px] text-zinc-600 font-mono">
          {new Date(ts).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
        </span>
      {/if}
    </div>

    {#if isEditing}
      <div class="w-full bg-zinc-900 border border-zinc-700 rounded-2xl p-3 space-y-2">
        <textarea
          bind:value={editDraft}
          rows={3}
          class="w-full bg-zinc-950 border border-zinc-800 rounded-lg p-2 text-xs text-zinc-200 font-sans focus:outline-none focus:border-zinc-600"
        ></textarea>
        <div class="flex justify-end gap-2">
          <button onclick={() => isEditing = false} class="px-2.5 py-1 text-xs text-zinc-400 hover:text-zinc-200">
            Cancel
          </button>
          <button onclick={submitEdit} class="px-3 py-1 bg-fuchsia-600 hover:bg-fuchsia-500 text-white rounded-lg text-xs font-medium">
            Save & Submit
          </button>
        </div>
      </div>
    {:else}
      <div
        class={`px-4 py-3 rounded-2xl text-sm leading-relaxed relative ${
          role === "user"
            ? "bg-gradient-to-br from-fuchsia-600 to-cyan-600 text-white shadow-md shadow-fuchsia-500/10"
            : "bg-zinc-900 border border-zinc-800 text-zinc-100"
        }`}
      >
        <div class="space-y-3">
          {#each segments as seg, idx}
            {#if seg.type === "text"}
              <div class="whitespace-pre-wrap">{seg.text}</div>
            {:else}
              <div class="rounded-xl overflow-hidden border border-zinc-700/60 bg-zinc-950 my-2">
                <div class="flex items-center justify-between px-3 py-1.5 bg-zinc-900/90 border-b border-zinc-800 text-[11px] font-mono text-zinc-400">
                  <span>{seg.lang}</span>
                  <button
                    type="button"
                    onclick={() => copyCode(seg.code, idx)}
                    class="hover:text-zinc-200 transition-colors px-1.5 py-0.5 rounded bg-zinc-800/60"
                  >
                    {copiedBlock === idx ? "✓ Copied!" : "Copy code"}
                  </button>
                </div>
                <pre class="p-3 text-xs font-mono text-zinc-200 overflow-x-auto leading-relaxed"><code>{seg.code}</code></pre>
              </div>
            {/if}
          {/each}
          {#if streaming && role === "assistant"}
            <span class="inline-block w-2 h-4 bg-fuchsia-400 animate-pulse ml-0.5 align-middle"></span>
          {/if}
        </div>

        {#if citations && citations.length > 0}
          <div class="mt-3 pt-2.5 border-t border-zinc-800/80 flex flex-wrap items-center gap-1.5">
            <span class="text-[10px] font-mono uppercase tracking-wider text-zinc-500">Sources:</span>
            {#each citations as cit}
              <button
                type="button"
                onclick={() => activeCitation = cit}
                class="inline-flex items-center gap-1 text-[11px] font-mono px-2 py-0.5 rounded-md bg-zinc-800/80 hover:bg-zinc-700/80 text-fuchsia-300 border border-zinc-700/60 transition-colors"
              >
                <span class="text-fuchsia-400 font-semibold">[{cit.index}]</span>
                <span class="max-w-[120px] truncate">{cit.title}</span>
              </button>
            {/each}
          </div>
        {/if}
      </div>

      <!-- Action buttons -->
      <div class="opacity-0 group-hover:opacity-100 focus-within:opacity-100 transition-opacity flex items-center gap-1.5 mt-1 text-[11px] text-zinc-500">
        <button
          type="button"
          onclick={copyMessage}
          class="hover:text-zinc-300 transition-colors px-1 py-0.5"
          title="Copy message to clipboard"
        >
          {copied ? "✓ Copied" : "Copy"}
        </button>

        {#if role === "user" && onEdit}
          <span>•</span>
          <button
            type="button"
            onclick={startEdit}
            class="hover:text-zinc-300 transition-colors px-1 py-0.5"
          >
            Edit
          </button>
        {/if}

        {#if role === "assistant" && onRetry}
          <span>•</span>
          <button
            type="button"
            onclick={onRetry}
            class="hover:text-zinc-300 transition-colors px-1 py-0.5"
          >
            Retry
          </button>
        {/if}
      </div>
    {/if}
  </div>
</div>

{#if activeCitation}
  <div class="fixed inset-0 bg-black/60 backdrop-blur-sm z-50 flex items-center justify-center p-4">
    <div class="bg-zinc-900 border border-zinc-800 rounded-2xl max-w-lg w-full p-5 space-y-3 shadow-2xl">
      <div class="flex items-center justify-between pb-2 border-b border-zinc-800">
        <div class="flex items-center gap-2">
          <span class="px-2 py-0.5 rounded bg-fuchsia-950 border border-fuchsia-800/50 text-fuchsia-300 font-mono text-xs">
            Source [{activeCitation.index}]
          </span>
          <span class="font-medium text-sm text-zinc-200">{activeCitation.title}</span>
        </div>
        <button onclick={() => activeCitation = null} class="text-zinc-500 hover:text-zinc-300 text-xs px-2 py-1">
          ✕
        </button>
      </div>
      <div class="text-[11px] font-mono text-zinc-500 break-all">
        {activeCitation.source}
      </div>
      <div class="bg-zinc-950 border border-zinc-800/80 rounded-xl p-3 max-h-60 overflow-y-auto">
        <pre class="text-xs font-mono text-zinc-300 whitespace-pre-wrap leading-relaxed">{activeCitation.snippet}</pre>
      </div>
      <div class="flex justify-between items-center text-[10px] font-mono text-zinc-500 pt-1">
        <span>Score: {activeCitation.score.toFixed(3)}</span>
        <button onclick={() => activeCitation = null} class="px-3 py-1 bg-zinc-800 hover:bg-zinc-700 text-zinc-200 rounded-lg text-xs">
          Close
        </button>
      </div>
    </div>
  </div>
{/if}
