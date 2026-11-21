<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { fade, fly, scale } from "svelte/transition";
  import { api, errorMessage } from "./lib/api/client";
  import { dur, enterDialog, enterScreen, leaveDialog, motion } from "./lib/motion";
  import { formatBytes } from "./lib/format";
  import { applyTheme, currentTheme } from "./lib/theme";
  import { LEGAL_DOCS, legal, openLegal } from "./lib/legal.svelte";
  import { requestSettings, screenRequest } from "./lib/nav.svelte";
  import { loadDev, moduleOn } from "./lib/dev.svelte";
  import { loadVoice } from "./lib/voice.svelte";
  import { refreshShare, share } from "./lib/share.svelte";
  import MarkdownDoc from "./components/MarkdownDoc.svelte";
  import Modal from "./components/Modal.svelte";
  import type { IconName } from "./components/Icon.svelte";
  import { dismissToast, showToast, toast } from "./lib/toast.svelte";
  import SetupWizard from "./screens/SetupWizard.svelte";
  import ChatScreen from "./screens/ChatScreen.svelte";
  import FocusTrap from "./components/FocusTrap.svelte";
  import Icon from "./components/Icon.svelte";
  import LogoMark from "./components/LogoMark.svelte";
  import MemoryScreen from "./screens/MemoryScreen.svelte";
  import SkillsScreen from "./screens/SkillsScreen.svelte";
  import ModelManagerScreen from "./screens/ModelManagerScreen.svelte";
  import SettingsScreen from "./screens/SettingsScreen.svelte";
  import SearchScreen from "./screens/SearchScreen.svelte";
  import SidebarChats from "./components/SidebarChats.svelte";
  import { chats, loadThreads, newChat } from "./lib/chats.svelte";
  import { applyScale, nudgeScale } from "./lib/scale.svelte";
  import type { EngineDownloadEvent } from "./lib/api/bindings";

  type Screen = "setup" | "chat" | "memory" | "skills" | "models" | "settings" | "search";
  type NavId = "chat" | "search" | "memory" | "skills" | "models" | "settings";

  // Null until startup decides: returning users with a downloaded model go
  // straight to chat; first-run users see the setup wizard.
  let screen = $state<Screen | null>(null);
  let ramTotal = $state(0);
  let ramUsable = $state(0);
  let budget = $state(0);
  let cmdOpen = $state(false);
  let cmdQuery = $state("");
  let cmdInput = $state<HTMLInputElement>();
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
    if (screen === "chat") chatMounted = true;
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
    { id: "chat", label: "Chat", hint: `${mod}1`, icon: "chat", group: "Go to", run: () => navigate("chat") },
    { id: "search", label: "Search", hint: `${mod}2`, icon: "search", group: "Go to", run: () => navigate("search") },
    { id: "memory", label: "Memory", hint: `${mod}3`, icon: "memory", group: "Go to", run: () => navigate("memory") },
    { id: "skills", label: "Skills", hint: `${mod}4`, icon: "skills", group: "Go to", run: () => navigate("skills") },
    { id: "models", label: "Models", hint: `${mod}5`, icon: "models", group: "Go to", run: () => navigate("models") },
    { id: "settings", label: "Settings", hint: `${mod}6`, icon: "settings", group: "Go to", run: () => navigate("settings") },
    { id: "new-chat", label: "New chat", hint: `${mod}N`, icon: "plus", group: "Actions", run: () => { if (navigate("chat")) newChat(chats.activeFolder); } },
    { id: "sidebar", label: "Collapse or expand the sidebar", hint: `${mod}B`, icon: "sidebar", group: "Actions", run: toggleSidebar },
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

  const shortcutScreens: NavId[] = ["chat", "search", "memory", "skills", "models", "settings"];

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
      screen = cfg.selected_models?.length ? "chat" : "setup";
    } catch (e: unknown) {
      startupError = errorMessage(e);
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
      } else if ((e.metaKey || e.ctrlKey) && /^[1-6]$/.test(e.key)) {
        e.preventDefault();
        const targetScreen = shortcutScreens[Number(e.key) - 1];
        if (targetScreen && navVisible(targetScreen)) navigate(targetScreen);
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
  <div class="flex flex-1 min-h-0">
  {#if mobileNav}
    <div role="presentation" class="fixed inset-0 z-30 bg-black/50 backdrop-blur-[2px] lg:hidden" in:fade={{ duration: dur(motion.base) }} out:fade={{ duration: dur(motion.fast) }} onclick={() => mobileNav = false}></div>
  {/if}
  <aside class={`fixed lg:relative inset-y-0 left-0 z-40 shrink-0 bg-ink-950 border-r border-edge lg:border-r-0 flex flex-col px-1.5 pb-1.5 transition-[transform,width] duration-300 ease-out w-64 ${collapsed ? 'lg:w-[60px]' : 'lg:w-64'} ${mobileNav ? 'translate-x-0' : '-translate-x-full lg:translate-x-0'}`} aria-label="Main navigation">
    <div class={`h-12 flex items-center gap-2.5 shrink-0 ${showLabels ? 'px-3' : 'px-0 justify-center'}`}>
      <LogoMark class="w-6 h-6 shrink-0" />
      {#if showLabels}<span class="text-sm font-semibold tracking-tight text-ink-50" in:fade={{ duration: dur(motion.fast) }}>LocalAide</span>{/if}
      <button aria-label="Close navigation" onclick={() => mobileNav = false} class="lg:hidden ml-auto p-2 rounded-md hover:bg-veil/5"><Icon name="close" class="w-4 h-4" /></button>
    </div>
    {#snippet navItem(id: NavId, label: string)}
      <button
        onclick={() => navigate(id)}
        aria-current={screen === id ? "page" : undefined}
        aria-label={showLabels ? undefined : label}
        title={showLabels ? undefined : label}
        class={`group relative w-full flex items-center h-10 rounded-lg text-sm text-left transition-[background-color,color] duration-150 ${showLabels ? 'gap-3.5 px-3' : 'justify-center px-0'} ${screen === id ? 'bg-ink-800 text-ink-50 font-semibold shadow-sm' : 'text-ink-300 hover:bg-veil/5 hover:text-ink-50'}`}
      >
        <span class={`absolute left-0 top-1/2 w-[3px] -translate-y-1/2 rounded-full bg-ember-500 transition-[height,opacity] duration-200 ease-out ${screen === id ? 'h-4 opacity-100' : 'h-0 opacity-0'}`} aria-hidden="true"></span>
        <Icon name={id} class={`w-[18px] h-[18px] shrink-0 transition-transform duration-200 ease-out ${screen === id ? 'text-ember-300' : 'group-hover:scale-110'}`} />
        {#if showLabels}<span class="truncate">{label}</span>{/if}
      </button>
    {/snippet}
    <nav class="flex-1 overflow-y-auto overflow-x-hidden space-y-0.5 pt-1" aria-label="Screens">
      {@render navItem("chat", "Chat")}
      {#if navVisible("search")}{@render navItem("search", "Search")}{/if}
      {#if navVisible("memory")}{@render navItem("memory", "Memory")}{/if}
      {#if navVisible("skills")}{@render navItem("skills", "Skills")}{/if}
      {@render navItem("models", "Models")}
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
    <div class="lg:hidden sticky top-0 z-30 shrink-0 bg-ink-950 p-3 border-b border-edge flex items-center gap-3">
      <button aria-label="Open navigation" onclick={() => mobileNav = true} class="p-2 rounded-md hover:bg-veil/5 text-ink-300"><Icon name="menu" class="w-4 h-4" /></button>
      <LogoMark class="w-6 h-6 shrink-0" />
      <span class="font-semibold">LocalAide</span>
    </div>
    {#if chatMounted && !startupError}
      <div class="min-h-0 flex-1" class:hidden={screen !== "chat"}>
        <ChatScreen onOpenModels={() => navigate("models")} active={screen === "chat"} />
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
        <SetupWizard onFinished={() => { setupBusy = false; navigate("chat"); }} onBusyChange={(busy) => setupBusy = busy} onSkip={() => navigate("chat")} />
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
      {:else if screen === "settings"}
        <SettingsScreen />
      {/if}
        </div>
      {/key}
    </div>
  </section>
  </div>

  <footer class="h-8 shrink-0 flex items-center gap-6 px-4 text-xs text-ink-400 bg-ink-950" aria-label="Status">
    {#if ramTotal > 0}
      <span>Computer memory: {formatBytes(ramUsable)} free of {formatBytes(ramTotal)}</span>
      <span>Models can use up to {formatBytes(budget)}</span>
    {/if}
    {#if share.status?.on}
      <button type="button" onclick={() => { if (navigate("settings")) requestSettings("share"); }} class="sharing-on" title={`Anyone with a link can reach this assistant: ${share.status.url ?? ""}`}>
        <span class="sharing-dot" aria-hidden="true"></span>Sharing is on · {share.status.requests} {share.status.requests === 1 ? "request" : "requests"}
      </button>
    {/if}
    <span class="ml-auto hidden sm:inline">Press ? for keyboard shortcuts</span>
  </footer>

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
    <div class="fixed bottom-12 right-4 z-50 w-72 rounded-xl border border-edge bg-ink-950 p-3 shadow-2xl" role="status" in:fly={{ y: 8, duration: dur(motion.base), easing: motion.enter }} out:fade={{ duration: dur(motion.fast) }}>
      <div class="flex items-center gap-2 text-sm font-medium text-ink-100"><Icon name="download" class="h-4 w-4 text-ember-300" />Getting the model engine</div>
      <p class="mt-1 text-xs leading-5 text-ink-400">Once only: the llama.cpp build for this processor, from github.com.</p>
      <div class="mt-2 h-1.5 overflow-hidden rounded-full bg-ink-800"><div class="h-full rounded-full bg-ember-500 transition-[width] duration-200" style={`width:${Math.max(3, engineDownload.percent)}%`}></div></div>
      <p class="mt-1 text-right text-xs tabular-nums text-ink-500">{formatBytes(engineDownload.downloaded_bytes)} of {formatBytes(engineDownload.total_bytes)}</p>
    </div>
  {/if}

  <!-- Always in the page, so screen readers hear each new message. -->
  <div class="fixed bottom-10 left-1/2 z-50 -translate-x-1/2" role={toast.tone === "error" ? "alert" : "status"}>
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
        <div class="space-y-2.5 text-xs font-mono">
          <div class="flex justify-between items-center"><span class="text-ink-400">Command palette</span><kbd class="px-2 py-0.5 rounded bg-ink-900 border border-edge text-ink-200">⌘K / Ctrl+K</kbd></div>
          <div class="flex justify-between items-center"><span class="text-ink-400">New conversation</span><kbd class="px-2 py-0.5 rounded bg-ink-900 border border-edge text-ink-200">⌘N / Ctrl+N</kbd></div>
          <div class="flex justify-between items-center"><span class="text-ink-400">Focus search</span><kbd class="px-2 py-0.5 rounded bg-ink-900 border border-edge text-ink-200">/</kbd></div>
          <div class="flex justify-between items-center"><span class="text-ink-400">Settings</span><kbd class="px-2 py-0.5 rounded bg-ink-900 border border-edge text-ink-200">⌘, / Ctrl+,</kbd></div>
          <div class="flex justify-between items-center"><span class="text-ink-400">Switch screens (1: Chat, 2: Search, ...)</span><kbd class="px-2 py-0.5 rounded bg-ink-900 border border-edge text-ink-200">⌘1 – ⌘6</kbd></div>
          <div class="flex justify-between items-center"><span class="text-ink-400">Cancel generation / Dismiss dialog</span><kbd class="px-2 py-0.5 rounded bg-ink-900 border border-edge text-ink-200">Esc</kbd></div>
          <div class="flex justify-between items-center"><span class="text-ink-400">Send message</span><kbd class="px-2 py-0.5 rounded bg-ink-900 border border-edge text-ink-200">Enter</kbd></div>
          <div class="flex justify-between items-center"><span class="text-ink-400">Choose the model (in Chat)</span><kbd class="px-2 py-0.5 rounded bg-ink-900 border border-edge text-ink-200">⌘⇧M / Ctrl+Shift+M</kbd></div>
          <div class="flex justify-between items-center"><span class="text-ink-400">Keyboard help</span><kbd class="px-2 py-0.5 rounded bg-ink-900 border border-edge text-ink-200">?</kbd></div>
        </div>
        <button onclick={() => showHelp = false} class="mt-6 w-full py-2 rounded-md bg-veil/5 border border-edge text-sm hover:bg-veil/10">Close</button>
        </FocusTrap>
      </div>
    </div>
  {/if}
  {#if legal.open}
    {@const doc = LEGAL_DOCS[legal.open]}
    <Modal title={doc.title} size="lg" onClose={() => legal.open = null}>
      <MarkdownDoc source={doc.source} />
    </Modal>
  {/if}
</main>

<style>
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
</style>
