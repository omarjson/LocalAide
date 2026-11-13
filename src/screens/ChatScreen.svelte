<script lang="ts">
  import { onMount, tick } from "svelte";
  import { api } from "../lib/api/client";
  import type { ChatMessage, ChatThread } from "../lib/api/bindings";
  import MessageBubble from "../components/MessageBubble.svelte";
  import Icon from "../components/Icon.svelte";
  import ChatInput from "../components/ChatInput.svelte";

  let messages = $state<ChatMessage[]>([]);
  let pendingImages = $state<string[]>([]);
  const MAX_IMAGES = 4;

  // Read picked files as data URLs. The backend accepts PNG, JPEG, WEBP and GIF only.
  const addImages = async (files: FileList | null) => {
    if (!files) return;
    const room = MAX_IMAGES - pendingImages.length;
    for (const file of Array.from(files).slice(0, Math.max(room, 0))) {
      if (!file.type.startsWith("image/")) continue;
      const dataUrl = await new Promise<string>((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () => resolve(String(reader.result));
        reader.onerror = () => reject(reader.error);
        reader.readAsDataURL(file);
      });
      pendingImages.push(dataUrl);
    }
  };

  const removeImage = (index: number) => {
    pendingImages.splice(index, 1);
  };
  let threads = $state<ChatThread[]>([]);
  let activeThreadId = $state<string | null>(null);
  let activeThreadTitle = $state<string>("New Conversation");

  let input = $state("");
  let streaming = $state(false);
  let activeJobId = $state<string | null>(null);

  let model = $state("Local model");

  // Set when the opt-in online provider is active. Shows a cloud badge in the
  // header so it is always clear which engine is answering.
  let cloudChat = $state<{ label: string; model: string } | null>(null);

  // Shown in the empty state: which model is active, or none yet.
  let activeModelName = $state<string | null>(null);
  let modelsChecked = $state(false);

  let { onOpenModels }: { onOpenModels?: () => void } = $props();

  onMount(async () => {
    try {
      const models = await api.listModels();
      activeModelName = models.find((m) => m.active)?.name ?? null;
    } catch {
      activeModelName = null;
    } finally {
      modelsChecked = true;
    }
    try {
      const cloud = await api.getCloudConfig();
      const provider = cloud.providers.find((p) => p.id === cloud.active_provider_id);
      cloudChat = cloud.cloud_enabled && provider ? { label: provider.label, model: provider.model } : null;
    } catch {
      cloudChat = null;
    }
  });
  let systemPrompt = $state("You are a helpful local AI assistant.");
  let showSystem = $state(false);
  let showTools = $state(false);
  let showThreads = $state(false);
  let scrollEl = $state<HTMLDivElement>();
  let toolPath = $state("");
  let userScrolledUp = $state(false);

  const systemPresets = [
    { label: "General", prompt: "You are a helpful local AI assistant." },
    { label: "Coding", prompt: "You are an expert software engineer. Provide clean, secure, production-grade code with concise explanations." },
    { label: "Writing", prompt: "You are an eloquent editor and writer. Focus on clarity, tone, and conciseness." },
    { label: "Concise", prompt: "You are a concise analyst. Answer directly with bullet points. No conversational filler." },
  ];

  const starterPrompts = [
    "Explain Rust's borrow checker with a real-world analogy.",
    "Write a TypeScript helper to debounce async function calls.",
    "Draft a release checklist for a local desktop application.",
    "Summarize how retrieval-augmented generation works locally.",
  ];

  const handleScroll = () => {
    if (!scrollEl) return;
    const distanceToBottom = scrollEl.scrollHeight - scrollEl.scrollTop - scrollEl.clientHeight;
    userScrolledUp = distanceToBottom > 80;
  };

  const scrollToBottom = async (force = false) => {
    await tick();
    if (scrollEl && (!userScrolledUp || force)) {
      scrollEl.scrollTop = scrollEl.scrollHeight;
      if (force) userScrolledUp = false;
    }
  };

  $effect(() => {
    if (messages.length > 0) {
      scrollToBottom();
    }
  });

  const loadThreads = async () => {
    try {
      threads = await api.listChatThreads();
    } catch {
      threads = [];
    }
  };

  const newChat = () => {
    if (streaming) return;
    activeThreadId = `t_${Date.now()}`;
    activeThreadTitle = "New Conversation";
    messages = [];
  };

  const selectThread = async (t: ChatThread) => {
    if (streaming) return;
    activeThreadId = t.id;
    activeThreadTitle = t.title;
    systemPrompt = t.system_prompt || "You are a helpful local AI assistant.";
    try {
      messages = await api.loadChatThread(t.id);
    } catch {
      messages = [];
    }
    showThreads = false;
    scrollToBottom(true);
  };

  const deleteThread = async (id: string, e: MouseEvent) => {
    e.stopPropagation();
    try {
      await api.deleteChatThread(id);
      if (activeThreadId === id) {
        newChat();
      }
      await loadThreads();
    } catch {}
  };

  const persistCurrentThread = async () => {
    if (!activeThreadId) {
      activeThreadId = `t_${Date.now()}`;
    }
    if (messages.length === 0) return;

    if (activeThreadTitle === "New Conversation" && messages[0]) {
      const firstLine = messages[0].content.slice(0, 35);
      activeThreadTitle = firstLine.length < messages[0].content.length ? `${firstLine}…` : firstLine;
    }

    try {
      await api.saveChatThread(activeThreadId, activeThreadTitle, systemPrompt, $state.snapshot(messages));
      await loadThreads();
    } catch {}
  };

  const exportCurrentThread = async (format: "markdown" | "json") => {
    if (!activeThreadId) return;
    try {
      const res = await api.exportChatThread({ thread_id: activeThreadId, format });
      const blob = new Blob([res.content], { type: format === "json" ? "application/json" : "text/markdown" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = res.filename;
      a.click();
      URL.revokeObjectURL(url);
    } catch (e: any) {
      alert("Export failed: " + (e?.message ?? String(e)));
    }
  };

  const send = async () => {
    if ((!input.trim() && pendingImages.length === 0) || streaming) return;
    const userMsg = input.trim();
    input = "";

    // Earlier turns only. The new message is sent separately as `message`.
    const history = messages
      .filter(m => m.content.trim() && !m.content.startsWith("[Chat Error"))
      .map(m => ({ role: m.role, content: m.content }));

    const images = $state.snapshot(pendingImages);
    pendingImages = [];
    messages.push({ role: "user", content: userMsg, ts: Date.now(), images });
    streaming = true;
    userScrolledUp = false;

    const assistantIdx = messages.length;
    messages.push({ role: "assistant", content: "", ts: Date.now() });
    let acc = "";

    try {
      activeJobId = await api.streamChat(
        {
          message: userMsg,
          history,
          images,
          model,
          system_prompt: systemPrompt,
          temperature: 0.7,
          max_tokens: 512,
        },
        (event) => {
          if (event.type === "token") {
            acc += event.payload.token;
            messages[assistantIdx].content = acc;
          } else if (event.type === "citation") {
            const cur = messages[assistantIdx].citations || [];
            messages[assistantIdx].citations = [...cur, event.payload];
          } else if (event.type === "done") {
            streaming = false;
            activeJobId = null;
            persistCurrentThread();
          } else if (event.type === "error") {
            messages[assistantIdx].content = `[Chat Error: ${event.payload.error}]`;
            streaming = false;
            activeJobId = null;
            persistCurrentThread();
          }
        }
      );
    } catch (e: any) {
      messages[assistantIdx].content = `[Chat Error: ${e?.message ?? String(e)}]`;
      streaming = false;
      activeJobId = null;
      persistCurrentThread();
    }
  };

  const stopGenerating = async () => {
    if (!streaming) return;
    if (activeJobId) {
      try {
        await api.cancelChat(activeJobId);
      } catch {}
    }
    streaming = false;
    activeJobId = null;
    persistCurrentThread();
  };

  const retryLast = () => {
    if (streaming || messages.length < 2) return;
    const lastUser = [...messages].reverse().find(m => m.role === "user");
    if (!lastUser) return;
    // Pop off last assistant turn
    if (messages[messages.length - 1]?.role === "assistant") {
      messages.pop();
    }
    input = lastUser.content;
    send();
  };

  const editTurn = (newText: string, index: number) => {
    if (streaming) return;
    messages = messages.slice(0, index);
    input = newText;
    send();
  };

  const summarizeLast = async () => {
    const last = messages.filter(m => m.role === "assistant").at(-1);
    if (!last || streaming) return;
    streaming = true;
    const summaryIdx = messages.length;
    messages.push({ role: "assistant", content: "Summarizing…", ts: Date.now() });
    try {
      const res = await api.summarize({ text: last.content });
      messages[summaryIdx].content = res.summary;
      persistCurrentThread();
    } catch (e: any) {
      messages[summaryIdx].content = `[Summarize error: ${e?.message ?? String(e)}]`;
    } finally {
      streaming = false;
    }
  };

  const readFile = async () => {
    if (!toolPath.trim()) return;
    messages.push({ role: "user", content: `Read file: ${toolPath}`, ts: Date.now() });
    try {
      const res = await api.executeTool({
        tool_name: "fs_tool",
        action: {
          action_type: "read_file",
          path: toolPath,
        },
        approved: true,
      });
      const out = res.success ? (res.truncated ? `${res.output.slice(0, 4000)}\n... [truncated at 1MB]` : res.output.slice(0, 4000)) : `Error: ${res.error}`;
      messages.push({ role: "assistant", content: `File contents:\n\`\`\`\n${out}\n\`\`\``, ts: Date.now() });
      persistCurrentThread();
    } catch (e: any) {
      messages.push({ role: "assistant", content: `[tool error: ${e?.message ?? String(e)}]`, ts: Date.now() });
    }
    toolPath = "";
  };

  onMount(() => {
    activeThreadId = `t_${Date.now()}`;
    loadThreads();
  });
</script>

<div class="max-w-4xl mx-auto h-full flex flex-col relative">
  <!-- Header Bar -->
  <div class="flex items-center justify-between px-2 py-3 border-b border-ink-800">
    <div class="flex items-center gap-3">
      <button
        onclick={() => showThreads = !showThreads}
        class="flex items-center gap-2 px-2.5 py-1 text-xs rounded-lg border border-ink-800 hover:bg-ink-800/60 text-ink-300 font-medium transition-colors"
      >
        <span class="text-ink-400 inline-flex"><Icon name="chat" class="w-4 h-4" /></span>
        <span class="max-w-[140px] truncate">{activeThreadTitle}</span>
        <span class="text-xs text-ink-400 font-mono">({threads.length})</span>
      </button>

      <div class="hidden sm:flex items-center gap-2">
        {#if cloudChat}
          <span class="text-xs font-mono" title="Chat is running through your online provider. Messages are sent to {cloudChat.label}.">☁️</span>
          <span class="text-xs font-mono text-amber-300" title="Chat is running through your online provider. Messages are sent to {cloudChat.label}.">Cloud: {cloudChat.model} · {cloudChat.label}</span>
        {:else}
          <div class="w-2 h-2 rounded-full bg-emerald-500 animate-pulse"></div>
          <span class="text-xs font-mono text-ink-400">{model}</span>
        {/if}
      </div>
    </div>

    <div class="flex items-center gap-2">
      <button
        onclick={newChat}
        class="px-2.5 py-1 text-xs rounded-lg bg-ink-800 hover:bg-ink-700 text-ink-200 font-medium transition-colors"
      >
        + New chat
      </button>
      <button
        onclick={() => showSystem = !showSystem}
        class="px-2.5 py-1 text-xs rounded-lg border border-ink-800 hover:bg-ink-800/60 text-ink-300 transition-colors"
      >
        Persona
      </button>
      <button
        onclick={() => exportCurrentThread("markdown")}
        disabled={messages.length === 0}
        class="px-2.5 py-1 text-xs rounded-lg border border-ink-800 hover:bg-ink-800/60 disabled:opacity-30 text-ink-400 transition-colors"
        title="Export conversation as Markdown"
      >
        Export
      </button>
      <button
        onclick={summarizeLast}
        disabled={streaming || messages.length < 2}
        class="px-2.5 py-1 text-xs rounded-lg bg-ink-800 hover:bg-ink-700 disabled:opacity-40 text-ink-300 transition-colors"
      >
        Summarize
      </button>
    </div>
  </div>

  <!-- Threads Drawer -->
  {#if showThreads}
    <div class="absolute top-14 left-2 z-20 w-72 bg-ink-900 border border-ink-800 rounded-2xl shadow-2xl p-3 space-y-2 max-h-96 overflow-y-auto">
      <div class="flex items-center justify-between pb-2 border-b border-ink-800 text-xs font-semibold text-ink-300">
        <span>Saved Conversations</span>
        <button onclick={newChat} class="text-[11px] text-ember-400 hover:underline">+ New</button>
      </div>
      {#if threads.length === 0}
        <div class="text-[11px] text-ink-400 py-3 text-center">No saved chats yet.</div>
      {:else}
        {#each threads as t}
          <div
            role="button"
            tabindex="0"
            onclick={() => selectThread(t)}
            onkeydown={(e) => { if (e.key === 'Enter') selectThread(t); }}
            class={`w-full text-left p-2 rounded-xl text-xs flex items-center justify-between group transition-colors cursor-pointer ${t.id === activeThreadId ? 'bg-ember-500/10 border border-ember-500/30 text-ink-100' : 'hover:bg-ink-800/50 text-ink-300'}`}
          >
            <div class="truncate pr-2">
              <div class="font-medium truncate">{t.title}</div>
              <div class="text-xs text-ink-400 font-mono">{t.message_count} messages</div>
            </div>
            <button
              onclick={(e) => deleteThread(t.id, e)}
              class="opacity-0 group-hover:opacity-100 p-1 text-ink-400 hover:text-rose-400 transition-opacity"
              title="Delete thread"
            >
              ×
            </button>
          </div>
        {/each}
      {/if}
    </div>
  {/if}

  <!-- Persona / System Prompt Selector -->
  {#if showSystem}
    <div class="p-3 bg-ink-900/60 border-b border-ink-800 text-xs space-y-2">
      <div class="flex items-center gap-2">
        <span class="text-ink-400">Presets:</span>
        {#each systemPresets as p}
          <button
            onclick={() => systemPrompt = p.prompt}
            class={`px-2 py-0.5 rounded font-mono text-[11px] transition-colors ${systemPrompt === p.prompt ? 'bg-ember-600 text-ink-950 font-medium' : 'bg-ink-800 text-ink-300 hover:bg-ink-700'}`}
          >
            {p.label}
          </button>
        {/each}
      </div>
      <textarea
        bind:value={systemPrompt}
        rows={2}
        class="w-full bg-ink-950 border border-ink-800 rounded-lg p-2 text-ink-200 font-mono text-xs focus:outline-none focus:border-ink-600"
      ></textarea>
    </div>
  {/if}

  <!-- Message Stream Container -->
  <div
    bind:this={scrollEl}
    onscroll={handleScroll}
    role="region"
    aria-live="polite"
    aria-label="Conversation message stream"
    class="flex-1 overflow-y-auto p-4 space-y-2 relative"
  >
    {#if messages.length === 0}
      <!-- Empty state: says what to do next, based on whether a model is active -->
      <div class="h-full flex flex-col justify-center items-center text-center p-6 space-y-6">
        <div class="space-y-2 max-w-sm">
          <img src="/logo.svg" alt="" aria-hidden="true" class="w-12 h-12 mx-auto" />
          {#if !modelsChecked}
            <h3 class="text-sm font-medium text-ink-200">Checking for a model…</h3>
          {:else if !activeModelName}
            <h3 class="text-sm font-medium text-ink-200">Pick a model to start chatting.</h3>
            <p class="text-xs text-ink-400">Models run on this computer. Download one in Models, then come back here.</p>
            {#if onOpenModels}
              <button
                type="button"
                onclick={onOpenModels}
                class="mt-2 px-4 py-2 bg-ember-600 hover:bg-ember-500 rounded-lg text-xs font-medium text-ink-950"
              >
                Open Models
              </button>
            {/if}
          {:else}
            <h3 class="text-sm font-medium text-ink-200">Ask about your notes, files, or an image.</h3>
            <p class="text-xs text-ink-400">Running {activeModelName}. Nothing leaves this computer.</p>
          {/if}
        </div>

        {#if activeModelName}
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-2.5 max-w-md w-full">
          {#each starterPrompts as sp}
            <button
              onclick={() => { input = sp; send(); }}
              class="text-left p-3 rounded-xl border border-ink-800/80 bg-ink-900/40 hover:bg-ink-900 hover:border-ink-700 text-xs text-ink-300 transition-all"
            >
              "{sp}"
            </button>
          {/each}
        </div>
        {/if}
      </div>
    {:else}
      {#each messages as msg, idx}
        <MessageBubble
          role={msg.role}
          content={msg.content}
          images={msg.images}
          ts={msg.ts}
          streaming={streaming && idx === messages.length - 1}
          citations={msg.citations}
          onRetry={msg.role === "assistant" && idx === messages.length - 1 ? retryLast : undefined}
          onEdit={msg.role === "user" ? (newText) => editTurn(newText, idx) : undefined}
        />
      {/each}
    {/if}

    <!-- Unpinned Scroll Button -->
    {#if userScrolledUp}
      <button
        onclick={() => scrollToBottom(true)}
        class="fixed bottom-28 right-8 z-10 px-3 py-1.5 rounded-full bg-ink-800/90 border border-ink-700 text-xs text-ink-200 shadow-xl hover:bg-ink-700 transition-all flex items-center gap-1.5"
      >
        <span>↓</span>
        <span>Scroll to bottom</span>
      </button>
    {/if}
  </div>

  <!-- Input Field -->
  <ChatInput
    bind:input={input}
    bind:toolPath={toolPath}
    bind:showTools={showTools}
    streaming={streaming}
    onSend={send}
    onCancel={stopGenerating}
    onReadFile={readFile}
    pendingImages={pendingImages}
    onPickImages={addImages}
    onRemoveImage={removeImage}
  />
</div>
