<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "./lib/api/client";
  import { formatBytes } from "./lib/format";
  import SetupWizard from "./screens/SetupWizard.svelte";
  import ChatScreen from "./screens/ChatScreen.svelte";
  import MemoryScreen from "./screens/MemoryScreen.svelte";
  import ModelManagerScreen from "./screens/ModelManagerScreen.svelte";
  import SettingsScreen from "./screens/SettingsScreen.svelte";
  import DevConfigScreen from "./screens/DevConfigScreen.svelte";
  import SearchScreen from "./screens/SearchScreen.svelte";

  type Screen = "setup" | "chat" | "memory" | "models" | "settings" | "dev" | "search";

  let screen = $state<Screen>("setup");
  let ramTotal = $state(0);
  let ramUsable = $state(0);
  let budget = $state(0);
  let cmdOpen = $state(false);
  let cmdQuery = $state("");
  let toast = $state<string | null>(null);
  let mobileNav = $state(false);
  let showHelp = $state(false);
  let fatalError = $state<string | null>(null);

  const navigate = (s: Screen) => {
    screen = s;
  };

  const showToast = (msg: string) => {
    toast = msg;
    setTimeout(() => toast = null, 2500);
  };

  const screens = [
    { id: "chat", label: "Chat", emoji: "💬" },
    { id: "search", label: "Search", emoji: "🔍" },
    { id: "memory", label: "Memory", emoji: "🧠" },
    { id: "models", label: "Models", emoji: "🧩" },
    { id: "settings", label: "Settings", emoji: "⚙️" },
    { id: "dev", label: "Dev", emoji: "🧪" },
  ] as const;

  onMount(() => {
    void (async () => {
      try {
        const probe = await api.systemProbe();
        ramTotal = probe.total_ram_gb * 1024 * 1024 * 1024;
        ramUsable = probe.available_ram_gb * 1024 * 1024 * 1024;
        budget = probe.budget_gb * 1024 * 1024 * 1024;
      } catch {}
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
        showToast("New chat ready");
      } else if ((e.metaKey || e.ctrlKey) && e.key === ",") {
        e.preventDefault();
        navigate("settings");
      } else if ((e.metaKey || e.ctrlKey) && e.key === "1") {
        e.preventDefault();
        navigate("chat");
      } else if ((e.metaKey || e.ctrlKey) && e.key === "2") {
        e.preventDefault();
        navigate("search");
      } else if ((e.metaKey || e.ctrlKey) && e.key === "3") {
        e.preventDefault();
        navigate("memory");
      } else if ((e.metaKey || e.ctrlKey) && e.key === "4") {
        e.preventDefault();
        navigate("models");
      } else if ((e.metaKey || e.ctrlKey) && e.key === "5") {
        e.preventDefault();
        navigate("settings");
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

<main class="h-screen flex overflow-hidden bg-[#0a0a0b] text-zinc-100 selection:bg-fuchsia-500/30">
  <aside class={`fixed lg:relative inset-y-0 left-0 z-40 w-[280px] shrink-0 bg-[#0f0f12]/95 lg:bg-[#0f0f12]/80 backdrop-blur-xl border-r border-zinc-800 flex flex-col transition-transform duration-300 ${mobileNav ? 'translate-x-0' : '-translate-x-full lg:translate-x-0'}`}>
    <div class="h-[72px] border-b border-zinc-800 flex items-center gap-3 px-5 shrink-0">
      <button aria-label="Close navigation" onclick={() => mobileNav = false} class="lg:hidden ml-auto p-2 rounded-lg bg-white/5 border border-zinc-800">✕</button>
      <div class="size-8 rounded-lg bg-fuchsia-600 grid place-items-center">
        <svg class="w-4 h-4 text-black" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
        </svg>
      </div>
      <div>
        <span class="font-[700] tracking-tight text-[15px]">LocalAide</span>
        <div class="text-xs uppercase tracking-widest text-zinc-400">local-first</div>
      </div>
    </div>
    <nav class="flex-1 overflow-y-auto p-4 space-y-1">
      <button
        onclick={() => navigate("chat")}
        class={`group w-full flex items-center gap-3 px-3 py-2.5 rounded-xl text-sm transition ${screen === 'chat' ? 'bg-white/5 text-white' : 'text-zinc-400'}`}
      >
        <span class="size-8 grid place-items-center rounded-lg bg-white/5 group-hover:bg-white/10">💬</span>
        Chat
      </button>
      <button
        onclick={() => navigate("search")}
        class={`group w-full flex items-center gap-3 px-3 py-2.5 rounded-xl text-sm transition ${screen === 'search' ? 'bg-white/5 text-white' : 'text-zinc-400'}`}
      >
        <span class="size-8 grid place-items-center rounded-lg bg-white/5 group-hover:bg-white/10">🔍</span>
        Search
      </button>
      <button
        onclick={() => navigate("memory")}
        class={`group w-full flex items-center gap-3 px-3 py-2.5 rounded-xl text-sm transition ${screen === 'memory' ? 'bg-white/5 text-white' : 'text-zinc-400'}`}
      >
        <span class="size-8 grid place-items-center rounded-lg bg-white/5 group-hover:bg-white/10">🧠</span>
        Memory
      </button>
      <button
        onclick={() => navigate("models")}
        class={`group w-full flex items-center gap-3 px-3 py-2.5 rounded-xl text-sm transition ${screen === 'models' ? 'bg-white/5 text-white' : 'text-zinc-400'}`}
      >
        <span class="size-8 grid place-items-center rounded-lg bg-white/5 group-hover:bg-white/10">🧩</span>
        Models
      </button>
      <button
        onclick={() => navigate("settings")}
        class={`group w-full flex items-center gap-3 px-3 py-2.5 rounded-xl text-sm transition ${screen === 'settings' ? 'bg-white/5 text-white' : 'text-zinc-400'}`}
      >
        <span class="size-8 grid place-items-center rounded-lg bg-white/5 group-hover:bg-white/10">⚙️</span>
        Settings
      </button>
      {#if screen !== "setup"}
        <button
          onclick={() => navigate("dev")}
          class={`group w-full flex items-center gap-3 px-3 py-2.5 rounded-xl text-sm transition ${screen === 'dev' ? 'bg-white/5 text-white' : 'text-zinc-400'}`}
        >
          <span class="size-8 grid place-items-center rounded-lg bg-white/5 group-hover:bg-white/10">🧪</span>
          Dev
        </button>
      {/if}
    </nav>
    <div class="p-4 border-t border-zinc-800 shrink-0 space-y-2">
      <div class="text-xs uppercase tracking-widest text-zinc-400">RAM Budget</div>
      <div class="grid grid-cols-3 gap-2 text-[11px] font-mono">
        <div class="rounded-lg bg-white/5 p-2"><div class="text-zinc-400">total</div><div class="text-zinc-200">{formatBytes(ramTotal)}</div></div>
        <div class="rounded-lg bg-white/5 p-2"><div class="text-zinc-400">usable</div><div class="text-zinc-200">{formatBytes(ramUsable)}</div></div>
        <div class="rounded-lg bg-fuchsia-500/10 border border-fuchsia-500/30 p-2"><div class="text-fuchsia-300">budget</div><div class="text-fuchsia-200 font-semibold">{formatBytes(budget)}</div></div>
      </div>
    </div>
  </aside>

  <section class="flex-1 overflow-y-auto">
    <div class="lg:hidden sticky top-0 z-30 bg-[#0a0a0b]/80 backdrop-blur p-3 border-b border-zinc-800 flex items-center gap-3">
      <button aria-label="Open navigation" onclick={() => mobileNav = true} class="p-2 rounded-lg bg-white/5 border border-zinc-800">☰</button>
      <span class="font-semibold">LocalAide</span>
    </div>
    <div class="min-h-full p-6 md:p-10">
      {#if fatalError}
        <div class="max-w-xl mx-auto my-12 p-8 rounded-2xl border border-red-500/30 bg-red-950/20 text-center">
          <div class="text-3xl mb-3">⚠️</div>
          <h2 class="text-lg font-semibold text-red-200 mb-2">LocalAide recovered from an unexpected error</h2>
          <p class="text-sm text-zinc-400 mb-6 font-mono text-left bg-black/40 p-4 rounded-xl border border-zinc-800 break-words">{fatalError}</p>
          <div class="flex justify-center gap-3">
            <button onclick={() => fatalError = null} class="px-4 py-2 rounded-xl bg-white/10 hover:bg-white/20 text-sm font-medium">Dismiss and Continue</button>
            <button onclick={() => navigator.clipboard.writeText(fatalError ?? "")} class="px-4 py-2 rounded-xl border border-zinc-700 hover:bg-white/5 text-sm text-zinc-300">Copy Diagnostics</button>
          </div>
        </div>
      {:else if screen === "setup"}
        <SetupWizard />
      {:else if screen === "chat"}
        <ChatScreen />
      {:else if screen === "search"}
        <SearchScreen />
      {:else if screen === "memory"}
        <MemoryScreen />
      {:else if screen === "models"}
        <ModelManagerScreen />
      {:else if screen === "settings"}
        <SettingsScreen />
      {:else if screen === "dev"}
        <DevConfigScreen />
      {/if}
    </div>
  </section>

  {#if cmdOpen}
    <div role="presentation" class="fixed inset-0 bg-black/60 backdrop-blur-sm grid place-items-start pt-[20vh] px-4 z-50" onclick={() => cmdOpen = false} onkeydown={(e) => { if (e.key === 'Escape') cmdOpen = false; }}>
      <div role="dialog" tabindex="-1" aria-modal="true" aria-label="Command palette" class="w-full max-w-lg mx-auto rounded-2xl border border-zinc-800 bg-zinc-950 shadow-2xl overflow-hidden" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
        <div class="flex items-center gap-3 px-4 py-3 border-b border-zinc-800">
          <span>🔍</span>
          <input aria-label="Jump to screen" bind:value={cmdQuery} placeholder="Jump to…" class="flex-1 bg-transparent outline-none text-sm placeholder-zinc-600"/>
          <span class="text-xs px-1.5 py-0.5 rounded bg-zinc-800 text-zinc-400">ESC</span>
        </div>
        <div class="p-2 max-h-80 overflow-auto">
          {#each screens.filter(s => s.label.toLowerCase().includes(cmdQuery.toLowerCase())) as s}
            <button onclick={() => { navigate(s.id as Screen); cmdOpen = false; cmdQuery = ""; }} class="w-full flex items-center gap-3 px-3 py-2.5 rounded-xl hover:bg-white/5 text-left">
              <span class="size-8 grid place-items-center rounded-lg bg-white/5">{s.emoji}</span>
              <span class="text-sm">{s.label}</span>
            </button>
          {:else}
            <div class="px-3 py-6 text-center text-sm text-zinc-400">No matches</div>
          {/each}
        </div>
      </div>
    </div>
  {/if}

  {#if toast}
    <div class="fixed bottom-6 left-1/2 -translate-x-1/2 z-50">
      <div class="px-4 py-2 rounded-xl bg-zinc-900 border border-zinc-800 shadow-lg text-sm text-zinc-100">{toast}</div>
    </div>
  {/if}

  {#if showHelp}
    <div role="presentation" class="fixed inset-0 bg-black/70 backdrop-blur-sm grid place-items-center p-4 z-[60]" onclick={() => showHelp = false} onkeydown={(e) => { if (e.key === 'Escape') showHelp = false; }}>
      <div role="dialog" tabindex="-1" aria-modal="true" aria-label="Keyboard shortcuts" class="w-full max-w-md rounded-2xl border border-zinc-800 bg-zinc-950 p-6 shadow-2xl" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
        <h3 class="text-lg font-semibold mb-4 text-zinc-100">Keyboard shortcuts</h3>
        <div class="space-y-2.5 text-xs font-mono">
          <div class="flex justify-between items-center"><span class="text-zinc-400">Command palette</span><kbd class="px-2 py-0.5 rounded bg-zinc-900 border border-zinc-700 text-zinc-200">⌘K / Ctrl+K</kbd></div>
          <div class="flex justify-between items-center"><span class="text-zinc-400">New conversation</span><kbd class="px-2 py-0.5 rounded bg-zinc-900 border border-zinc-700 text-zinc-200">⌘N / Ctrl+N</kbd></div>
          <div class="flex justify-between items-center"><span class="text-zinc-400">Focus search</span><kbd class="px-2 py-0.5 rounded bg-zinc-900 border border-zinc-700 text-zinc-200">/</kbd></div>
          <div class="flex justify-between items-center"><span class="text-zinc-400">Settings</span><kbd class="px-2 py-0.5 rounded bg-zinc-900 border border-zinc-700 text-zinc-200">⌘, / Ctrl+,</kbd></div>
          <div class="flex justify-between items-center"><span class="text-zinc-400">Switch tabs (1: Chat, 2: Search, ...)</span><kbd class="px-2 py-0.5 rounded bg-zinc-900 border border-zinc-700 text-zinc-200">⌘1 – ⌘5</kbd></div>
          <div class="flex justify-between items-center"><span class="text-zinc-400">Cancel generation / Dismiss dialog</span><kbd class="px-2 py-0.5 rounded bg-zinc-900 border border-zinc-700 text-zinc-200">Esc</kbd></div>
          <div class="flex justify-between items-center"><span class="text-zinc-400">Send message</span><kbd class="px-2 py-0.5 rounded bg-zinc-900 border border-zinc-700 text-zinc-200">Enter</kbd></div>
          <div class="flex justify-between items-center"><span class="text-zinc-400">Keyboard help</span><kbd class="px-2 py-0.5 rounded bg-zinc-900 border border-zinc-700 text-zinc-200">?</kbd></div>
        </div>
        <button onclick={() => showHelp = false} class="mt-6 w-full py-2 rounded-xl bg-white/5 border border-zinc-800 text-sm hover:bg-white/10">Close</button>
      </div>
    </div>
  {/if}
</main>
