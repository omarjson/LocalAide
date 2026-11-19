<script lang="ts">
  import FocusTrap from "./FocusTrap.svelte";
  import Icon from "./Icon.svelte";
  import LogoMark from "./LogoMark.svelte";
  import MarkdownDoc from "./MarkdownDoc.svelte";
  import { openInBrowser } from "../lib/links";
  import { dur, motion } from "../lib/motion";
  import { fade, fly, scale } from "svelte/transition";
  import type { Citation } from "../lib/api/bindings";
  import type { AnswerStats } from "../lib/chatModes";

  interface Props {
    role: string;
    content: string;
    ts?: number;
    streaming?: boolean;
    citations?: Citation[];
    onRetry?: () => void;
    onEdit?: (newContent: string) => void;
    images?: string[];
    /** What LocalAide did before answering, newest last. */
    steps?: string[];
    stats?: AnswerStats | null;
    /** Developer flag: allow the plain-text view. */
    allowRaw?: boolean;
    onSendToOpenCode?: () => void;
    /** The newest message keeps its actions visible. */
    isLast?: boolean;
  }

  let {
    role,
    content,
    ts,
    streaming = false,
    citations = [],
    onRetry,
    onEdit,
    images = [],
    steps = [],
    stats = null,
    allowRaw = false,
    onSendToOpenCode,
    isLast = false,
  }: Props = $props();

  let copied = $state(false);
  let copiedBlock = $state<number | null>(null);
  let copyError = $state<string | null>(null);
  let isEditing = $state(false);
  let editDraft = $state("");
  let editEl = $state<HTMLTextAreaElement>();
  let activeCitation = $state<Citation | null>(null);
  let lightbox = $state<string | null>(null);
  let raw = $state(false);

  $effect(() => {
    if (isEditing) editEl?.focus();
  });

  const copyMessage = async () => {
    try {
      await navigator.clipboard.writeText(content);
      copied = true;
      copyError = null;
      setTimeout(() => { copied = false; }, 1600);
    } catch {
      copyError = "Copy failed. Try again.";
    }
  };

  const copyCode = async (code: string, idx: number) => {
    try {
      await navigator.clipboard.writeText(code);
      copiedBlock = idx;
      copyError = null;
      setTimeout(() => { copiedBlock = null; }, 1600);
    } catch {
      copyError = "Copy failed. Try again.";
    }
  };

  const submitEdit = () => {
    if (editDraft.trim() && onEdit) onEdit(editDraft);
    isEditing = false;
  };

  type Segment = { type: "text"; text: string } | { type: "code"; lang: string; code: string };

  // Code blocks are split out first; a block that is still streaming (no
  // closing fence yet) shows as code too, so it does not flicker.
  const segments = $derived.by<Segment[]>(() => {
    if (!content) return [];
    const parts: Segment[] = [];
    const fence = /```([\w+-]*)\n([\s\S]*?)(```|$)/g;
    let last = 0;
    let m: RegExpExecArray | null;
    while ((m = fence.exec(content)) !== null) {
      if (m.index > last) parts.push({ type: "text", text: content.slice(last, m.index) });
      parts.push({ type: "code", lang: m[1] || "code", code: m[2].trimEnd() });
      last = m.index + m[0].length;
      if (m[0].length === 0) break;
    }
    if (last < content.length) parts.push({ type: "text", text: content.slice(last) });
    return parts;
  });

  const host = (source: string): string => {
    try {
      return new URL(source).hostname.replace(/^www\./, "");
    } catch {
      return source.split(/[\\/]/).pop() ?? source;
    }
  };
  const isWeb = (source: string) => /^https?:\/\//.test(source);

  const openCitation = (event: MouseEvent, cit: Citation) => {
    if (isWeb(cit.source)) openInBrowser(event, cit.source);
    else activeCitation = cit;
  };

  const speed = $derived(stats && stats.ms > 0 ? (stats.tokens / (stats.ms / 1000)).toFixed(1) : null);
  const timeLabel = $derived(ts ? new Date(ts).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) : "");
</script>

