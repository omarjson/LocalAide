<script lang="ts">
  // One chrome row for the whole window: the drag area and the window
  // controls. On Windows and Linux the logo is the window menu (Minimize /
  // Restore / Close): the old title-bar menu button duplicated the sidebar
  // control and, on wide windows, did nothing visible. On macOS there is
  // no logo in the chrome at all — real Mac apps don't put one next to the
  // traffic lights, and the traffic lights already are the window controls,
  // so the menu would be redundant. The project name is gone from the bar.
  //
  // macOS keeps its own traffic lights and hides the custom buttons. Its
  // chat tabs live in a row of their own below the chrome, so they no
  // longer crowd the title bar or the traffic lights. Windows and Linux
  // keep the tabs in the chrome row, which already reads right there.
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Icon from "./Icon.svelte";
  import LogoMark from "./LogoMark.svelte";
  import { platform } from "../lib/platform";
  import type { ChatTab } from "../lib/chats.svelte";

  interface Props {
    /** Chat tabs are rendered in the chrome row, or below it on macOS. */
    tabs?: ChatTab[];
    activeKey?: string | null;
    visible?: boolean;
    onFocus?: (key: string) => void;
    onClose?: (key: string) => void;
    onNew?: () => void;
  }
  let {
    tabs = [],
    activeKey = null,
    visible = false,
    onFocus,
    onClose,
    onNew,
  }: Props = $props();

  const isMac = platform() === "mac";
  // macOS tabs get their own row below the chrome; everywhere else they
  // stay in the chrome row.
  const tabsBelow = isMac;
  // Resolved in onMount: the shim installs before the app boots, and a
  // module-level call here would run before that in the browser preview.
  let win: ReturnType<typeof getCurrentWindow> | null = null;
  let maximized = $state(false);
  let menuOpen = $state(false);

  // Roving tabindex: only one tab is in the tab order, and the arrow keys
  // move focus between tabs without tabbing through every one of them.
  const rovingKey = $derived(activeKey ?? tabs[0]?.key ?? null);

  const onTabKeydown = (e: KeyboardEvent, index: number) => {
    if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
      e.preventDefault();
      if (tabs.length === 0) return;
      const step = e.key === "ArrowRight" ? 1 : -1;
      const next = (index + step + tabs.length) % tabs.length;
      const list = (e.currentTarget as HTMLElement).closest('[role="tablist"]');
      const nodes = list?.querySelectorAll<HTMLElement>('[role="tab"]');
      nodes?.[next]?.focus();
      onFocus?.(tabs[next].key);
    } else if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onFocus?.(tabs[index].key);
    }
  };

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
  const minimizeWindow = () => act("minimize", win ? () => win!.minimize() : undefined);
  const toggleMaximizeWindow = () => act("maximize", win ? () => win!.toggleMaximize() : undefined);

  // Double-clicking empty chrome zooms the window, like a native title
  // bar. Clicks on buttons, tabs and links are left alone.
  const onChromeDblClick = (e: MouseEvent) => {
    const target = e.target as HTMLElement | null;
    if (target?.closest("button, [role='tab'], input, a")) return;
    toggleMaximizeWindow();
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

<svelte:window onkeydown={(e) => { if (e.key === "Escape") menuOpen = false; }} />

<div class="relative">
  {#if menuOpen}
    <!-- Clicking anywhere else closes the window menu. -->
    <button type="button" aria-hidden="true" tabindex="-1" class="fixed inset-0 z-40 cursor-default" onclick={() => (menuOpen = false)}></button>
  {/if}
  <!-- svelte-ignore a11y_no_static_element_interactions -- Double-clicking
       empty chrome zooms the window, like a native title bar; the header
       itself is not interactive content. -->
  <header
    class={`chrome flex h-10 shrink-0 select-none items-center gap-2 border-b border-edge bg-ink-950 ${isMac ? "pl-[80px] pr-1.5" : "pl-3"}`}
    aria-label="Window and open chats"
    ondblclick={onChromeDblClick}
  >
    {#if !isMac}
      <div class="relative shrink-0">
        <button
          type="button"
          class="logo-btn grid h-7 w-7 place-items-center rounded-md hover:bg-veil/10"
          aria-label="Window menu"
          title="Window menu"
          aria-haspopup="menu"
          aria-expanded={menuOpen}
          onclick={() => (menuOpen = !menuOpen)}
        >
          <LogoMark class="h-4 w-4" />
        </button>
        {#if menuOpen}
          <div role="menu" aria-label="Window" class="absolute left-0 top-9 z-50 w-44 overflow-hidden rounded-lg border border-edge bg-ink-900 py-1 shadow-2xl">
            <button type="button" role="menuitem" class="menu-item" onclick={() => { menuOpen = false; minimizeWindow(); }}>Minimize</button>
            <button type="button" role="menuitem" class="menu-item" onclick={() => { menuOpen = false; toggleMaximizeWindow(); }}>{maximized ? "Restore" : "Maximize"}</button>
            <button type="button" role="menuitem" class="menu-item" onclick={() => { menuOpen = false; closeWindow(); }}>Close</button>
          </div>
        {/if}
      </div>
    {/if}

    {#if !tabsBelow && visible && tabs.length > 0}
      {@render tabStrip()}
      <span class="h-full w-4 shrink-0" data-tauri-drag-region aria-hidden="true"></span>
    {:else}
      <span class="min-w-0 flex-1" data-tauri-drag-region aria-hidden="true"></span>
    {/if}

    {#if !isMac}
      <button type="button" class="win-btn" aria-label="Minimize the window" title="Minimize" onclick={minimizeWindow}>
        <svg viewBox="0 0 24 24" class="h-3.5 w-3.5" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"><path d="M5 12h14" /></svg>
      </button>
      <button
        type="button"
        class="win-btn"
        aria-label={maximized ? "Restore the window size" : "Maximize the window"}
        title={maximized ? "Restore" : "Maximize"}
        onclick={toggleMaximizeWindow}
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

  {#if tabsBelow && visible && tabs.length > 0}
    <div class="flex h-10 shrink-0 items-stretch gap-2 border-b border-edge bg-ink-950 px-2">
      {@render tabStrip()}
    </div>
  {/if}
</div>

{#snippet tabStrip()}
  <div
    class="surface-enter tab-strip flex min-w-0 flex-1 items-end gap-1 self-stretch overflow-x-auto pt-1.5"
    role="tablist"
    aria-label="Open chats"
  >
    {#each tabs as tab, i (tab.key)}
      <div
        class={`tab group/tab flex h-8 max-w-52 min-w-0 shrink-0 items-center gap-1 rounded-t-lg border-x border-t px-2 text-sm transition-colors ${
          tab.key === activeKey
            ? "border-ink-700 bg-ink-900 font-medium text-ink-50"
            : "border-transparent text-ink-400 hover:bg-veil/5 hover:text-ink-100"
        }`}
      >
        <div
          role="tab"
          tabindex={tab.key === rovingKey ? 0 : -1}
          aria-selected={tab.key === activeKey}
          aria-label={`Chat tab: ${tab.title}${tab.busy ? " (answering)" : ""}`}
          class="flex min-w-0 flex-1 cursor-pointer items-center gap-1.5 rounded-sm focus-visible:outline focus-visible:outline-2 focus-visible:outline-ember-400"
          onclick={() => onFocus?.(tab.key)}
          onkeydown={(e) => onTabKeydown(e, i)}
        >
          {#if tab.busy}
            <span class="busy-dot" aria-hidden="true"></span>
          {:else}
            <LogoMark class="h-3.5 w-3.5 shrink-0 opacity-70" />
          {/if}
          <span class="min-w-0 flex-1 truncate" title={tab.title}>{tab.title}</span>
        </div>
        <button
          type="button"
          class="tab-close grid h-5 w-5 shrink-0 place-items-center rounded-md text-ink-500 opacity-40 transition-[opacity,color] group-hover/tab:opacity-100 hover:bg-veil/10 hover:text-ink-50 hover:opacity-100 focus-visible:opacity-100"
          aria-label={`Close the tab ${tab.title}`}
          onclick={() => onClose?.(tab.key)}
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
{/snippet}

<style>
  .win-btn {
    display: grid;
    height: 2.25rem;
    width: 2.75rem;
    place-items: center;
    color: var(--color-ink-400);
    transition: background-color var(--dur-fast) ease-out, color var(--dur-fast) ease-out;
  }
  .win-btn:hover {
    background: color-mix(in oklab, var(--color-veil) 10%, transparent);
    color: var(--color-ink-50);
  }
  .logo-btn {
    color: var(--color-ink-300);
  }
  .logo-btn:hover {
    color: var(--color-ink-50);
  }
  .menu-item {
    display: block;
    width: 100%;
    padding: 0.5rem 0.75rem;
    text-align: left;
    font-size: 0.875rem;
    color: var(--color-ink-200);
  }
  .menu-item:hover {
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
    animation: tab-pulse var(--dur-loop) ease-in-out infinite;
  }
  /* Touch has no hover: the close affordance stays at full opacity there. */
  @media (pointer: coarse), (hover: none) {
    .tab-close {
      opacity: 1;
    }
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
