<script lang="ts">
  import Icon from "./Icon.svelte";
  import MarkdownDoc from "./MarkdownDoc.svelte";

  interface Props {
    text: string;
    /** Show the blinking caret after the text. */
    caret?: boolean;
  }

  let { text, caret = false }: Props = $props();

  let copiedBlock = $state<number | null>(null);
  let copyError = $state(false);

  type Segment = { type: "text"; text: string } | { type: "code"; lang: string; code: string };

  // Code blocks are split out first; a block that is still streaming (no
  // closing fence yet) shows as code too, so it does not flicker.
  const segments = $derived.by<Segment[]>(() => {
    if (!text) return [];
    const parts: Segment[] = [];
    const fence = /```([\w+-]*)\n([\s\S]*?)(```|$)/g;
    let last = 0;
    let m: RegExpExecArray | null;
    while ((m = fence.exec(text)) !== null) {
      if (m.index > last) parts.push({ type: "text", text: text.slice(last, m.index) });
      parts.push({ type: "code", lang: m[1] || "code", code: m[2].trimEnd() });
      last = m.index + m[0].length;
      if (m[0].length === 0) break;
    }
    if (last < text.length) parts.push({ type: "text", text: text.slice(last) });
    return parts;
  });

  const copyCode = async (code: string, idx: number) => {
    try {
      await navigator.clipboard.writeText(code);
      copiedBlock = idx;
      copyError = false;
      setTimeout(() => {
        copiedBlock = null;
      }, 1600);
    } catch {
      copyError = true;
    }
  };
</script>

<div class="answer space-y-3 text-[15px] leading-7 text-ink-100">
  {#each segments as seg, idx (idx)}
    {#if seg.type === "text"}
      <MarkdownDoc source={seg.text} class="answer-text" />
    {:else}
      <div class="overflow-hidden rounded-xl border border-ink-700 bg-ink-950">
        <div class="flex items-center justify-between border-b border-ink-800 bg-ink-900/70 px-3 py-1.5 font-mono text-xs text-ink-400">
          <span>{seg.lang}</span>
          <button type="button" onclick={() => copyCode(seg.code, idx)} class="flex items-center gap-1 rounded px-1.5 py-0.5 transition-colors hover:bg-veil/5 hover:text-ink-100">
            <Icon name={copiedBlock === idx ? "check" : "copy"} class="h-3.5 w-3.5" />{copiedBlock === idx ? "Copied" : copyError ? "Copy failed" : "Copy"}
          </button>
        </div>
        <pre class="overflow-x-auto p-3 font-mono text-[13px] leading-6 text-ink-100"><code>{seg.code}</code></pre>
      </div>
    {/if}
  {/each}
  {#if caret}<span class="caret" aria-hidden="true"></span>{/if}
</div>

<style>
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