{#if role === "user"}
  <div class="group flex justify-end" in:fly={{ y: 6, duration: dur(motion.base), easing: motion.enter }}>
    <div class="flex max-w-[82%] flex-col items-end">
      {#if isEditing}
        <div class="w-[min(36rem,80vw)] rounded-2xl border border-edge bg-ink-950 p-3">
          <textarea
            bind:this={editEl}
            bind:value={editDraft}
            rows={3}
            aria-label="Edit your message"
            onkeydown={(e) => { if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) submitEdit(); if (e.key === "Escape") isEditing = false; }}
            class="field-bare w-full resize-y bg-transparent text-[15px] leading-6 text-ink-100"
          ></textarea>
          <div class="mt-2 flex justify-end gap-2">
            <button type="button" onclick={() => isEditing = false} class="rounded-lg px-3 py-1.5 text-sm text-ink-300 hover:bg-veil/5">Cancel</button>
            <button type="button" onclick={submitEdit} class="rounded-lg bg-ember-500 px-3 py-1.5 text-sm font-semibold text-on-ember hover:bg-ember-400">Send</button>
          </div>
        </div>
      {:else}
        {#if images.length > 0}
          <div class="mb-2 flex flex-wrap justify-end gap-2">
            {#each images as src, i (i)}
              <button type="button" onclick={() => (lightbox = src)} aria-label="View image {i + 1}" class="block cursor-zoom-in overflow-hidden rounded-xl border border-ink-700">
                <img src={src} alt="Attachment {i + 1}" class="max-h-48 object-cover transition-transform duration-300 hover:scale-[1.02]" />
              </button>
            {/each}
          </div>
        {/if}
        {#if content}
          <div class="whitespace-pre-wrap break-words rounded-3xl rounded-br-lg bg-ink-800 px-4 py-2.5 text-[15px] leading-6 text-ink-50">{content}</div>
        {/if}
        <div class="mt-1 flex items-center gap-0.5 text-ink-500 opacity-0 transition-opacity duration-150 focus-within:opacity-100 group-hover:opacity-100">
          {#if timeLabel}<span class="px-1.5 text-xs">{timeLabel}</span>{/if}
          <button type="button" onclick={copyMessage} class="msg-action" aria-label="Copy message" title="Copy"><Icon name={copied ? "check" : "copy"} class="h-4 w-4" /></button>
          {#if onEdit}<button type="button" onclick={() => { editDraft = content; isEditing = true; }} class="msg-action" aria-label="Edit and resend" title="Edit"><Icon name="pencil" class="h-4 w-4" /></button>{/if}
          {#if onSendToOpenCode}<button type="button" onclick={onSendToOpenCode} class="msg-action" aria-label="Send to OpenCode" title="Send this task to OpenCode"><Icon name="code" class="h-4 w-4" /></button>{/if}
        </div>
      {/if}
    </div>
  </div>
{:else}
  <div class="group flex gap-3" in:fly={{ y: 6, duration: dur(motion.base), easing: motion.enter }}>
    <div class="mt-0.5 shrink-0"><LogoMark class="h-7 w-7" /></div>
    <div class="min-w-0 flex-1">
      <div class="mb-1 flex h-7 items-center gap-2 text-sm">
        <span class="font-semibold text-ink-100">LocalAide</span>
        {#if timeLabel}<span class="text-xs text-ink-500">{timeLabel}</span>{/if}
      </div>

      {#if steps.length > 0}
        <ol class="mb-3 space-y-1.5 border-l-2 border-ink-800 pl-3" aria-label="What LocalAide did">
          {#each steps as step, i (i)}
            {@const active = streaming && i === steps.length - 1 && !content}
            <li class="flex items-center gap-2 text-sm text-ink-400" in:fly={{ x: -4, duration: dur(motion.base) }}>
              {#if active}
                <span class="h-3.5 w-3.5 shrink-0 animate-spin rounded-full border-2 border-ink-700 border-t-ember-500" aria-hidden="true"></span>
              {:else}
                <Icon name="check" class="h-3.5 w-3.5 shrink-0 text-emerald-400" />
              {/if}
              <span class={active ? "text-ink-200" : ""}>{step}</span>
            </li>
          {/each}
        </ol>
      {/if}

      {#if streaming && !content.trim()}
        <div class="typing flex h-7 items-center gap-1.5" role="status" aria-label="LocalAide is thinking">
          <span></span><span></span><span></span>
        </div>
      {/if}

      {#if raw}
        <pre class="whitespace-pre-wrap break-words rounded-xl border border-ink-800 bg-ink-950 p-3 font-mono text-[13px] leading-6 text-ink-200">{content}</pre>
      {:else}
        <div class="answer space-y-3 text-[15px] leading-7 text-ink-100">
          {#each segments as seg, idx (idx)}
            {#if seg.type === "text"}
              <MarkdownDoc source={seg.text} class="answer-text" />
            {:else}
              <div class="overflow-hidden rounded-xl border border-ink-700 bg-ink-950">
                <div class="flex items-center justify-between border-b border-ink-800 bg-ink-900/70 px-3 py-1.5 font-mono text-xs text-ink-400">
                  <span>{seg.lang}</span>
                  <button type="button" onclick={() => copyCode(seg.code, idx)} class="flex items-center gap-1 rounded px-1.5 py-0.5 transition-colors hover:bg-veil/5 hover:text-ink-100">
                    <Icon name={copiedBlock === idx ? "check" : "copy"} class="h-3.5 w-3.5" />{copiedBlock === idx ? "Copied" : "Copy"}
                  </button>
                </div>
                <pre class="overflow-x-auto p-3 font-mono text-[13px] leading-6 text-ink-100"><code>{seg.code}</code></pre>
              </div>
            {/if}
          {/each}
          {#if streaming && content.trim()}<span class="caret" aria-hidden="true"></span>{/if}
        </div>
      {/if}

      {#if citations.length > 0}
        <div class="mt-4">
          <div class="mb-1.5 text-xs font-semibold uppercase tracking-wide text-ink-500">Sources</div>
          <div class="grid gap-2 sm:grid-cols-2">
            {#each citations as cit (cit.index)}
              <a
                href={isWeb(cit.source) ? cit.source : "#"}
                rel="noreferrer"
                onclick={(e) => { e.preventDefault(); openCitation(e, cit); }}
                class="lift flex min-w-0 items-start gap-2.5 rounded-lg border border-ink-700 border-l-2 border-l-ember-500/70 bg-ink-900 px-3 py-2 hover:border-edge"
                in:fade={{ duration: dur(motion.base) }}
              >
                <span class="mt-0.5 grid h-5 min-w-5 shrink-0 place-items-center rounded bg-ember-500/15 px-1 text-xs font-semibold text-ember-300">{cit.index}</span>
                <span class="min-w-0">
                  <span class="block truncate text-sm font-medium text-ink-100">{cit.title}</span>
                  <span class="flex items-center gap-1 truncate text-xs text-ink-500"><Icon name={isWeb(cit.source) ? "globe" : "file"} class="h-3 w-3 shrink-0" />{host(cit.source)}</span>
                </span>
              </a>
            {/each}
          </div>
        </div>
      {/if}

      {#if !streaming && content}
        <div class={`mt-2 flex flex-wrap items-center gap-0.5 text-ink-500 transition-opacity duration-150 focus-within:opacity-100 group-hover:opacity-100 ${isLast ? "opacity-100" : "opacity-0"}`}>
          <button type="button" onclick={copyMessage} class="msg-action" aria-label="Copy answer" title="Copy"><Icon name={copied ? "check" : "copy"} class="h-4 w-4" /></button>
          {#if onRetry}<button type="button" onclick={onRetry} class="msg-action" aria-label="Answer again" title="Answer again"><Icon name="refresh" class="h-4 w-4" /></button>{/if}
          {#if allowRaw}<button type="button" onclick={() => raw = !raw} aria-pressed={raw} class="msg-action" aria-label="Show plain text" title="Plain text"><Icon name="eye" class="h-4 w-4" /></button>{/if}
          {#if onSendToOpenCode}<button type="button" onclick={onSendToOpenCode} class="msg-action" aria-label="Send to OpenCode" title="Continue this in OpenCode"><Icon name="code" class="h-4 w-4" /></button>{/if}
          {#if stats}
            <span class="ml-1 font-mono text-xs text-ink-500" title="Answer statistics">{stats.tokens} tokens · {(stats.ms / 1000).toFixed(1)} s{speed ? ` · ${speed} tok/s` : ""}{stats.firstTokenMs !== null ? ` · first token ${Math.round(stats.firstTokenMs)} ms` : ""}</span>
          {/if}
          {#if copyError}<span role="alert" class="ml-1 text-xs text-rose-300">{copyError}</span>{/if}
        </div>
      {/if}
    </div>
  </div>
{/if}

{#if lightbox}
  <div
    class="fixed inset-0 z-[60] flex items-center justify-center bg-black/90 p-6"
    role="presentation"
    in:fade={{ duration: dur(100) }}
    out:fade={{ duration: dur(80) }}
    onclick={() => (lightbox = null)}
    onkeydown={(e) => { if (e.key === 'Escape') lightbox = null; }}
  >
    <FocusTrap>
      <button type="button" class="max-h-[85vh] max-w-[92vw] cursor-zoom-out border-none bg-transparent p-0" title="Close" onclick={(e) => e.stopPropagation()}>
        <img src={lightbox} alt="" class="max-h-[85vh] max-w-[92vw] rounded-xl border border-edge" />
      </button>
      <button type="button" onclick={() => (lightbox = null)} aria-label="Close fullscreen image" class="absolute right-5 top-5 rounded-xl border border-edge bg-ink-900 p-2 text-ink-300 transition-colors hover:text-ink-100">
        <Icon name="close" class="h-4 w-4" />
      </button>
    </FocusTrap>
  </div>
{/if}

{#if activeCitation}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4 backdrop-blur-sm"
    role="presentation"
    in:fade={{ duration: dur(100) }}
    out:fade={{ duration: dur(80) }}
    onclick={() => activeCitation = null}
    onkeydown={(e) => { if (e.key === 'Escape') activeCitation = null; }}
  >
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Source"
      tabindex="-1"
      in:scale={{ start: 0.96, duration: dur(motion.base) }}
      out:scale={{ start: 0.98, duration: dur(80) }}
      class="w-full max-w-lg space-y-3 rounded-xl border border-edge bg-ink-900 p-5 shadow-2xl"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => { if (e.key === 'Escape') activeCitation = null; e.stopPropagation(); }}
    >
      <FocusTrap>
        <div class="flex items-center justify-between border-b border-ink-800 pb-2">
          <div class="flex min-w-0 items-center gap-2">
            <span class="rounded bg-ember-500/15 px-2 py-0.5 text-xs font-semibold text-ember-300">Source {activeCitation.index}</span>
            <span class="truncate text-sm font-medium text-ink-100">{activeCitation.title}</span>
          </div>
          <button type="button" aria-label="Close" onclick={() => activeCitation = null} class="rounded p-1 text-ink-400 hover:text-ink-100"><Icon name="close" class="h-4 w-4" /></button>
        </div>
        <div class="break-all font-mono text-xs text-ink-500">{activeCitation.source}</div>
        <div class="max-h-60 overflow-y-auto rounded-lg border border-ink-800 bg-ink-950 p-3">
          <p class="whitespace-pre-wrap text-sm leading-6 text-ink-300">{activeCitation.snippet}</p>
        </div>
        <div class="flex justify-end">
          <button type="button" onclick={() => activeCitation = null} class="rounded-lg bg-ink-800 px-3 py-1.5 text-sm text-ink-100 hover:bg-ink-700">Close</button>
        </div>
      </FocusTrap>
    </div>
  </div>
{/if}

<style>
  .msg-action {
    display: grid;
    height: 1.875rem;
    width: 1.875rem;
    place-items: center;
    border-radius: 0.5rem;
    transition: background-color 150ms ease-out, color 150ms ease-out;
  }
  .msg-action:hover,
  .msg-action[aria-pressed="true"] {
    background: color-mix(in oklab, var(--color-veil) 6%, transparent);
    color: var(--color-ink-50);
  }
  .typing span {
    height: 0.45rem;
    width: 0.45rem;
    border-radius: 9999px;
    background: var(--color-ink-400);
    animation: typing-dot 1.2s ease-in-out infinite;
  }
  .typing span:nth-child(2) {
    animation-delay: 0.15s;
  }
  .typing span:nth-child(3) {
    animation-delay: 0.3s;
  }
  @keyframes typing-dot {
    0%,
    80%,
    100% {
      opacity: 0.3;
      transform: translateY(0);
    }
    40% {
      opacity: 1;
      transform: translateY(-3px);
    }
  }
  .caret {
    display: inline-block;
    height: 1.1em;
    width: 0.5rem;
    margin-left: 0.125rem;
    vertical-align: text-bottom;
    border-radius: 2px;
    background: var(--color-ember-400);
    animation: caret-blink 1s steps(2, start) infinite;
  }
  @keyframes caret-blink {
    to {
      visibility: hidden;
    }
  }
  /* Chat answers read like prose: a little more room than settings text. */
  :global(.answer-text) {
    font-size: 15px !important;
    line-height: 1.75rem !important;
    color: var(--color-ink-100) !important;
  }
</style>
