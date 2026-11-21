<script lang="ts">
  // An interface mockup or small page from a ```html block. It runs in a
  // frame served by the app's mockup: scheme with its own strict policy
  // (no network, no app access), sandboxed without same-origin. A button
  // on it can call localaide.choose("…") to send the user's pick.
  import { convertFileSrc } from "@tauri-apps/api/core";
  import Icon from "./Icon.svelte";
  import { api, errorMessage } from "../lib/api/client";

  let { source, onChoose }: { source: string; onChoose?: (text: string) => void } = $props();

  let frame = $state<HTMLIFrameElement>();
  let url = $state<string | null>(null);
  let failed = $state<string | null>(null);
  let height = $state(320);
  let showCode = $state(false);
  let chosen = $state<string | null>(null);

  $effect(() => {
    const html = source;
    let stale = false;
    void api
      .renderFrame(html)
      .then((id) => {
        if (!stale) url = convertFileSrc(id, "mockup");
      })
      .catch((e: unknown) => {
        if (!stale) failed = errorMessage(e);
      });
    return () => {
      stale = true;
    };
  });

  $effect(() => {
    const onMessage = (event: MessageEvent) => {
      if (!frame || event.source !== frame.contentWindow) return;
      const data = event.data as { localaide?: string; height?: number; text?: string } | null;
      if (data?.localaide === "size" && typeof data.height === "number") {
        height = Math.max(120, Math.min(720, Math.ceil(data.height)));
      } else if (data?.localaide === "choose" && typeof data.text === "string") {
        // Only a pick the user made by clicking in the frame counts.
        if (chosen || !onChoose || document.activeElement !== frame) return;
        chosen = data.text.slice(0, 500);
        onChoose(chosen);
      }
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  });
</script>

<figure class="frame">
  <div class="frame-bar">
    <Icon name="window" class="h-3.5 w-3.5" />
    <span>Mockup</span>
    <span class="text-ink-600">·</span>
    <span class="text-ink-500">Sandboxed: no internet, no access to your files</span>
    <button type="button" class="ml-auto rounded px-1.5 py-0.5 hover:bg-veil/5 hover:text-ink-100" aria-pressed={showCode} onclick={() => (showCode = !showCode)}>{showCode ? "Show page" : "Show code"}</button>
  </div>
  {#if showCode}
    <pre class="max-h-96 overflow-auto p-3 font-mono text-[12.5px] leading-5 text-ink-300">{source}</pre>
  {:else if failed}
    <p class="p-3 text-sm text-rose-300">{failed}</p>
  {:else if url}
    <iframe bind:this={frame} src={url} title="Mockup from the answer" sandbox="allow-scripts" referrerpolicy="no-referrer" style={`height:${height}px`} class="block w-full bg-white"></iframe>
  {:else}
    <div class="skeleton h-40"></div>
  {/if}
  {#if chosen}
    <p class="flex items-center gap-2 border-t border-ink-800 px-3 py-1.5 text-[13px] text-ink-300"><Icon name="check" class="h-3.5 w-3.5 text-emerald-400" />You picked “{chosen}”.</p>
  {/if}
</figure>

<style>
  .frame {
    overflow: hidden;
    border-radius: 0.75rem;
    border: 1px solid var(--color-ink-700);
    background: var(--color-ink-950);
  }
  .frame-bar {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    border-bottom: 1px solid var(--color-ink-800);
    padding: 0.35rem 0.75rem;
    font-size: 0.75rem;
    color: var(--color-ink-400);
  }
</style>
