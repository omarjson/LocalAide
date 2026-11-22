<script lang="ts">
  // One chrome row for the whole window: identity, chat tabs and window
  // controls. Native decorations are off on Windows and Linux, so the custom
  // buttons stay there; macOS keeps its own traffic lights and hides them.
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Icon from "./Icon.svelte";
  import LogoMark from "./LogoMark.svelte";
  import { platform } from "../lib/platform";
  import type { ChatTab } from "../lib/chats.svelte";

  interface Props {
    /** Opens the navigation on narrow windows, where the sidebar is hidden. */
    onMenu?: () => void;
    /** Chat tabs are rendered in this same row when the chat screen is showing. */
    tabs?: ChatTab[];
    activeKey?: string | null;
    visible?: boolean;
    onFocus?: (key: string) => void;
    onClose?: (key: string) => void;
    onNew?: () => void;
  }
  let {
    onMenu,
    tabs = [],
    activeKey = null,
    visible = false,
    onFocus,
    onClose,
    onNew,
  }: Props = $props();

  const isMac = platform() === "mac";
  // Resolved in onMount: the shim installs before the app boots, and a
  // module-level call here would run before that in the browser preview.
  let win: ReturnType<typeof getCurrentWindow> | null = null;
  let maximized = $state(false);

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
  class={`chrome flex h-10 shrink-0 select-none items-center gap-2 border-b border-edge bg-ink-950 ${isMac ? "pl-[80px] pr-1.5" : "pl-3"}`}
  aria-label="Window and open chats"
>
  {#if onMenu}
    <button type="button" class="menu-btn lg:hidden" aria-label="Open navigation" title="Menu" onclick={onMenu}>
      <svg viewBox="0 0 24 24" class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M4 7h16M4 12h16M4 17h16" /></svg>
    </button>
  {/if}
  <span class="flex shrink-0 items-center gap-1.5" data-tauri-drag-region>
    <LogoMark class="h-4 w-4" />
    <span class="hidden text-xs font-semibold text-ink-200 sm:inline" data-tauri-drag-region>LocalAide</span>
  </span>

  {#if visible && tabs.length > 0}
    <div
      class="flex min-w-0 flex-1 items-end gap-1 self-stretch overflow-x-auto pt-1.5"
      role="tablist"
      aria-label="Open chats"
    >
      {#each tabs as tab (tab.key)}
        <div
          role="tab"
          tabindex="0"
          aria-selected={tab.key === activeKey}
          aria-label={`Chat tab: ${tab.title}${tab.busy ? " (answering)" : ""}`}
          class={`tab group/tab flex h-8 max-w-52 min-w-0 shrink-0 cursor-pointer items-center gap-1.5 rounded-t-lg border-x border-t px-2.5 text-sm transition-colors ${
            tab.key === activeKey
              ? "border-ink-700 bg-ink-900 font-medium text-ink-50"
              : "border-transparent text-ink-400 hover:bg-veil/5 hover:text-ink-100"
          }`}
          onclick={() => onFocus?.(tab.key)}
          onkeydown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              onFocus?.(tab.key);
            }
          }}
        >
          {#if tab.busy}
            <span class="busy-dot" aria-hidden="true"></span>
          {:else}
            <LogoMark class="h-3.5 w-3.5 shrink-0 opacity-70" />
          {/if}
          <span class="min-w-0 flex-1 truncate">{tab.title}</span>
          <button
            type="button"
            class="grid h-5 w-5 shrink-0 place-items-center rounded-md text-ink-500 opacity-0 transition-[opacity,color] group-hover/tab:opacity-100 hover:bg-veil/10 hover:text-ink-50 focus-visible:opacity-100"
            aria-label={`Close the tab ${tab.title}`}
            onclick={(e) => {
              e.stopPropagation();
              onClose?.(tab.key);
            }}
          >
            <Icon name="close" class="h-3 w-3" />
          </button>
        </div>
      {/each}
      <button
        type="button"
        class="mb-1 ml-0.5 flex h-7 shrink-0 items-center gap-1 self-center rounded-md px-2 text-sm text-ink-400 transition-colors hover:bg-veil/5 hover:text-ink-100"
        title="New chat tab (Ctrl+N)"
        aria-label="Open a new chat tab"
        onclick={() => onNew?.()}
      >
        <Icon name="plus" class="h-3.5 w-3.5" />
      </button>
    </div>
    <span class="h-full w-4 shrink-0" data-tauri-drag-region aria-hidden="true"></span>
  {:else}
    <span class="min-w-0 flex-1" data-tauri-drag-region aria-hidden="true"></span>
  {/if}

  {#if !isMac}
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
  {/if}
</header>

<style>
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
  .busy-dot {
    height: 0.55rem;
    width: 0.55rem;
    flex-shrink: 0;
    border-radius: 9999px;
    background: var(--color-ember-400);
    animation: tab-pulse 1.4s ease-in-out infinite;
  }
  @keyframes tab-pulse {
    0%,
    100% {
      opacity: 1;
      transform: scale(1);
    }
    50% {
      opacity: 0.45;
      transform: scale(0.8);
    }
  }
</style>
