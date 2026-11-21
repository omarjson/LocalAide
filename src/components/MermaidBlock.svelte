<script lang="ts">
  // A ```mermaid diagram. Mermaid loads only when a chat has one. The
  // result is shown as an image, so nothing in the diagram can run.
  import { currentTheme } from "../lib/theme";

  let { source }: { source: string } = $props();

  let src = $state<string | null>(null);
  let failed = $state<string | null>(null);
  let counter = 0;

  $effect(() => {
    const code = source;
    const theme = currentTheme();
    let stale = false;
    failed = null;
    void (async () => {
      try {
        const { default: mermaid } = await import("mermaid");
        mermaid.initialize({
          startOnLoad: false,
          securityLevel: "strict",
          theme: theme === "light" ? "default" : "dark",
          htmlLabels: false,
          flowchart: { htmlLabels: false },
          fontFamily: "system-ui, -apple-system, 'Segoe UI', sans-serif",
        });
        counter += 1;
        const { svg } = await mermaid.render(`la-mermaid-${Date.now()}-${counter}`, code);
        if (stale) return;
        src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
      } catch (e: unknown) {
        if (!stale) failed = e instanceof Error ? e.message.split("\n")[0] : "This diagram could not be drawn.";
      }
    })();
    return () => {
      stale = true;
    };
  });
</script>

{#if src && !failed}
  <figure class="diagram"><img {src} alt="Diagram" class="mx-auto max-h-[32rem] max-w-full" /></figure>
{:else if failed}
  <div class="rounded-xl border border-ink-700 bg-ink-950">
    <p class="border-b border-ink-800 px-3 py-1.5 text-xs text-ink-500">The diagram could not be drawn: {failed}</p>
    <pre class="overflow-x-auto p-3 font-mono text-[13px] leading-6 text-ink-300">{source}</pre>
  </div>
{:else}
  <div class="diagram skeleton h-40" aria-label="Drawing the diagram"></div>
{/if}

<style>
  .diagram {
    border-radius: 0.75rem;
    border: 1px solid var(--color-ink-700);
    background: var(--color-ink-950);
    padding: 1rem;
  }
</style>
