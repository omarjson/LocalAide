<script lang="ts">
  import { onMount } from "svelte";
  import { fade, fly, scale } from "svelte/transition";
  import { api } from "./lib/api/client";
  import { dur } from "./lib/motion";
  import { formatBytes } from "./lib/format";
  import SetupWizard from "./screens/SetupWizard.svelte";
  import ChatScreen from "./screens/ChatScreen.svelte";
  import FocusTrap from "./components/FocusTrap.svelte";
  import Icon from "./components/Icon.svelte";
  import LogoMark from "./components/LogoMark.svelte";
  import MemoryScreen from "./screens/MemoryScreen.svelte";
  import SkillsScreen from "./screens/SkillsScreen.svelte";
  import ModelManagerScreen from "./screens/ModelManagerScreen.svelte";
  import SettingsScreen from "./screens/SettingsScreen.svelte";
  import DevConfigScreen from "./screens/DevConfigScreen.svelte";
  import SearchScreen from "./screens/SearchScreen.svelte";

  type Screen = "setup" | "chat" | "memory" | "skills" | "models" | "settings" | "dev" | "search";
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
  let toast = $state<string | null>(null);
  let mobileNav = $state(false);
  let showHelp = $state(false);
  let fatalError = $state<string | null>(null);
  // Bumped by Ctrl/Cmd+N so ChatScreen starts a fresh thread even when it is
  // already on screen.
  let chatResetTick = $state(0);

  const navigate = (s: Screen) => {
    screen = s;
  };

  $effect(() => {
    if (cmdOpen) {
      cmdInput?.focus();
    }
  });

  const showToast = (msg: string) => {
    toast = msg;
    setTimeout(() => toast = null, 2500);
  };

  const screens = [
    { id: "chat", label: "Chat" },
    { id: "search", label: "Search" },
    { id: "memory", label: "Memory" },
    { id: "skills", label: "Skills" },
    { id: "models", label: "Models" },
    { id: "settings", label: "Settings" },
    { id: "dev", label: "Developer config" },
  ] as const;

  const shortcutScreens: NavId[] = ["chat", "search", "memory", "skills", "models", "settings"];

  onMount(() => {
    void (async () => {
      try {
        const theme = await api.getTheme();
        if (theme === "light" || theme === "dark") {
          document.documentElement.dataset.theme = theme;
        }
      } catch (e: unknown) {
        console.error("Theme could not load", e);
      }
      try {
        const cfg = await api.loadConfig();
        screen = cfg.selected_models?.length ? "chat" : "setup";
      } catch {
        screen = "setup";
      }
      try {
        const probe = await api.systemProbe();
        ramTotal = probe.total_ram_gb * 1024 * 1024 * 1024;
        ramUsable = probe.available_ram_gb * 1024 * 1024 * 1024;
        budget = probe.budget_gb * 1024 * 1024 * 1024;
      } catch (e: unknown) {
        console.error("System details could not load", e);
      }
    })();

    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const isInput = target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable);

      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        cmdOpen = !cmdOpen;
      } else if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "n") {
        e.preventDefault();
        navigate("chat");
        chatResetTick += 1;
        showToast("New chat ready");
      } else if ((e.metaKey || e.ctrlKey) && e.key === ",") {
        e.preventDefault();
        navigate("settings");
      } else if ((e.metaKey || e.ctrlKey) && /^[1-6]$/.test(e.key)) {
        e.preventDefault();
        const targetScreen = shortcutScreens[Number(e.key) - 1];
        if (targetScreen) navigate(targetScreen);
      } else if (e.key === "/" && !e.metaKey && !e.ctrlKey && !e.altKey && !isInput) {
        e.preventDefault();
        navigate("search");
      }

      if (e.key === "Escape") {
        cmdOpen = false;
        showHelp = false;
      }
      if (e.key === "?" && !e.metaKey && !e.ctrlKey && !e.altKey && !isInput) {
        e.preventDefault();
        showHelp = !showHelp;
      }
    };

    const onError = (e: ErrorEvent) => {
      fatalError = e.message || "An unhandled error occurred";
    };

    const onRejection = (e: PromiseRejectionEvent) => {
      fatalError = String(e.reason) || "An unhandled async failure occurred";
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
  <aside class={`fixed lg:relative inset-y-0 left-0 z-40 w-64 shrink-0 bg-ink-950 border-r border-edge lg:border-r-0 flex flex-col px-1.5 pb-1.5 transition-transform duration-300 ${mobileNav ? 'translate-x-0' : '-translate-x-full lg:translate-x-0'}`}>
    <div class="h-12 flex items-center gap-2.5 px-3 shrink-0">
      <LogoMark class="w-5 h-5 shrink-0" />
      <span class="text-sm font-semibold text-ink-50">LocalAide</span>
      <button aria-label="Close navigation" onclick={() => mobileNav = false} class="lg:hidden ml-auto p-2 rounded-md hover:bg-veil/5"><Icon name="close" class="w-4 h-4" /></button>
    </div>
    {#snippet navItem(id: NavId, label: string)}
      <button
        onclick={() => navigate(id)}
        aria-current={screen === id ? "page" : undefined}
        class={`relative w-full flex items-center gap-3.5 h-10 px-3 rounded-md text-sm text-left transition-colors ${screen === id ? 'bg-ink-800 text-ink-50 font-semibold' : 'text-ink-200 hover:bg-veil/5'}`}
      >
        {#if screen === id}<span class="absolute left-0 top-3 h-4 w-[3px] rounded-full bg-ember-500" aria-hidden="true"></span>{/if}
        <Icon name={id} class="w-[18px] h-[18px] shrink-0" />
        {label}
      </button>
    {/snippet}
    <nav class="flex-1 overflow-y-auto space-y-0.5" aria-label="Screens">
      {@render navItem("chat", "Chat")}
      {@render navItem("search", "Search")}
      {@render navItem("memory", "Memory")}
      {@render navItem("skills", "Skills")}
      {@render navItem("models", "Models")}
    </nav>
    <div class="shrink-0 space-y-1 pt-1.5 border-t border-ink-800">
      {@render navItem("settings", "Settings")}
    </div>
  </aside>

  <section class="flex-1 overflow-y-auto bg-ink-900 lg:border-l lg:border-t border-ink-700 lg:rounded-tl-lg">
    <div class="lg:hidden sticky top-0 z-30 bg-ink-950 p-3 border-b border-edge flex items-center gap-3">
      <button aria-label="Open navigation" onclick={() => mobileNav = true} class="p-2 rounded-md hover:bg-veil/5 text-ink-300"><Icon name="menu" class="w-4 h-4" /></button>
      <LogoMark class="w-6 h-6 shrink-0" />
      <span class="font-semibold">LocalAide</span>
    </div>
    <div class="min-h-full p-6 md:p-10">
      {#key screen}
        <div in:fade={{ duration: dur(120) }}>
      {#if fatalError}
        <div class="max-w-xl mx-auto my-12 p-8 rounded-2xl border border-rose-500/30 bg-rose-950/20 text-center">
          <div class="text-rose-300 mb-3 flex justify-center"><Icon name="alert" class="w-7 h-7" /></div>
          <h2 class="text-lg font-semibold text-rose-200 mb-2">LocalAide recovered from an unexpected error</h2>
          <p class="text-sm text-ink-400 mb-6 font-mono text-left bg-ink-950 p-4 rounded-xl border border-edge break-words">{fatalError}</p>
          <div class="flex justify-center gap-3">
            <button onclick={() => fatalError = null} class="px-4 py-2 rounded-xl bg-veil/10 hover:bg-veil/20 text-sm font-medium transition-colors">Dismiss and Continue</button>
            <button onclick={() => navigator.clipboard.writeText(fatalError ?? "")} class="px-4 py-2 rounded-xl border border-edge hover:bg-veil/5 text-sm text-ink-300 transition-colors">Copy Diagnostics</button>
          </div>
        </div>
      {:else if screen === null}
        <div class="max-w-2xl mx-auto my-24 flex flex-col items-center gap-3">
          <div class="w-10 h-10 rounded-full border-4 border-edge border-t-ember-500 animate-spin"></div>
          <p class="text-sm text-ink-400">Starting LocalAide…</p>
        </div>
      {:else if screen === "setup"}
        <SetupWizard onFinished={() => navigate("chat")} />
      {:else if screen === "chat"}
        <ChatScreen onOpenModels={() => navigate("models")} newThreadTick={chatResetTick} />
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
      {:else if screen === "dev"}
        <DevConfigScreen />
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
    <span class="ml-auto hidden sm:inline">Press ? for keyboard shortcuts</span>
  </footer>

  {#if cmdOpen}
    <div role="presentation" in:fade={{ duration: dur(100) }} out:fade={{ duration: dur(80) }} class="fixed inset-0 bg-black/60 backdrop-blur-sm grid place-items-start pt-[20vh] px-4 z-50" onclick={() => cmdOpen = false} onkeydown={(e) => { if (e.key === 'Escape') cmdOpen = false; }}>
      <div role="dialog" tabindex="-1" aria-modal="true" aria-label="Command palette" in:scale={{ start: 0.97, duration: dur(130) }} out:scale={{ start: 0.97, duration: dur(80) }} class="w-full max-w-lg mx-auto rounded-2xl border border-edge bg-ink-950 shadow-2xl overflow-hidden" onclick={(e) => e.stopPropagation()} onkeydown={(e) => { if (e.key === 'Escape') cmdOpen = false; e.stopPropagation(); }}>
        <FocusTrap>
        <div class="flex items-center gap-3 px-4 py-3 border-b border-edge">
          <span class="text-ink-400"><Icon name="search" /></span>
          <input aria-label="Jump to screen" bind:this={cmdInput} bind:value={cmdQuery} placeholder="Jump to…" class="flex-1 bg-transparent outline-none text-sm placeholder-ink-600"/>
          <span class="text-xs px-1.5 py-0.5 rounded bg-ink-800 text-ink-400">ESC</span>
        </div>
        <div class="p-2 max-h-80 overflow-auto">
          {#each screens.filter(s => s.label.toLowerCase().includes(cmdQuery.toLowerCase())) as s}
            <button onclick={() => { navigate(s.id as Screen); cmdOpen = false; cmdQuery = ""; }} class="w-full flex items-center gap-3 px-3 py-2.5 rounded-xl hover:bg-veil/5 text-left">
              <span class="size-8 grid place-items-center rounded-lg bg-veil/5"><Icon name={s.id} /></span>
              <span class="text-sm">{s.label}</span>
            </button>
          {:else}
            <div class="px-3 py-6 text-center text-sm text-ink-400">No matches</div>
          {/each}
        </div>
        </FocusTrap>
      </div>
    </div>
  {/if}

  {#if toast}
    <div class="fixed bottom-6 left-1/2 -translate-x-1/2 z-50" role="status" in:fly={{ y: 10, duration: dur(160) }} out:fade={{ duration: dur(100) }}>
      <div class="px-4 py-2 rounded-xl bg-ink-900 border border-edge shadow-lg text-sm text-ink-100">{toast}</div>
    </div>
  {/if}

  {#if showHelp}
    <div role="presentation" in:fade={{ duration: dur(100) }} out:fade={{ duration: dur(80) }} class="fixed inset-0 bg-black/70 backdrop-blur-sm grid place-items-center p-4 z-[60]" onclick={() => showHelp = false} onkeydown={(e) => { if (e.key === 'Escape') showHelp = false; }}>
      <div role="dialog" tabindex="-1" aria-modal="true" aria-label="Keyboard shortcuts" in:scale={{ start: 0.97, duration: dur(130) }} out:scale={{ start: 0.97, duration: dur(80) }} class="w-full max-w-md rounded-2xl border border-edge bg-ink-950 p-6 shadow-2xl" onclick={(e) => e.stopPropagation()} onkeydown={(e) => { if (e.key === 'Escape') showHelp = false; e.stopPropagation(); }}>
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
          <div class="flex justify-between items-center"><span class="text-ink-400">Keyboard help</span><kbd class="px-2 py-0.5 rounded bg-ink-900 border border-edge text-ink-200">?</kbd></div>
        </div>
        <button onclick={() => showHelp = false} class="mt-6 w-full py-2 rounded-xl bg-veil/5 border border-edge text-sm hover:bg-veil/10">Close</button>
        </FocusTrap>
      </div>
    </div>
  {/if}
</main>
