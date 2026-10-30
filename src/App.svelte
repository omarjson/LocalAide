<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
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

  onMount(async () => {
    try {
      const probe = await invoke("system_probe") as any;
      ramTotal = probe.total_ram;
      ramUsable = probe.usable_ram;
      budget = probe.budget_ram;
    } catch {}
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        cmdOpen = !cmdOpen;
      }
      if (e.key === "Escape") { cmdOpen = false; showHelp = false; }
      if (e.key === "?" && !e.metaKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        showHelp = !showHelp;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<main class="h-screen flex overflow-hidden bg-[#0a0a0b] text-zinc-100 selection:bg-fuchsia-500/30">
  <aside class={`fixed lg:relative inset-y-0 left-0 z-40 w-[280px] shrink-0 bg-[#0f0f12]/95 lg:bg-[#0f0f12]/80 backdrop-blur-xl border-r border-zinc-800 flex flex-col transition-transform duration-300 ${mobileNav ? 'translate-x-0' : '-translate-x-full lg:translate-x-0'}`}>
    <div class="h-[72px] border-b border-zinc-800 flex items-center gap-3 px-5 shrink-0">
      <button onclick={() => mobileNav = false} class="lg:hidden ml-auto p-2 rounded-lg bg-white/5 border border-zinc-800">✕</button>
      <div class="size-8 rounded-lg bg-gradient-to-br from-fuchsia-500 to-cyan-400 grid place-items-center shadow-[0_0_30px_rgba(217,70,239,0.35)]">
        <svg class="w-4 h-4 text-black" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 10V3L4 14h7v7l9-11h-7z" />
        </svg>
      </div>
      <div>
        <span class="font-[700] tracking-tight text-[15px]">ai-assistant</span>
        <div class="text-[10px] uppercase tracking-widest text-zinc-500">local-first</div>
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
      <div class="text-[10px] uppercase tracking-widest text-zinc-500">RAM</div>
      <div class="grid grid-cols-3 gap-2 text-[11px] font-mono">
        <div class="rounded-lg bg-white/5 p-2"><div class="text-zinc-500">total</div><div class="text-zinc-200">{formatBytes(ramTotal)}</div></div>
        <div class="rounded-lg bg-white/5 p-2"><div class="text-zinc-500">usable</div><div class="text-zinc-200">{formatBytes(ramUsable)}</div></div>
        <div class="rounded-lg bg-fuchsia-500/10 border border-fuchsia-500/30 p-2"><div class="text-fuchsia-300">budget</div><div class="text-fuchsia-200 font-semibold">{formatBytes(budget)}</div></div>
      </div>
    </div>
  </aside>

  <section class="flex-1 overflow-y-auto">
    <div class="lg:hidden sticky top-0 z-30 bg-[#0a0a0b]/80 backdrop-blur p-3 border-b border-zinc-800 flex items-center gap-3">
      <button onclick={() => mobileNav = true} class="p-2 rounded-lg bg-white/5 border border-zinc-800">☰</button>
      <span class="font-semibold">ai-assistant</span>
    </div>
    <div class="min-h-full p-6 md:p-10">
      {#if screen === "setup"}
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
    <div class="fixed inset-0 bg-black/60 backdrop-blur-sm grid place-items-start pt-[20vh] px-4 z-50" onclick={() => cmdOpen = false}>
      <div class="w-full max-w-lg mx-auto rounded-2xl border border-zinc-800 bg-zinc-950 shadow-2xl overflow-hidden" onclick={(e) => e.stopPropagation()}>
        <div class="flex items-center gap-3 px-4 py-3 border-b border-zinc-800">
          <span>🔍</span>
          <input bind:value={cmdQuery} autofocus placeholder="Jump to…" class="flex-1 bg-transparent outline-none text-sm placeholder-zinc-600"/>
          <span class="text-[10px] px-1.5 py-0.5 rounded bg-zinc-800 text-zinc-400">ESC</span>
        </div>
        <div class="p-2 max-h-80 overflow-auto">
          {#each screens.filter(s => s.label.toLowerCase().includes(cmdQuery.toLowerCase())) as s}
            <button onclick={() => { navigate(s.id as Screen); cmdOpen = false; cmdQuery = ""; }} class="w-full flex items-center gap-3 px-3 py-2.5 rounded-xl hover:bg-white/5 text-left">
              <span class="size-8 grid place-items-center rounded-lg bg-white/5">{s.emoji}</span>
              <span class="text-sm">{s.label}</span>
            </button>
          {:else}
            <div class="px-3 py-6 text-center text-sm text-zinc-500">No matches</div>
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
    <div class="fixed inset-0 bg-black/70 backdrop-blur-sm grid place-items-center p-4 z-[60]" onclick={() => showHelp = false}>
      <div class="w-full max-w-md rounded-2xl border border-zinc-800 bg-zinc-950 p-6 shadow-2xl" onclick={(e) => e.stopPropagation()}>
        <h3 class="text-lg font-semibold mb-4">Keyboard shortcuts</h3>
        <div class="space-y-3 text-sm">
          <div class="flex justify-between"><span class="text-zinc-400">Command palette</span><kbd class="px-2 py-0.5 rounded bg-zinc-800 border border-zinc-700 text-xs">⌘K</kbd></div>
          <div class="flex justify-between"><span class="text-zinc-400">Help</span><kbd class="px-2 py-0.5 rounded bg-zinc-800 border border-zinc-700 text-xs">?</kbd></div>
          <div class="flex justify-between"><span class="text-zinc-400">Close dialog</span><kbd class="px-2 py-0.5 rounded bg-zinc-800 border border-zinc-700 text-xs">Esc</kbd></div>
          <div class="flex justify-between"><span class="text-zinc-400">Send message</span><kbd class="px-2 py-0.5 rounded bg-zinc-800 border border-zinc-700 text-xs">Enter</kbd></div>
        </div>
        <button onclick={() => showHelp = false} class="mt-6 w-full py-2 rounded-xl bg-white/5 border border-zinc-800 text-sm hover:bg-white/10">Close</button>
      </div>
    </div>
  {/if}
</main>

<script lang="ts" context="module">
  // Svelte 5 module context reserved for shared state if needed later
</script>