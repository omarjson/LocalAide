<script lang="ts">
  import Icon from "./Icon.svelte";
  import { dur } from "../lib/motion";
  import { fade, scale } from "svelte/transition";
  import type { Citation } from "../lib/api/bindings";

  interface Props {
    role: string;
    content: string;
    ts?: number;
    streaming?: boolean;
    citations?: Citation[];
    onRetry?: () => void;
    onEdit?: (newContent: string) => void;
    images?: string[];
  }

  let { role, content, ts, streaming = false, citations = [], onRetry, onEdit, images = [] }: Props = $props();

  let copied = $state(false);
  let copiedBlock = $state<number | null>(null);
  let isEditing = $state(false);
  let editDraft = $state("");
  let editEl = $state<HTMLTextAreaElement>();
  let activeCitation = $state<Citation | null>(null);
  let lightbox = $state<string | null>(null);

  $effect(() => {
    if (isEditing) {
      editEl?.focus();
    }
  });

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

  // Inline markdown: `code`, **bold**, *italic*, and bare links. Rendered as
  // tokens through {#each} — never {@html} — so nothing is ever injected.
  type InlineToken =
    | { t: "text"; v: string }
    | { t: "code"; v: string }
    | { t: "bold"; v: string }
    | { t: "italic"; v: string }
    | { t: "link"; v: string; href: string };

  const inlineRegex = /`([^`\n]+)`|\*\*([^*\n]+)\*\*|\*([^*\n]+)\*|(https?:\/\/[^\s<>()[\]]+)/g;

  function parseInline(text: string): InlineToken[] {
    const tokens: InlineToken[] = [];
    let last = 0;
    let m: RegExpExecArray | null;
    inlineRegex.lastIndex = 0;
    while ((m = inlineRegex.exec(text)) !== null) {
      if (m.index > last) {
        tokens.push({ t: "text", v: text.slice(last, m.index) });
      }
      if (m[1] !== undefined) {
        tokens.push({ t: "code", v: m[1] });
      } else if (m[2] !== undefined) {
        tokens.push({ t: "bold", v: m[2] });
      } else if (m[3] !== undefined) {
        tokens.push({ t: "italic", v: m[3] });
      } else if (m[4] !== undefined) {
        tokens.push({ t: "link", v: m[4], href: m[4] });
      }
      last = m.index + m[0].length;
    }
    if (last < text.length) {
      tokens.push({ t: "text", v: text.slice(last) });
    }
    return tokens.length > 0 ? tokens : [{ t: "text", v: text }];
  }
</script>

<div class={role === "user" ? "flex justify-end group mb-4" : "flex justify-start group mb-4"}>
  <div class="max-w-[85%] flex flex-col {role === 'user' ? 'items-end' : 'items-start'}">
    <div class="flex items-center gap-2 mb-1 px-1">
      <span class="text-xs font-mono uppercase tracking-wider text-ink-400 font-medium">
        {role === "user" ? "You" : "LocalAide"}
      </span>
      {#if ts}
        <span class="text-xs text-ink-400 font-mono">
          {new Date(ts).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
        </span>
      {/if}
    </div>

    {#if isEditing}
      <div class="w-full bg-ink-900 border border-ink-700 rounded-2xl p-3 space-y-2">
        <textarea
          bind:this={editEl}
          bind:value={editDraft}
          rows={3}
          class="w-full bg-ink-950 border border-ink-800 rounded-lg p-2 text-xs text-ink-200 font-sans focus:outline-none focus:border-ink-600"
        ></textarea>
        <div class="flex justify-end gap-2">
          <button onclick={() => isEditing = false} class="px-2.5 py-1 text-xs text-ink-400 hover:text-ink-200">
            Cancel
          </button>
          <button onclick={submitEdit} class="px-3 py-1 bg-ember-600 hover:bg-ember-500 text-ink-950 rounded-lg text-xs font-medium">
            Save & Submit
          </button>
        </div>
      </div>
    {:else}
      <div
        class={`px-4 py-3 rounded-2xl text-sm leading-relaxed relative ${
          role === "user"
            ? "bg-ember-600 text-ink-950 shadow-md shadow-ember-500/10"
            : "bg-ink-900 border border-ink-800 text-ink-100"
        }`}
      >
        {#if images.length > 0}
          <div class="flex flex-wrap gap-2 mb-2">
            {#each images as src, i (i)}
              <button
                type="button"
                onclick={() => (lightbox = src)}
                aria-label="View image {i + 1} fullscreen"
                class="block cursor-zoom-in"
              >
                <img src={src} alt="Attached image {i + 1}" class="max-h-48 rounded-lg border border-white/20" />
              </button>
            {/each}
          </div>
        {/if}
        <div class="space-y-3">
          {#if streaming && role === "assistant" && !content.trim()}
            <span class="text-ink-400 text-xs">Thinking…</span>
          {/if}
          {#each segments as seg, idx}
            {#if seg.type === "text"}
              <div class="whitespace-pre-wrap">
                {#each parseInline(seg.text) as tok, ti (ti)}
                  {#if tok.t === "code"}<code class="px-1 py-0.5 rounded bg-ink-950 border border-ink-700/60 text-[0.85em] font-mono text-ember-200">{tok.v}</code>{:else if tok.t === "bold"}<strong class="font-semibold text-ink-50">{tok.v}</strong>{:else if tok.t === "italic"}<em class="italic">{tok.v}</em>{:else if tok.t === "link"}<a href={tok.href} target="_blank" rel="noopener noreferrer" class="underline decoration-ember-500/50 hover:decoration-ember-400 text-ember-300 break-all">{tok.v}</a>{:else}{tok.v}{/if}
                {/each}
              </div>
            {:else}
              <div class="rounded-xl overflow-hidden border border-ink-700/60 bg-ink-950 my-2">
                <div class="flex items-center justify-between px-3 py-1.5 bg-ink-900/90 border-b border-ink-800 text-[11px] font-mono text-ink-400">
                  <span>{seg.lang}</span>
                  <button
                    type="button"
                    onclick={() => copyCode(seg.code, idx)}
                    class="hover:text-ink-200 transition-colors px-1.5 py-0.5 rounded bg-ink-800/60"
                  >
                    {copiedBlock === idx ? "✓ Copied!" : "Copy code"}
                  </button>
                </div>
                <pre class="p-3 text-xs font-mono text-ink-200 overflow-x-auto leading-relaxed"><code>{seg.code}</code></pre>
              </div>
            {/if}
          {/each}
          {#if streaming && role === "assistant"}
            <span class="inline-block w-2 h-4 bg-ember-400 animate-pulse ml-0.5 align-middle"></span>
          {/if}
        </div>

        {#if citations && citations.length > 0}
          <div class="mt-3 pt-2.5 border-t border-ink-800/80 flex flex-wrap items-center gap-1.5">
            <span class="text-xs font-mono uppercase tracking-wider text-ink-400">Sources:</span>
            {#each citations as cit}
              <button
                type="button"
                onclick={() => activeCitation = cit}
                class="inline-flex items-center gap-1 text-[11px] font-mono px-2 py-0.5 rounded-md bg-ink-800/80 hover:bg-ink-700/80 text-ember-300 border border-ink-700/60 transition-colors"
              >
                <span class="text-ember-400 font-semibold">[{cit.index}]</span>
                <span class="max-w-[120px] truncate">{cit.title}</span>
              </button>
            {/each}
          </div>
        {/if}
      </div>

      <!-- Action buttons -->
      <div class="opacity-0 group-hover:opacity-100 focus-within:opacity-100 transition-opacity flex items-center gap-1.5 mt-1 text-[11px] text-ink-400">
        <button
          type="button"
          onclick={copyMessage}
          class="hover:text-ink-300 transition-colors px-1 py-0.5"
          title="Copy message to clipboard"
        >
          {copied ? "✓ Copied" : "Copy"}
        </button>

        {#if role === "user" && onEdit}
          <span>•</span>
          <button
            type="button"
            onclick={startEdit}
            class="hover:text-ink-300 transition-colors px-1 py-0.5"
          >
            Edit
          </button>
        {/if}

        {#if role === "assistant" && onRetry}
          <span>•</span>
          <button
            type="button"
            onclick={onRetry}
            class="hover:text-ink-300 transition-colors px-1 py-0.5"
          >
            Retry
          </button>
        {/if}
      </div>
    {/if}
  </div>
</div>

{#if lightbox}
  <div
    class="fixed inset-0 z-[60] bg-black/90 flex items-center justify-center p-6"
    role="presentation"
    in:fade={{ duration: dur(100) }}
    out:fade={{ duration: dur(80) }}
    onclick={() => (lightbox = null)}
    onkeydown={(e) => { if (e.key === 'Escape') lightbox = null; }}
  >
    <img
      src={lightbox}
      alt="Attached image fullscreen"
      class="max-h-[85vh] max-w-[92vw] rounded-xl border border-ink-700"
      onclick={(e) => e.stopPropagation()}
    />
    <button
      type="button"
      onclick={() => (lightbox = null)}
      aria-label="Close fullscreen image"
      class="absolute top-5 right-5 p-2 rounded-xl bg-ink-900 border border-ink-700 text-ink-300 hover:text-ink-100 transition-colors"
    >
      <Icon name="close" class="w-4 h-4" />
    </button>
  </div>
{/if}

{#if activeCitation}
  <div
    class="fixed inset-0 bg-black/60 backdrop-blur-sm z-50 flex items-center justify-center p-4"
    role="presentation"
    in:fade={{ duration: dur(100) }}
    out:fade={{ duration: dur(80) }}
    onclick={() => activeCitation = null}
    onkeydown={(e) => { if (e.key === 'Escape') activeCitation = null; }}
  >
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Citation source"
      tabindex="-1"
      in:scale={{ start: 0.97, duration: dur(130) }}
      out:scale={{ start: 0.97, duration: dur(80) }}
      class="bg-ink-900 border border-ink-800 rounded-2xl max-w-lg w-full p-5 space-y-3 shadow-2xl"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <div class="flex items-center justify-between pb-2 border-b border-ink-800">
        <div class="flex items-center gap-2">
          <span class="px-2 py-0.5 rounded bg-ember-950 border border-ember-800/50 text-ember-300 font-mono text-xs">
            Source [{activeCitation.index}]
          </span>
          <span class="font-medium text-sm text-ink-200">{activeCitation.title}</span>
        </div>
        <button onclick={() => activeCitation = null} class="text-ink-400 hover:text-ink-300 text-xs px-2 py-1">
          <Icon name="close" class="w-3.5 h-3.5" />
        </button>
      </div>
      <div class="text-[11px] font-mono text-ink-400 break-all">
        {activeCitation.source}
      </div>
      <div class="bg-ink-950 border border-ink-800/80 rounded-xl p-3 max-h-60 overflow-y-auto">
        <pre class="text-xs font-mono text-ink-300 whitespace-pre-wrap leading-relaxed">{activeCitation.snippet}</pre>
      </div>
      <div class="flex justify-between items-center text-xs font-mono text-ink-400 pt-1">
        <span>Score: {activeCitation.score.toFixed(3)}</span>
        <button onclick={() => activeCitation = null} class="px-3 py-1 bg-ink-800 hover:bg-ink-700 text-ink-200 rounded-lg text-xs">
          Close
        </button>
      </div>
    </div>
  </div>
{/if}
