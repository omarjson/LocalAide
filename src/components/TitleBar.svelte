  // One chrome row for the whole window: identity, chat tabs and window
  // controls. Native decorations are off on Windows and Linux, so the custom
  // buttons stay there; macOS keeps its own traffic lights and hides them.
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Icon from "./Icon.svelte";
  import LogoMark from "./LogoMark.svelte";
  import { platform } from "../lib/platform";
  import { flag } from "../lib/dev.svelte";
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

  // A real tablist: one tab is in the tab order (roving tabindex), and the
  // arrow keys move between tabs with automatic activation. The close button
  // is a sibling of the focusable tab face, never nested inside it.
  let tablistEl = $state<HTMLDivElement>();
  const tabbableKey = $derived(activeKey ?? tabs[0]?.key ?? null);
  const tabEls = () => Array.from(tablistEl?.querySelectorAll<HTMLElement>('[role="tab"]') ?? []);
  const onTabKeydown = (e: KeyboardEvent) => {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(e.key)) return;
    const els = tabEls();
    const at = els.findIndex((el) => el === document.activeElement);
    if (at < 0 || els.length === 0) return;
    e.preventDefault();
    const next = e.key === "Home" ? 0
      : e.key === "End" ? els.length - 1
      : (at + (e.key === "ArrowRight" ? 1 : els.length - 1)) % els.length;
    els[next].focus();
    const key = tabs[next]?.key;
    if (key) onFocus?.(key);
  };

  onMount(() => {
    const current = getCurrentWindow();
    win = current;
    let stop: (() => void) | undefined;
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
      <Icon name="menu" class="h-4 w-4" strokeWidth={1.5} />
    </button>
  {/if}

  <div class="flex flex-1 items-center gap-2 overflow-hidden">
    <!-- Tab List (Roving Tabindex) -->
    {#if tabs && tabs.length > 0}
      <div bind:this={tablistEl} role="tablist" class="flex overflow-x-auto">
        {#each tabs as tab (tab.key)}
          <button
            type="button"
            role="tab"
            aria-selected={activeKey === tab.key}
            on:keydown={onTabKeydown}
            tabindex={activeKey === tab.key ? "0" : "-1"}
            class={`flex items-center gap-2 rounded-md px-3 py-1 text-sm transition-colors duration-150 ${activeKey === tab.key
              ? "bg-ink-800 font-semibold text-ink-50"
              : "text-ink-300 hover:bg-veil/5 hover:text-ink-50"}`}
            onclick={() => onFocus?.(tab.key)}
          >
            <Icon name={tab.icon} class="h-[18px] w-[18px]" size={tab.icon ? "20px" : "20px"} strokeWidth={2} />
            <span class="truncate">{tab.title}</span>
            {#if tab.closable}
              <button
                type="button"
                class="ml-2 text-ink-500 hover:text-ember-500 transition-colors"
                onclick={() => onClose?.(tab.key)}
                aria-label={`Close ${tab.title}`}
              >
                <Icon name="x" class="h-4 w-4" size={20} strokeWidth={2} />
              </button>
            {/if}
          </button>
        {/each}
      </div>
    {/if}
  </div>

  {#if !tabs || tabs.length === 0}
    <div class="flex items-center gap-2">
      <LogoMark />
    </div>
  {/if}

  {#if onNew}
    <button type="button" class="ml-2 flex h-8 w-8 items-center justify-center rounded-full bg-ember-600 text-white transition-colors hover:bg-ember-500" onclick={onNew} aria-label="New chat" title="New Chat">
      <Icon name="plus" class="h-4 w-4" size={20} strokeWidth={2} />
    </button>
  {/if}
</header>