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

  interface Props {
    /** Opens the navigation on narrow windows, where the sidebar is hidden. */
    onMenu?: () => void;
  }
  let { onMenu }: Props = $props();

  // A window call that fails is logged, never shown as an error toast.
  const act = (what: string, call: (() => Promise<void>) | undefined) => {
    if (!call) return;
    call().catch((e: unknown) => console.warn(`Window ${what} failed`, e));
  };
  // Close asks the window to close (so the app shuts down cleanly); if
  // that is refused, the window is destroyed instead of leaving the user
  // with a button that does nothing.
  const closeWindow = () => {
    const current = win;
    if (!current) return;
    current.close().catch((e: unknown) => {
      console.warn("Window close failed, destroying it instead", e);
      current.destroy().catch((err: unknown) => console.warn("Window destroy failed", err));
    });
  };

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
  {#if onMenu}
    <button type="button" class="menu-btn lg:hidden" aria-label="Open navigation" title="Menu" onclick={onMenu}>
      <svg viewBox="0 0 24 24" class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M4 7h16M4 12h16M4 17h16" /></svg>
    </button>
  {/if}
  <span class="flex shrink-0 items-center" data-tauri-drag-region><LogoMark class="h-4 w-4" /></span>
  <span class="text-xs font-semibold text-ink-200" data-tauri-drag-region>LocalAide</span>
  <span class="min-w-0 flex-1" data-tauri-drag-region></span>
  <button type="button" class="win-btn" aria-label="Minimize the window" title="Minimize" onclick={() => act("minimize", win ? () => win!.minimize() : undefined)}>
    <svg viewBox="0 0 24 24" class="h-3.5 w-3.5" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M5 12h14" /></svg>
  </button>
  <button
    type="button"
    class="win-btn"
    aria-label={maximized ? "Restore the window size" : "Maximize the window"}
    title={maximized ? "Restore" : "Maximize"}
    onclick={() => act("maximize", win ? () => win!.toggleMaximize() : undefined)}
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
  <button type="button" class="win-btn win-close" aria-label="Close the window" title="Close" onclick={closeWindow}>
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
  .menu-btn {
    display: grid;
    height: 1.75rem;
    width: 1.75rem;
    margin-left: -0.375rem;
    place-items: center;
    border-radius: 0.375rem;
    color: var(--color-ink-300);
  }
  .menu-btn:hover {
    background: color-mix(in oklab, var(--color-veil) 10%, transparent);
    color: var(--color-ink-50);
  }
  .win-close:hover {
    background: #c42b1c;
    color: #fff;
  }
</style>
