<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { fade, fly, scale } from "svelte/transition";
  import { api, errorMessage } from "./lib/api/client";
  import { dur, enterDialog, enterScreen, leaveDialog, motion, slideUp } from "./lib/motion";
  import { formatBytes } from "./lib/format";
  import { applyTheme, currentTheme } from "./lib/theme";
  import { LEGAL_DOCS, legal, openLegal } from "./lib/legal.svelte";
  import { requestSettings, screenRequest } from "./lib/nav.svelte";
  import { loadDev, moduleOn, activeEffectFlags } from "./lib/dev.svelte";
  import { SCREEN_REGISTRY, screenForKey, type ScreenId } from "./lib/screens";
  import { loadVoice } from "./lib/voice.svelte";
  import { refreshShare, share } from "./lib/share.svelte";
  import MarkdownDoc from "./components/MarkdownDoc.svelte";
  import TitleBar from "./components/TitleBar.svelte";
  import Modal from "./components/Modal.svelte";
  import type { IconName } from "./components/Icon.svelte";
  import { dismissToast, showToast, toast } from "./lib/toast.svelte";
  import { portal } from "./lib/portal";
  import SetupWizard from "./screens/SetupWizard.svelte";
  import ChatScreen from "./screens/ChatScreen.svelte";
  import FocusTrap from "./components/FocusTrap.svelte";
  import Icon from "./components/Icon.svelte";
  import ProgressBar from "./components/ProgressBar.svelte";
  import MemoryScreen from "./screens/MemoryScreen.svelte";
  import SkillsScreen from "./screens/SkillsScreen.svelte";
  import ModelManagerScreen from "./screens/ModelManagerScreen.svelte";
  import SettingsScreen from "./screens/SettingsScreen.svelte";
  import SearchScreen from "./screens/SearchScreen.svelte";
  import UsageScreen from "./screens/UsageScreen.svelte";
  import AutomationsScreen from "./screens/AutomationsScreen.svelte";
  import SidebarChats from "./components/SidebarChats.svelte";
  import ChatSearchModal from "./components/ChatSearchModal.svelte";
  import SessionPicker from "./components/SessionPicker.svelte";
  import MultiRunModal from "./components/MultiRunModal.svelte";
  import MultiRunFusion from "./components/MultiRunFusion.svelte";
  import { notifyTurnFinished } from "./lib/notifications.svelte";
  import { chats, loadThreads, newChat, openChat, restoreTabs, seedTab, focusTab, closeTab, tabThreadStarted, tabTitleChanged, tabBusyChanged } from "./lib/chats.svelte";
  import { applyScale, nudgeScale } from "./lib/scale.svelte";
  import type { EngineDownloadEvent, SessionHit, UpdateCheckResponse } from "./lib/api/bindings";

  type Screen = ScreenId | "setup" | "skills";
  type NavId = ScreenId | "skills";
  const activeFlags = $derived(activeEffectFlags());

  // Null until startup decides: returning users with a downloaded model go
  // straight to chat; first-run users see the setup wizard.
  let screen = $state<Screen | null>(null);
  let ramTotal = $state(0);
  let ramUsable = $state(0);
  let budget = $state(0);
  let cmdOpen = $state(false);
  let cmdQuery = $state("");
  let cmdInput = $state<HTMLInputElement>();
  // Palette actions that open their own dialogs.
  let chatSearchOpen = $state(false);
  let sessionPickerOpen = $state(false);
  let compareOpen = $state(false);
  let fusionOpen = $state(false);
  // Last reported busy state per tab, so a turn-finish notification fires on
  // the true -> false edge only (not on mount or on repeated false reports).
  const workingTabs = new Map<string, boolean>();
  const tabBusy = (key: string, busy: boolean) => {
    const wasBusy = workingTabs.get(key) === true;
    workingTabs.set(key, busy);
    tabBusyChanged(key, busy);
    if (wasBusy && !busy && typeof document !== "undefined" && document.hidden) {
      notifyTurnFinished();
    }
  };
  let mobileNav = $state(false);
  // The sidebar can shrink to icons. Remembered for the next launch.
  const SIDEBAR_KEY = "localaide-sidebar";
  let collapsed = $state((() => {
    try {
      return localStorage.getItem(SIDEBAR_KEY) === "collapsed";
    } catch {
      return false;
    }
  })());
  const toggleSidebar = () => {
    // On narrow windows the sidebar is off-canvas, so the same shortcut
    // opens the drawer instead of toggling a collapse nobody can see.
    // (The title bar no longer carries its own menu button: it did nothing
    // on wide windows and duplicated this control.)
    if (typeof window !== "undefined" && window.matchMedia("(max-width: 1023px)").matches) {
      mobileNav = !mobileNav;
      return;
    }
    collapsed = !collapsed;
    try {
      localStorage.setItem(SIDEBAR_KEY, collapsed ? "collapsed" : "open");
    } catch {
      // Storage can be unavailable; the choice still applies now.
    }
  };
  // Narrow labels only on large screens; the mobile drawer always shows them.
  const showLabels = $derived(!collapsed || mobileNav);
  const navVisible = (id: NavId): boolean =>
    id === "search" ? moduleOn("search") : id === "memory" ? moduleOn("memory") : id === "skills" ? moduleOn("skills") : true;
  let showHelp = $state(false);
  // Set when the settings file cannot be read at startup. Showing setup then
  // would hide the real problem, so the app says what failed instead.
  let startupError = $state<string | null>(null);
  // True while setup downloads a model. Leaving setup then would hide the
  // download, so navigation waits until it ends or is cancelled.
  let setupBusy = $state(false);

  // The chat screen stays loaded once opened, so its conversation (and an
  // answer that is still streaming) survives a visit to another screen.
  let chatMounted = $state(false);
  $effect(() => {
    if (screen === "chat") {
      chatMounted = true;
      // Reopen the last session's tabs, or seed one fresh chat.
      if (!restoreTabs()) seedTab(chats.activeFolder);
    }
  });
  // Where each screen was scrolled, so coming back keeps the place.
  let canvasEl = $state<HTMLElement>();
  const scrollTops: Partial<Record<Screen, number>> = {};

  /** Shows a screen. Returns false when setup is busy and the screen stays. */
  const navigate = (s: Screen): boolean => {
    if (setupBusy && s !== "setup") {
      showToast("Wait for the download to finish, or cancel it first.");
      return false;
    }
    // The memory share for models can change in Settings, so read it again.
    if (screen === "settings" && s !== "settings") void refreshProbe();
    if (screen && canvasEl) scrollTops[screen] = canvasEl.scrollTop;
    screen = s;
    mobileNav = false;
    const top = scrollTops[s] ?? 0;
    void tick().then(() => tick()).then(() => {
      if (canvasEl) canvasEl.scrollTop = top;
    });
    return true;
  };

  // The model engine downloads the first time a model loads.
  let engineDownload = $state<EngineDownloadEvent | null>(null);
  onMount(() => {
    let stop: (() => void) | undefined;
    let closed = false;
    void listen<EngineDownloadEvent>("engine-download", (event) => {
      engineDownload = event.payload;
      if (event.payload.state === "ready") {
        showToast("The model engine for this processor is ready.");
        setTimeout(() => (engineDownload = null), 1500);
      } else if (event.payload.state === "failed") {
        showToast(`The model engine could not be downloaded: ${event.payload.message ?? "unknown error"}`, "error");
        engineDownload = null;
      }
    })
      .then((unlisten) => {
        if (closed) unlisten();
        else stop = unlisten;
      })
      .catch(() => undefined);
    return () => {
      closed = true;
      stop?.();
    };
  });

  // Screens can ask for another screen (for example chat opening Settings).
  let handledScreenTick = 0;
  $effect(() => {
    const tick = screenRequest.tick;
    const target = screenRequest.screen;
    if (tick === handledScreenTick || !target) return;
    handledScreenTick = tick;
    untrack(() => {
      if (screen !== target) navigate(target);
    });
  });

  $effect(() => {
    if (cmdOpen) {
      cmdInput?.focus();
    }
  });

  type Command = { id: string; label: string; hint?: string; icon: IconName; group: "Go to" | "Actions"; run: () => void };

  const toggleTheme = async () => {
    const next = currentTheme() === "dark" ? "light" : "dark";
    applyTheme(next);
    try {
      await api.setTheme(next);
    } catch (e: unknown) {
      showToast(`The theme could not be saved: ${errorMessage(e)}`, "error");
    }
  };

  // macOS shows ⌘, everyone else Ctrl.
  const mod = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.userAgent) ? "⌘" : "Ctrl+";

  const commands = $derived<Command[]>([
    // All "Go to" entries come from the one registry, so the hint is always
    // the key that opens that screen. Skills is a module with no number.
    ...SCREEN_REGISTRY.map((s) => ({
      id: s.id as string,
      label: s.label,
      hint: `${mod}${s.key}`,
      icon: s.icon,
      group: "Go to" as const,
      run: () => { navigate(s.id); },
    })),
    { id: "skills", label: "Skills", icon: "skills", group: "Go to", run: () => navigate("skills") },
    { id: "new-chat", label: "New chat", hint: `${mod}N`, icon: "plus", group: "Actions", run: () => { if (navigate("chat")) newChat(chats.activeFolder); } },
    { id: "sidebar", label: "Collapse or expand the sidebar", hint: `${mod}B`, icon: "sidebar", group: "Actions", run: toggleSidebar },
    { id: "search-chats", label: "Search chats", icon: "history", group: "Actions", run: () => { chatSearchOpen = true; } },
    { id: "session-picker", label: "Pick a session (saved or open)", icon: "chat", group: "Actions", run: () => { sessionPickerOpen = true; } },
    { id: "compare-models", label: "Compare models", icon: "compare", group: "Actions", run: () => { compareOpen = true; } },
    { id: "fuse-models", label: "Compare and fuse model answers", icon: "merge", group: "Actions", run: () => { fusionOpen = true; } },
    { id: "theme", label: "Switch light or dark theme", icon: "sun", group: "Actions", run: () => void toggleTheme() },
    { id: "providers", label: "Online providers", icon: "cloud", group: "Actions", run: () => { if (navigate("settings")) requestSettings("online"); } },
    { id: "updates", label: "Check for updates", icon: "download", group: "Actions", run: () => { if (navigate("settings")) requestSettings("about", true); } },
    { id: "privacy", label: "Privacy Policy", icon: "shield", group: "Actions", run: () => openLegal("privacy") },
    { id: "terms", label: "Terms of Use", icon: "scale", group: "Actions", run: () => openLegal("terms") },
    { id: "help", label: "Keyboard shortcuts", hint: "?", icon: "info", group: "Actions", run: () => { showHelp = true; } },
  ]);

  let cmdIndex = $state(0);
  const filteredCommands = $derived(
    commands
      .filter((c) => c.group !== "Go to" || navVisible(c.id as NavId))
      .filter((c) => c.id !== "providers" || moduleOn("online"))
      .filter((c) => c.label.toLowerCase().includes(cmdQuery.trim().toLowerCase())),
  );

  const closePalette = () => {
    cmdOpen = false;
    cmdQuery = "";
    cmdIndex = 0;
  };

  const runCommand = (c: Command) => {
    closePalette();
    c.run();
  };

  /** A chat-search hit opens a real tab via the shared chats helper. */
  const openChatHit = (hit: SessionHit) => {
    chatSearchOpen = false;
    if (!navigate("chat")) return;
    const saved = chats.threads.find((t) => t.id === hit.thread_id);
    openChat(saved ?? {
      id: hit.thread_id,
      title: hit.title,
      created_at: 0,
      updated_at: 0,
      message_count: 0,
      folder: null,
    });
  };

  const paletteKeydown = (e: KeyboardEvent) => {
    const count = filteredCommands.length;
    if (e.key === "ArrowDown" && count) {
      e.preventDefault();
      cmdIndex = (cmdIndex + 1) % count;
    } else if (e.key === "ArrowUp" && count) {
      e.preventDefault();
      cmdIndex = (cmdIndex - 1 + count) % count;
    } else if (e.key === "Enter" && count) {
      e.preventDefault();
      runCommand(filteredCommands[Math.min(cmdIndex, count - 1)]);
    }
  };

  $effect(() => {
    // A new query starts at the top match.
    void cmdQuery;
    cmdIndex = 0;
  });

  $effect(() => {
    if (!cmdOpen) return;
    document.getElementById(`cmd-${cmdIndex}`)?.scrollIntoView({ block: "nearest" });
  });

  const refreshProbe = async () => {
    try {
      const probe = await api.systemProbe();
      ramTotal = probe.total_ram_gb * 1024 * 1024 * 1024;
      ramUsable = probe.available_ram_gb * 1024 * 1024 * 1024;
      budget = probe.budget_gb * 1024 * 1024 * 1024;
    } catch (e: unknown) {
      console.error("System details could not load", e);
    }
  };

  const decideStartScreen = async () => {
    startupError = null;
    try {
      const cfg = await api.loadConfig();
      // Setup is done when the wizard finished (or was skipped), when a
      // local model is chosen, or when an online provider is saved. The
      // local-models case alone sent online-only users back to setup on
      // every launch, including after an update.
      screen =
        cfg.setup_complete || cfg.selected_models?.length || cfg.has_online_providers
          ? "chat"
          : "setup";
    } catch (e: unknown) {
      startupError = errorMessage(e);
    }
  };

  // A release found on its own, and its install progress for the chip.
  let autoUpdate = $state<UpdateCheckResponse | null>(null);
  let updateStage = $state<"idle" | "running" | "done" | "failed">("idle");
  let updatePercent = $state(0);
  // The chip opens this first: the release notes, then Install. Installing
  // straight from the chip gave no chance to read what changes.
  let updateModalOpen = $state(false);
  const installFromChip = async () => {
    if (!autoUpdate || updateStage === "running") return;
    if (!autoUpdate.can_install) {
      void api.openLink(autoUpdate.release_url);
      return;
    }
    updateStage = "running";
    updatePercent = 0;
    try {
      await api.installUpdate((event) => {
        updatePercent = event.total ? Math.min(100, Math.round((event.downloaded / event.total) * 100)) : updatePercent;
        if (event.stage === "installing") updatePercent = 100;
      });
      updateStage = "done";
    } catch {
      updateStage = "failed";
    }
  };

  onMount(() => {
    void (async () => {
      try {
        const theme = await api.getTheme();
        if (theme === "light" || theme === "dark") {
          applyTheme(theme);
        }
      } catch (e: unknown) {
        console.error("Theme could not load", e);
      }
      await loadDev();
      void applyScale();
      void loadThreads();
      void loadVoice();
      void refreshShare();
      await decideStartScreen();
      await refreshProbe();
      // One quiet update check shortly after launch, so a new release is
      // found without digging through Settings. Never blocks startup.
      setTimeout(() => {
        void api.checkForUpdate()
          .then((res) => {
            if (res.update_available) autoUpdate = res;
          })
          .catch(() => undefined);
      }, 6000);
    })();

    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const isInput = target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable);

      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        if (moduleOn("command_palette")) cmdOpen = !cmdOpen;
      } else if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "b" && !isInput) {
        e.preventDefault();
        toggleSidebar();
      } else if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "n") {
        e.preventDefault();
        if (navigate("chat")) {
          newChat(chats.activeFolder);
          showToast("New chat ready");
        }
      } else if ((e.metaKey || e.ctrlKey) && !e.altKey && (e.key === "=" || e.key === "+")) {
        e.preventDefault();
        void nudgeScale(0.1);
      } else if ((e.metaKey || e.ctrlKey) && !e.altKey && e.key === "-") {
        e.preventDefault();
        void nudgeScale(-0.1);
      } else if ((e.metaKey || e.ctrlKey) && !e.altKey && e.key === "0") {
        e.preventDefault();
        void nudgeScale(0);
      } else if ((e.metaKey || e.ctrlKey) && e.key === ",") {
        e.preventDefault();
        navigate("settings");
      } else if ((e.metaKey || e.ctrlKey) && /^[1-7]$/.test(e.key)) {
        e.preventDefault();
        const target = screenForKey(Number(e.key));
        if (target && navVisible(target.id)) navigate(target.id);
      } else if (e.key === "/" && !e.metaKey && !e.ctrlKey && !e.altKey && !isInput) {
        e.preventDefault();
        navigate("search");
      }

      if (e.key === "Escape") {
        closePalette();
        showHelp = false;
        mobileNav = false;
      }
      if (e.key === "?" && !e.metaKey && !e.ctrlKey && !e.altKey && !isInput) {
        e.preventDefault();
        showHelp = !showHelp;
      }
    };

    // An unexpected error shows a message and leaves the screen as it was.
    const onError = (e: ErrorEvent) => {
      console.error(e.error ?? e.message);
      showToast(`Something went wrong: ${errorMessage(e.error ?? e.message)}`, "error");
    };

    const onRejection = (e: PromiseRejectionEvent) => {
      console.error(e.reason);
      showToast(`Something went wrong: ${errorMessage(e.reason)}`, "error");
    };

    window.addEventListener("keydown", onKey);
    window.addEventListener("error", onError);
    window.addEventListener("unhandledrejection", onRejection);

    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("error", onError);
      window.removeEventListener("unhandledrejection", onRejection);
    };
  });
