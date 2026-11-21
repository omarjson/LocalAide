<script lang="ts">
  // The window's own title bar: drag region, identity, and the three
  // window buttons. Native decorations are off, so this is the frame —
  // and the one place Deep Ember can dress the whole window.
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import LogoMark from "./LogoMark.svelte";

  // Resolved in onMount: the shim installs before the app boots, and a
  // module-level call here would run before that in the browser preview.
  let win: ReturnType<typeof getCurrentWindow> | null = null;
  let maximized = $state(false);

  onMount(() => {
    const current = getCurrentWindow();
    win = current;
    let stop: (() => void) | undefined;
    void current.isMaximized().then((m) => (maximized = m === true)).catch(() => undefined);
    void current
      .onResized(() => {
        void current.isMaximized().then((m) => (maximized = m === true)).catch(() => undefined);
      })
      .then((unlisten) => {
        stop = unlisten;
      })
      .catch(() => undefined);
    return () => stop?.();
  });
</script>

<header
  class="titlebar flex h-9 shrink-0 select-none items-center gap-2 pl-3"
  data-tauri-drag-region
  aria-label="Window"
>
  <span class="flex shrink-0 items-center" data-tauri-drag-region><LogoMark class="h-4 w-4" /></span>
  <span class="text-xs font-semibold text-ink-200" data-tauri-drag-region>LocalAide</span>
  <span class="min-w-0 flex-1" data-tauri-drag-region></span>
  <button type="button" class="win-btn" aria-label="Minimize the window" title="Minimize" onclick={() => void win?.minimize()}>
    <svg viewBox="0 0 24 24" class="h-3.5 w-3.5" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M5 12h14" /></svg>
  </button>
  <button
    type="button"
    class="win-btn"
    aria-label={maximized ? "Restore the window size" : "Maximize the window"}
    title={maximized ? "Restore" : "Maximize"}
    onclick={() => void win?.toggleMaximize()}
  >
    {#if maximized}
      <svg viewBox="0 0 24 24" class="h-3.5 w-3.5" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
        <rect x="8" y="5" width="11" height="11" rx="1" />
        <path d="M5 9v9a1 1 0 0 0 1 1h9" />
      </svg>
    {:else}
      <svg viewBox="0 0 24 24" class="h-3.5 w-3.5" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round">
        <rect x="6" y="6" width="12" height="12" rx="1" />
      </svg>
    {/if}
  </button>
  <button type="button" class="win-btn win-close" aria-label="Close the window" title="Close" onclick={() => void win?.close()}>
    <svg viewBox="0 0 24 24" class="h-3.5 w-3.5" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M6 6l12 12M18 6L6 18" /></svg>
  </button>
</header>

<style>
  .titlebar {
    background: var(--color-ink-950);
    border-bottom: 1px solid var(--color-edge);
  }
  .win-btn {
    display: grid;
    height: 2.25rem;
    width: 2.75rem;
    place-items: center;
    color: var(--color-ink-400);
    transition: background-color 120ms ease-out, color 120ms ease-out;
  }
  .win-btn:hover {
    background: color-mix(in oklab, var(--color-veil) 10%, transparent);
    color: var(--color-ink-50);
  }
  .win-close:hover {
    background: #c42b1c;
    color: #fff;
  }
</style>
