<script lang="ts">
  // Renders a document the app ships (privacy policy, terms, release notes).
  // Uses the parsed blocks from lib/markdown.ts; no {@html}.
  import { parseMarkdown, type Inline } from "../lib/markdown";
  import { openInBrowser } from "../lib/links";

  let { source, class: className = "" }: { source: string; class?: string } = $props();
  const blocks = $derived(parseMarkdown(source));
</script>

{#snippet inline(parts: Inline[])}
  {#each parts as part, i (i)}
    {#if part.kind === "bold"}<strong class="font-semibold text-ink-50">{part.text}</strong>
    {:else if part.kind === "code"}<code class="rounded bg-ink-800 px-1 py-0.5 font-mono text-[0.85em] text-ink-100">{part.text}</code>
    {:else if part.kind === "link"}<a href={part.href} rel="noreferrer" class="text-ember-300 underline-offset-2 hover:underline" onclick={(e) => openInBrowser(e, part.href)}>{part.text}</a>
    {:else}{part.text}{/if}
  {/each}
{/snippet}

<div class={`space-y-3 text-sm leading-6 text-ink-200 ${className}`}>
  {#each blocks as block, i (i)}
    {#if block.kind === "heading"}
      {#if block.level === 1}
        <h2 class="text-lg font-semibold text-ink-50">{@render inline(block.text)}</h2>
      {:else if block.level === 2}
        <h3 class="pt-3 text-[15px] font-semibold text-ink-50">{@render inline(block.text)}</h3>
      {:else}
        <h4 class="pt-2 font-semibold text-ink-100">{@render inline(block.text)}</h4>
      {/if}
    {:else if block.kind === "paragraph"}
      <p>{@render inline(block.text)}</p>
    {:else if block.kind === "list"}
      {#if block.ordered}
        <ol class="list-decimal space-y-1 pl-5">{#each block.items as item, j (j)}<li>{@render inline(item)}</li>{/each}</ol>
      {:else}
        <ul class="list-disc space-y-1 pl-5 marker:text-ember-500">{#each block.items as item, j (j)}<li>{@render inline(item)}</li>{/each}</ul>
      {/if}
    {:else if block.kind === "table"}
      <div class="overflow-x-auto rounded-md border border-ink-700">
        <table class="w-full text-left text-sm">
          <thead class="bg-ink-800/60 text-ink-50">
            <tr>{#each block.header as cell, j (j)}<th class="px-3 py-2 font-semibold">{@render inline(cell)}</th>{/each}</tr>
          </thead>
          <tbody>
            {#each block.rows as row, r (r)}
              <tr class="border-t border-ink-800 align-top">{#each row as cell, j (j)}<td class="px-3 py-2">{@render inline(cell)}</td>{/each}</tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  {/each}
</div>