</script>

<main class="h-screen flex flex-col overflow-hidden bg-ink-950 text-ink-100">
  <TitleBar
    tabs={chats.tabs}
    activeKey={chats.activeTabKey}
    visible={screen === "chat"}
    onFocus={focusTab}
    onClose={closeTab}
    onNew={() => newChat(chats.activeFolder)}
  />
  <div class="flex flex-1 min-h-0">
  {#if mobileNav}
    <!-- Starts under the chrome bar (h-10), so the window buttons stay reachable. -->
    <div role="presentation" class="fixed inset-x-0 bottom-0 top-10 z-30 bg-black/50 backdrop-blur-[2px] lg:hidden" in:fade={{ duration: dur(motion.base) }} out:fade={{ duration: dur(motion.fast) }} onclick={() => mobileNav = false}></div>
  {/if}
  <!-- The title bar above carries no logo or name (on macOS it is just
       the traffic lights and the drag area), so the sidebar starts with
       its actions; on narrow windows it slides in under the
       title bar rather than over it. -->
  <aside class={`fixed lg:relative top-10 bottom-0 lg:top-auto lg:bottom-auto left-0 z-40 shrink-0 bg-ink-950 border-r border-edge lg:border-r-0 flex flex-col px-1.5 pb-1.5 pt-2 transition-[transform,width] duration-300 ease-out w-64 ${collapsed ? 'lg:w-[60px]' : 'lg:w-64'} ${mobileNav ? 'translate-x-0' : '-translate-x-full lg:translate-x-0'}`} aria-label="Main navigation">
    <div class="lg:hidden flex h-9 shrink-0 items-center justify-end px-1">
      <button aria-label="Close navigation" onclick={() => mobileNav = false} class="p-2 rounded-md hover:bg-veil/5"><Icon name="close" class="w-4 h-4" /></button>
    </div>
    {#if showLabels}
      <button
        type="button"
        onclick={() => { if (navigate("chat")) newChat(chats.activeFolder); }}
        class="mx-1 mb-2 flex h-9 shrink-0 items-center gap-2 rounded-lg border border-ember-500/25 bg-ember-500/10 px-3 text-[13px] font-semibold text-ember-200 transition-colors hover:bg-ember-500/15"
        title="New chat (Ctrl+N)"
      >
        <Icon name="plus" class="h-4 w-4" />
        New chat
        <kbd class="ml-auto rounded bg-veil/5 px-1.5 py-0.5 font-mono text-xs font-normal text-ink-400">{mod}N</kbd>
      </button>
    {:else}
      <button
        type="button"
        onclick={() => { if (navigate("chat")) newChat(chats.activeFolder); }}
        aria-label="New chat"
        title="New chat (Ctrl+N)"
        class="mx-1 mb-2 grid h-9 w-9 shrink-0 place-items-center rounded-lg border border-ember-500/25 bg-ember-500/10 text-ember-200 transition-[background-color,transform] duration-200 hover:scale-110 hover:bg-ember-500/15 active:scale-95"
      ><Icon name="plus" class="h-4 w-4" /></button>
    {/if}
    {#snippet navItem(id: NavId, label: string)}
      <button
        onclick={() => navigate(id)}
        data-nav={id}
        aria-current={screen === id ? "page" : undefined}
        aria-label={showLabels ? undefined : label}
        title={showLabels ? undefined : label}
        class={`nav-item group relative w-full flex items-center h-10 rounded-lg text-sm text-left transition-[background-color,color] duration-150 ${showLabels ? 'gap-3.5 px-3' : 'justify-center px-0'} ${screen === id ? 'bg-ink-800 text-ink-50 font-semibold shadow-sm' : 'text-ink-300 hover:bg-veil/5 hover:text-ink-50'}`}
      >
        <span class={`absolute left-0 top-1/2 w-[3px] -translate-y-1/2 rounded-full bg-ember-500 transition-[height,opacity] duration-200 ease-out ${screen === id ? 'h-4 opacity-100' : 'h-0 opacity-0'}`} aria-hidden="true"></span>
        <span class={`nav-ico ${screen === id ? 'text-ember-300' : ''}`}><Icon name={id} class="w-[18px] h-[18px] shrink-0" /></span>
        {#if showLabels}<span class="truncate">{label}</span>{/if}
      </button>
    {/snippet}
    <nav class="flex-1 overflow-y-auto overflow-x-hidden space-y-0.5 pt-1" aria-label="Screens">
      {#each SCREEN_REGISTRY as s (s.id)}
        {#if s.id !== "settings" && navVisible(s.id)}{@render navItem(s.id, s.label)}{/if}
      {/each}
      {#if navVisible("skills")}{@render navItem("skills", "Skills")}{/if}
      {#if showLabels && screen !== "setup"}
        <div class="mt-2 border-t border-ink-800">
          <SidebarChats onOpen={() => navigate("chat")} />
        </div>
      {/if}
    </nav>
    <div class="shrink-0 space-y-0.5 pt-1.5 border-t border-ink-800">
      {@render navItem("settings", "Settings")}
      <button
        type="button"
        onclick={toggleSidebar}
        aria-label={collapsed ? "Expand the sidebar" : "Collapse the sidebar"}
        title={collapsed ? "Expand the sidebar (Ctrl+B)" : "Collapse the sidebar (Ctrl+B)"}
        aria-expanded={!collapsed}
        class={`hidden lg:flex w-full items-center h-9 rounded-lg text-sm text-ink-400 transition-colors hover:bg-veil/5 hover:text-ink-50 ${showLabels ? 'gap-3.5 px-3' : 'justify-center'}`}
      >
        <Icon name="sidebar" class={`w-[18px] h-[18px] shrink-0 transition-transform duration-300 ${collapsed ? 'rotate-180' : ''}`} />
        {#if showLabels}<span>Collapse</span>{/if}
      </button>
    </div>
  </aside>

  <section bind:this={canvasEl} class={`app-canvas min-h-0 min-w-0 flex-1 flex flex-col bg-ink-900 lg:border-l lg:border-t border-ink-700 lg:rounded-tl-xl ${screen === "chat" ? "overflow-hidden" : "overflow-y-auto"}`}>
    {#if chatMounted && !startupError}
      <div class="min-h-0 flex-1 flex flex-col" class:hidden={screen !== "chat"}>
        {#each chats.tabs as tab (tab.key)}
          <div class="min-h-0 flex-1" class:hidden={tab.key !== chats.activeTabKey}>
            <ChatScreen
              onOpenModels={() => navigate("models")}
              active={screen === "chat" && tab.key === chats.activeTabKey}
              startThread={{ id: tab.id, folder: tab.folder }}
              onThreadStarted={(id) => tabThreadStarted(tab.key, id)}
              onTitleChanged={(title) => tabTitleChanged(tab.key, title)}
              onBusyChanged={(busy) => tabBusy(tab.key, busy)}
              onNewChatRequested={() => newChat(tab.folder)}
            />
          </div>
        {/each}
      </div>
    {/if}
    <div class={screen === "chat" ? "hidden" : "flex-1 p-6 md:p-10"}>
      {#key screen}
        <div in:fly={enterScreen()} class={screen === "chat" ? "h-full" : ""}>
      {#if startupError}
        <div class="max-w-xl mx-auto my-12 p-8 rounded-2xl border border-rose-500/30 bg-rose-950/20 text-center" role="alert">
          <div class="text-rose-300 mb-3 flex justify-center"><Icon name="alert" class="w-7 h-7" /></div>
          <h2 class="text-lg font-semibold text-rose-200 mb-2">LocalAide could not read its settings</h2>
          <p class="text-sm text-ink-300 mb-6 break-words">{startupError}</p>
          <div class="flex justify-center gap-3">
            <button onclick={() => void decideStartScreen()} class="px-4 py-2 rounded-xl bg-veil/10 hover:bg-veil/20 text-sm font-medium transition-colors">Try again</button>
            <button onclick={() => { startupError = null; screen = "chat"; }} class="px-4 py-2 rounded-xl border border-edge hover:bg-veil/5 text-sm text-ink-300 transition-colors">Continue anyway</button>
          </div>
        </div>
      {:else if screen === null}
        <div class="max-w-2xl mx-auto my-24 flex flex-col items-center gap-3">
          <div class="w-10 h-10 rounded-full border-4 border-edge border-t-ember-500 animate-spin"></div>
          <p class="text-sm text-ink-400">Starting LocalAide…</p>
        </div>
      {:else if screen === "setup"}
        <SetupWizard onFinished={() => { setupBusy = false; void api.markSetupComplete().catch((e: unknown) => showToast(`Setup could not be saved as finished: ${errorMessage(e)}`, "error")); navigate("chat"); }} onBusyChange={(busy) => setupBusy = busy} onSkip={() => { void api.markSetupComplete().catch((e: unknown) => showToast(`Setup could not be saved as finished: ${errorMessage(e)}`, "error")); navigate("chat"); }} />
      {:else if screen === "chat"}
        <!-- The chat lives outside this block, so it is not reloaded. -->
      {:else if screen === "search"}
        <SearchScreen />
      {:else if screen === "memory"}
        <MemoryScreen />
      {:else if screen === "skills"}
        <SkillsScreen />
      {:else if screen === "models"}
        <ModelManagerScreen />
      {:else if screen === "automations"}
        <AutomationsScreen />
      {:else if screen === "usage"}
        <UsageScreen />
      {:else if screen === "settings"}
        <SettingsScreen onSectionChange={() => { if (canvasEl) canvasEl.scrollTop = 0; }} />
      {/if}
        </div>
      {/key}
    </div>
  </section>
  </div>

  <footer class="h-8 shrink-0 flex items-center gap-5 px-4 text-xs text-ink-500 bg-ink-950" aria-label="Status">
    {#if ramTotal > 0}
      <span class="flex items-center gap-2.5"><span class="h-1.5 w-1.5 rounded-full bg-emerald-400/80" aria-hidden="true"></span>{formatBytes(ramUsable)} free of {formatBytes(ramTotal)}</span>
      <span class="hidden md:inline">Models can use up to {formatBytes(budget)}</span>
    {/if}
    {#if share.status?.on}
      <button type="button" onclick={() => { if (navigate("settings")) requestSettings("share"); }} class="sharing-on" title={`Anyone with a link can reach this assistant: ${share.status.url ?? ""}`}>
        <span class="sharing-dot" aria-hidden="true"></span>Sharing is on · {share.status.requests} {share.status.requests === 1 ? "request" : "requests"}
      </button>
    {/if}
    {#if autoUpdate?.update_available}
      <button
        type="button"
        onclick={() => (updateModalOpen = true)}
        class={`flex items-center gap-2 rounded px-1.5 py-0.5 font-medium transition-colors ${updateStage === "failed" ? "text-rose-300 hover:text-rose-200" : "text-ember-200 hover:text-ember-100"}`}
        title={autoUpdate.can_install ? "See what's new, then install" : autoUpdate.release_url}
      >
        <span class="h-1.5 w-1.5 rounded-full bg-ember-400" aria-hidden="true"></span>
        {#if updateStage === "running"}
          Updating… {updatePercent}%
        {:else if updateStage === "done"}
          Installed — restarting
        {:else if updateStage === "failed"}
          Update failed — see what's new
        {:else}
          LocalAide {autoUpdate.latest_version ?? ""} is ready — see what's new
        {/if}
      </button>
    {/if}
    <span class="ml-auto hidden sm:flex items-center gap-2">
      <button type="button" onclick={() => (showHelp = true)} class="flex items-center gap-2 rounded px-1.5 py-0.5 text-ink-500 transition-colors hover:text-ink-200" title="Keyboard shortcuts (?)">Keyboard shortcuts <kbd class="rounded border border-ink-700 bg-veil/5 px-1.5 py-0.5 font-mono text-xs text-ink-400">?</kbd></button>
    </span>
    {#if activeFlags.length > 0}
      <span class="debug-chip" title={`Developer effects on: ${activeFlags.join(", ")}`}><Icon name="flask" class="h-3.5 w-3.5" />{activeFlags.length} debug {activeFlags.length === 1 ? "flag" : "flags"}</span>
    {/if}
  </footer>

  {#if updateModalOpen && autoUpdate?.update_available}
    {@const update = autoUpdate}
    <Modal title={`LocalAide ${update.latest_version ?? ""} is ready`} size="lg" onClose={() => (updateModalOpen = false)}>
      <p class="text-sm leading-6 text-ink-400">You have {update.current_version}. Updates keep your chats, notes, models and preferences.</p>
      {#if update.notes}
        <div class="mt-4 max-h-64 overflow-y-auto rounded-md border border-ink-800 bg-ink-950 p-3">
          <MarkdownDoc source={update.notes} />
        </div>
      {:else}
        <p class="mt-4 text-sm text-ink-400">No release notes were published for this version.</p>
      {/if}
      {#if updateStage === "running"}
        <p class="mt-4 text-sm text-ink-300" aria-live="polite">Updating… {updatePercent}%</p>
      {:else if updateStage === "done"}
        <p class="mt-4 text-sm text-emerald-300">Installed — restarting LocalAide…</p>
      {:else if updateStage === "failed"}
        <p class="mt-4 text-sm text-rose-300">The update failed. Your current version still works — try again.</p>
      {/if}
      {#snippet footer()}
        <button type="button" onclick={() => (updateModalOpen = false)} class="rounded-lg px-4 py-2 text-sm text-ink-300 hover:bg-veil/5">Later</button>
        {#if update.can_install}
          <button type="button" onclick={() => void installFromChip()} disabled={updateStage === "running" || updateStage === "done"} class="rounded-lg bg-ember-500 px-4 py-2 text-sm font-semibold text-on-ember hover:bg-ember-400 disabled:opacity-50">
            {updateStage === "running" ? `Updating… ${updatePercent}%` : updateStage === "done" ? "Installed — restarting" : "Install and restart"}
          </button>
        {:else}
          <button type="button" onclick={() => void api.openLink(update.release_url)} class="rounded-lg bg-ember-500 px-4 py-2 text-sm font-semibold text-on-ember hover:bg-ember-400">Open the download page</button>
        {/if}
      {/snippet}
    </Modal>
  {/if}

  {#if cmdOpen}
    <div role="presentation" in:fade={{ duration: dur(motion.fast) }} out:fade={{ duration: dur(80) }} class="fixed inset-0 bg-black/60 backdrop-blur-sm grid place-items-start pt-[18vh] px-4 z-50" onclick={closePalette} onkeydown={(e) => { if (e.key === 'Escape') closePalette(); }}>
      <div role="dialog" tabindex="-1" aria-modal="true" aria-label="Command palette" in:scale={enterDialog()} out:scale={leaveDialog()} class="w-full max-w-lg mx-auto rounded-xl border border-edge bg-ink-950 shadow-2xl overflow-hidden" onclick={(e) => e.stopPropagation()} onkeydown={(e) => { if (e.key === 'Escape') closePalette(); e.stopPropagation(); }}>
        <FocusTrap>
        <div class="flex items-center gap-3 px-4 py-3 border-b border-edge">
          <span class="text-ink-400"><Icon name="search" /></span>
          <input
            aria-label="Search commands"
            role="combobox"
            aria-expanded="true"
            aria-controls="cmd-list"
            aria-activedescendant={filteredCommands.length ? `cmd-${cmdIndex}` : undefined}
            bind:this={cmdInput}
            bind:value={cmdQuery}
            onkeydown={paletteKeydown}
            placeholder="Go to a screen or run a command…"
            class="field-bare flex-1 bg-transparent text-sm placeholder:text-ink-500"
          />
          <kbd class="text-xs px-1.5 py-0.5 rounded bg-ink-800 text-ink-400">Esc</kbd>
        </div>
        <div id="cmd-list" role="listbox" aria-label="Commands" class="p-2 max-h-80 overflow-auto">
          {#each filteredCommands as c, i (c.id)}
            {#if i === 0 || filteredCommands[i - 1].group !== c.group}
              <div class="px-3 pb-1 pt-2 text-xs font-semibold text-ink-500" role="presentation">{c.group}</div>
            {/if}
            <button
              id={`cmd-${i}`}
              role="option"
              aria-selected={i === cmdIndex}
              tabindex="-1"
              onmouseenter={() => cmdIndex = i}
              onclick={() => runCommand(c)}
              class={`w-full flex items-center gap-3 px-3 py-2 rounded-md text-left transition-colors duration-100 ${i === cmdIndex ? 'bg-veil/10 text-ink-50' : 'text-ink-200'}`}
            >
              <span class={`size-8 grid place-items-center rounded-md transition-colors ${i === cmdIndex ? 'bg-ember-500/15 text-ember-300' : 'bg-veil/5'}`}><Icon name={c.icon} /></span>
              <span class="flex-1 text-sm">{c.label}</span>
              {#if c.hint}<kbd class="text-xs text-ink-500">{c.hint}</kbd>{/if}
            </button>
          {:else}
            <div class="px-3 py-6 text-center text-sm text-ink-400">No matches</div>
          {/each}
        </div>
        </FocusTrap>
      </div>
    </div>
  {/if}

  {#if engineDownload && engineDownload.state === "downloading"}
    <div class="fixed bottom-12 right-4 z-50 w-72 rounded-xl border border-edge bg-ink-950 p-3 shadow-2xl" role="status" in:slideUp={{ y: 8 }} out:fade={{ duration: dur(motion.fast) }}>
      <div class="flex items-center gap-2 text-sm font-medium text-ink-100"><Icon name="download" class="h-4 w-4 text-ember-300" />Getting the model engine</div>
      <p class="mt-1 text-xs leading-5 text-ink-400">Once only: the llama.cpp build for this processor, from github.com.</p>
      <ProgressBar value={engineDownload.percent} min={3} label="Getting the model engine" class="mt-2 h-1.5" />
      <p class="mt-1 text-right text-xs tabular-nums text-ink-500">{formatBytes(engineDownload.downloaded_bytes)} of {formatBytes(engineDownload.total_bytes)}</p>
    </div>
  {/if}

  <!-- Always in the page, so screen readers hear each new message.
       Portaled to the document root: reporting from the shell costs the
       page no layout (a `position: fixed` node inside a transformed
       ancestor would be positioned against that ancestor instead). -->
  <div use:portal class="fixed bottom-10 left-1/2 z-50 -translate-x-1/2" role={toast.tone === "error" ? "alert" : "status"}>
    {#if toast.message}
      <div
        in:fly={{ y: 12, duration: dur(motion.base), easing: motion.enter }}
        out:fly={{ y: 6, duration: dur(motion.fast) }}
        class={`flex max-w-[min(36rem,90vw)] items-start gap-3 rounded-lg border px-4 py-2 text-sm shadow-lg ${toast.tone === "error" ? "border-rose-500/40 bg-ink-900 text-rose-200" : "border-edge bg-ink-900 text-ink-100"}`}
      >
        <span class="min-w-0 break-words">{toast.message}</span>
        {#if toast.tone === "error"}
          <button type="button" aria-label="Dismiss message" onclick={dismissToast} class="shrink-0 rounded p-0.5 hover:bg-veil/10"><Icon name="close" class="h-3.5 w-3.5" /></button>
        {/if}
      </div>
    {/if}
  </div>

  {#if showHelp}
    <div role="presentation" in:fade={{ duration: dur(100) }} out:fade={{ duration: dur(80) }} class="fixed inset-0 bg-black/70 backdrop-blur-sm grid place-items-center p-4 z-[60]" onclick={() => showHelp = false} onkeydown={(e) => { if (e.key === 'Escape') showHelp = false; }}>
      <div role="dialog" tabindex="-1" aria-modal="true" aria-label="Keyboard shortcuts" in:scale={enterDialog()} out:scale={leaveDialog()} class="w-full max-w-md rounded-xl border border-edge bg-ink-950 p-6 shadow-2xl" onclick={(e) => e.stopPropagation()} onkeydown={(e) => { if (e.key === 'Escape') showHelp = false; e.stopPropagation(); }}>
        <FocusTrap>
        <h3 class="text-lg font-semibold mb-4 text-ink-100">Keyboard shortcuts</h3>
        <div class="space-y-2.5 text-sm">
          <div class="flex justify-between items-center"><span class="text-ink-400">Command palette</span><kbd class="kbd">{mod}K</kbd></div>
          <div class="flex justify-between items-center"><span class="text-ink-400">New conversation</span><kbd class="kbd">{mod}N</kbd></div>
          <div class="flex justify-between items-center"><span class="text-ink-400">Switch screens ({SCREEN_REGISTRY.map((s) => `${s.key}: ${s.label}`).join(", ")})</span><kbd class="kbd">{mod}1 – {mod}7</kbd></div>
          <div class="flex justify-between items-center"><span class="text-ink-400">Settings</span><kbd class="kbd">{mod},</kbd></div>
          <div class="flex justify-between items-center"><span class="text-ink-400">Show or hide the sidebar</span><kbd class="kbd">{mod}B</kbd></div>
          <div class="flex justify-between items-center"><span class="text-ink-400">Search this chat (in Chat)</span><kbd class="kbd">{mod}F</kbd></div>
          <div class="flex justify-between items-center"><span class="text-ink-400">Choose the model (in Chat)</span><kbd class="kbd">{mod}Shift+M</kbd></div>
          <div class="flex justify-between items-center"><span class="text-ink-400">Thinking effort (in Chat)</span><kbd class="kbd">{mod}T</kbd></div>
          <div class="flex justify-between items-center"><span class="text-ink-400">Focus search</span><kbd class="kbd">/</kbd></div>
          <div class="flex justify-between items-center"><span class="text-ink-400">Interface scale — bigger, smaller, reset</span><span class="flex gap-1"><kbd class="kbd">{mod}+</kbd><kbd class="kbd">{mod}−</kbd><kbd class="kbd">{mod}0</kbd></span></div>
          <div class="flex justify-between items-center"><span class="text-ink-400">Send message</span><kbd class="kbd">Enter</kbd></div>
          <div class="flex justify-between items-center"><span class="text-ink-400">Cancel generation / dismiss a dialog</span><kbd class="kbd">Esc</kbd></div>
          <div class="flex justify-between items-center"><span class="text-ink-400">Keyboard help</span><kbd class="kbd">?</kbd></div>
        </div>
        <button onclick={() => showHelp = false} class="mt-6 w-full py-2 rounded-md bg-veil/5 border border-edge text-sm hover:bg-veil/10">Close</button>
        </FocusTrap>
      </div>
    </div>
  {/if}
  {#if chatSearchOpen}
    <ChatSearchModal onClose={() => (chatSearchOpen = false)} onOpen={openChatHit} />
  {/if}
  {#if sessionPickerOpen}
    <SessionPicker onClose={() => (sessionPickerOpen = false)} onOpen={openChatHit} />
  {/if}
  {#if compareOpen}
    <MultiRunModal onClose={() => (compareOpen = false)} />
  {/if}
  {#if fusionOpen}
    <MultiRunFusion onClose={() => (fusionOpen = false)} />
  {/if}
  {#if legal.open}
    {@const doc = LEGAL_DOCS[legal.open]}
    <Modal title={doc.title} size="lg" onClose={() => legal.open = null}>
      <MarkdownDoc source={doc.source} />
    </Modal>
  {/if}
</main>

<style>
  .kbd {
    border-radius: 0.25rem;
    border: 1px solid var(--color-edge);
    background: var(--color-ink-900);
    padding: 0.125rem 0.5rem;
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 0.75rem;
    color: var(--color-ink-200);
  }
  .sharing-on {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 22px;
    border-radius: 9999px;
    padding: 0 9px;
    font-weight: 600;
    background: color-mix(in oklab, var(--color-ember-500) 16%, transparent);
    color: var(--color-ember-200);
  }
  .sharing-dot {
    height: 7px;
    width: 7px;
    border-radius: 9999px;
    background: var(--color-ember-500);
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--color-ember-500) 25%, transparent);
  }
  .debug-chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 22px;
    border-radius: 9999px;
    padding: 0 9px;
    font-weight: 600;
    background: color-mix(in oklab, var(--color-ink-500) 16%, transparent);
    color: var(--color-ink-300);
  }
</style>
