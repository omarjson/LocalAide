<script lang="ts">
  import { onDestroy, onMount, tick, untrack } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { listen } from "@tauri-apps/api/event";
  import { api, errorMessage } from "../lib/api/client";
  import { popover } from "../lib/actions/popover";
  import { dur, stagger } from "../lib/motion";
  import { showToast } from "../lib/toast.svelte";
  import Button from "../components/Button.svelte";
  import ConfirmDialog from "../components/ConfirmDialog.svelte";
  import ErrorBanner from "../components/ErrorBanner.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import ToolApprovalModal from "../components/ToolApprovalModal.svelte";
  import type { ChatMessage, ChatThread, EngineStatusEvent, Skill, ToolAction, ToolExecutionResponse } from "../lib/api/bindings";
  import MessageBubble from "../components/MessageBubble.svelte";
  import Icon from "../components/Icon.svelte";
  import LogoMark from "../components/LogoMark.svelte";
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
  // Installed models for the header picker; switching loads the chosen one
  // through the same path the Models screen uses.
  let installedModels = $state<{ id: string; name: string }[]>([]);
  let pickerOpen = $state(false);
  let switchingModel = $state(false);
  let switchError = $state<string | null>(null);
  // Whether a local model is loaded in the engine right now.
  let engineLoaded = $state(false);
  let chatError = $state<string | null>(null);
  // True when chatError came from a failed answer, so Retry resends it.
  let sendFailed = $state(false);
  // Read by screen readers once per answer, instead of every streamed token.
  let liveStatus = $state("");
  let showMoreActions = $state(false);
  let pendingDeleteThreadId = $state<string | null>(null);
  let modelsChecked = $state(false);

  let { onOpenModels, newThreadTick = 0 }: { onOpenModels?: () => void; newThreadTick?: number } = $props();

  // The saved model loads in the background at startup. Show progress, and show a
  // failed load in the chat so the user knows why nothing answers.
  let engineLoading = $state(false);
  onMount(() => {
    let unlisten: (() => void) | undefined;
    let closed = false;
    void (async () => {
      try {
        const status = await api.getInferenceStatus();
        engineLoading = status.loading;
        engineLoaded = status.loaded;
        const stop = await listen<EngineStatusEvent>("engine-status", (event) => {
          const { state, message } = event.payload;
          engineLoading = state === "loading";
          if (state === "ready") engineLoaded = true;
          if (state === "failed") {
            engineLoaded = false;
            sendFailed = false;
            chatError = message ?? "The model did not load. Try again from Models.";
          }
        });
        if (closed) {
          stop();
        } else {
          unlisten = stop;
        }
      } catch (e: unknown) {
        console.error("Engine status could not load", e);
      }
    })();
    return () => {
      closed = true;
      unlisten?.();
    };
  });

  onMount(async () => {
    try {
      const models = await api.listModels();
      activeModelName = models.find((m) => m.active)?.name ?? null;
      installedModels = models
        .filter((m) => m.installed)
        .map((m) => ({ id: m.id, name: m.name }));
    } catch (e: unknown) {
      activeModelName = null;
      chatError = errorMessage(e);
    } finally {
      modelsChecked = true;
    }
    try {
      const cloud = await api.getCloudConfig();
      const provider = cloud.providers.find((p) => p.id === cloud.active_provider_id);
      cloudChat = cloud.cloud_enabled && provider ? { label: provider.label, model: provider.model } : null;
    } catch (e: unknown) {
      cloudChat = null;
      chatError = `Online settings could not load: ${errorMessage(e)}`;
    }
    // Restore the chat preferences from the last session (honest local
    // adaptation: what the user chose, not a profile).
    try {
      const prefs = await api.getChatPrefs();
      if (prefs.system_prompt) systemPrompt = prefs.system_prompt;
      if (prefs.temperature !== null) temperature = prefs.temperature;
      if (prefs.max_tokens !== null) maxTokens = prefs.max_tokens;
      if (prefs.persona) persona = prefs.persona;
    } catch (e: unknown) {
      chatError = `Chat settings could not load: ${errorMessage(e)}`;
    }
    // Skills for the "/" slash menu in the input box.
    try {
      skills = await api.skillsList();
    } catch (e: unknown) {
      chatError = `Saved skills could not load: ${errorMessage(e)}`;
    }
  });

  // Persist the chosen chat preferences for the next session. The backend
  // also counts custom-prompt reuse for the save-as-skill suggestions.
  const savePrefs = async () => {
    try {
      await api.setChatPrefs({
        system_prompt: systemPrompt,
        temperature,
        max_tokens: maxTokens,
        persona,
      });
    } catch (e: unknown) {
      chatError = `Chat settings could not save: ${errorMessage(e)}`;
    }
  };
  let systemPrompt = $state("You are a helpful local AI assistant.");
  let temperature = $state(0.7);
  let maxTokens = $state(512);
  let persona = $state("General");
  let skills = $state<Skill[]>([]);
  let showSystem = $state(false);
  let showTools = $state(false);
  let showThreads = $state(false);
  let scrollEl = $state<HTMLDivElement>();
  let toolPath = $state("");
  let toolContent = $state("");
  // A pending write_file waiting for the user's approval decision, and a
  // stable token the backend's ApprovalManager remembers for this chat visit.
  let pendingTool = $state<{ tool_name: string; action: ToolAction } | null>(null);
  let showApproval = $state(false);
  const sessionToken = `sess_${Date.now().toString(36)}`;
  let userScrolledUp = $state(false);

  const systemPresets = [
    { label: "General", prompt: "You are a helpful local AI assistant." },
    { label: "Coding", prompt: "You are an expert software engineer. Provide clean, secure, production-grade code with concise explanations." },
    { label: "Writing", prompt: "You are an eloquent editor and writer. Focus on clarity, tone, and conciseness." },
    { label: "Concise", prompt: "You are a concise analyst. Answer directly with bullet points. No conversational filler." },
  ];

  const starterPrompts: { title: string; prompt: string; icon: "clock" | "chat" | "file" | "sliders" }[] = [
    { title: "Plan my week", prompt: "Help me plan my week.", icon: "clock" },
    { title: "Write an email", prompt: "Draft a polite email about a late payment.", icon: "chat" },
    { title: "Summarize", prompt: "Summarize this note in five short points.", icon: "file" },
    { title: "Compare options", prompt: "Compare these two choices and explain the trade-offs.", icon: "sliders" },
  ];

  const greeting = (() => {
    const hour = new Date().getHours();
    if (hour < 5) return "Working late?";
    if (hour < 12) return "Good morning.";
    if (hour < 18) return "Good afternoon.";
    return "Good evening.";
  })();

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

  // Follow the answer while it streams: the last message grows without the
  // message count changing, so its length is tracked too.
  $effect(() => {
    const lastLength = messages.at(-1)?.content.length ?? 0;
    if (messages.length > 0 && lastLength >= 0) {
      scrollToBottom();
    }
  });

  const loadThreads = async () => {
    try {
      threads = await api.listChatThreads();
    } catch (e: unknown) {
      threads = [];
      chatError = `Saved chats could not load: ${errorMessage(e)}`;
    }
  };

  const newChat = () => {
    if (streaming) return;
    activeThreadId = `t_${Date.now()}`;
    activeThreadTitle = "New Conversation";
    messages = [];
  };

  // Ctrl/Cmd+N bumps the tick in App.svelte. Only react to actual changes:
  // on a remount (navigating back to chat) the tick already equals its initial
  // value, and the fresh component is a new conversation anyway. The seen-tick
  // marker is deliberately non-reactive; untrack captures the initial value.
  let lastThreadTick = untrack(() => newThreadTick);
  $effect(() => {
    if (newThreadTick !== lastThreadTick) {
      lastThreadTick = newThreadTick;
      newChat();
    }
  });

  const selectThread = async (t: ChatThread) => {
    if (streaming) return;
    activeThreadId = t.id;
    activeThreadTitle = t.title;
    systemPrompt = t.system_prompt || "You are a helpful local AI assistant.";
    try {
      messages = await api.loadChatThread(t.id);
    } catch (e: unknown) {
      messages = [];
      chatError = `That chat could not load: ${errorMessage(e)}`;
    }
    showThreads = false;
    scrollToBottom(true);
  };

  const deleteThread = async (id: string) => {
    try {
      await api.deleteChatThread(id);
      if (activeThreadId === id) newChat();
      await loadThreads();
    } catch (e: unknown) {
      chatError = `That chat could not be deleted: ${errorMessage(e)}`;
    }
  };

  const askDeleteThread = (id: string) => {
    pendingDeleteThreadId = id;
  };

  const confirmDeleteThread = async () => {
    const id = pendingDeleteThreadId;
    pendingDeleteThreadId = null;
    if (id) await deleteThread(id);
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

    // Saved chats keep text only. Images would be sent and then dropped.
    const textOnly = $state.snapshot(messages).map(({ images: _images, ...rest }) => rest);
    try {
      await api.saveChatThread(activeThreadId, activeThreadTitle, systemPrompt, textOnly);
      await loadThreads();
    } catch (e: unknown) {
      sendFailed = false;
      chatError = `The chat could not be saved: ${errorMessage(e)}`;
    }
  };

  const exportCurrentThread = async (format: "markdown" | "json") => {
    if (!activeThreadId) return;
    try {
      const savedTo = await api.saveChatExport({ thread_id: activeThreadId, format });
      if (savedTo) showToast(`Chat saved to ${savedTo}`);
    } catch (e: unknown) {
      sendFailed = false;
      chatError = `The chat could not be saved as a file: ${errorMessage(e)}`;
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
    sendFailed = false;
    liveStatus = "LocalAide is answering.";
    userScrolledUp = false;
    void savePrefs();

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
          temperature,
          max_tokens: maxTokens,
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
            liveStatus = "Answer ready.";
            persistCurrentThread();
          } else if (event.type === "error") {
            messages[assistantIdx].content = "I could not answer that. Try again.";
            chatError = errorMessage(event.payload.error);
            sendFailed = true;
            streaming = false;
            activeJobId = null;
            liveStatus = "LocalAide could not answer.";
            persistCurrentThread();
          }
        }
      );
    } catch (e: unknown) {
      messages[assistantIdx].content = "I could not answer that. Try again.";
      chatError = errorMessage(e);
      sendFailed = true;
      streaming = false;
      activeJobId = null;
      liveStatus = "LocalAide could not answer.";
      persistCurrentThread();
    }
  };

  const stopGenerating = async () => {
    if (!streaming) return;
    if (activeJobId) {
      try {
        await api.cancelChat(activeJobId);
      } catch (e: unknown) {
        chatError = `The response could not stop: ${errorMessage(e)}`;
      }
    }
    streaming = false;
    activeJobId = null;
    liveStatus = "Answer stopped.";
    persistCurrentThread();
  };

  // Ask the last question again. The old question and answer are removed
  // first, so the question does not appear twice.
  const retryLast = () => {
    if (streaming) return;
    let userIdx = messages.length - 1;
    while (userIdx >= 0 && messages[userIdx].role !== "user") userIdx -= 1;
    if (userIdx < 0) return;
    const lastUser = messages[userIdx];
    chatError = null;
    sendFailed = false;
    messages = messages.slice(0, userIdx);
    input = lastUser.content;
    pendingImages = [...(lastUser.images ?? [])];
    void send();
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
    } catch (e: unknown) {
      messages[summaryIdx].content = "I could not make a summary. Try again.";
      chatError = `The summary could not be made: ${errorMessage(e)}`;
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
    } catch (e: unknown) {
      chatError = `The file action failed: ${errorMessage(e)}`;
      messages.push({ role: "assistant", content: "The file action failed. See the message above and try again.", ts: Date.now() });
    }
    toolPath = "";
  };

  const writeFile = async () => {
    if (!toolPath.trim()) return;
    const action: ToolAction = {
      action_type: "write_file",
      path: toolPath,
      content: toolContent,
    };
    messages.push({ role: "user", content: `Write file: ${action.path}`, ts: Date.now() });
    toolPath = "";
    toolContent = "";
    try {
      const res = await api.executeTool({ tool_name: "fs_tool", action });
      if (res.requires_approval) {
        pendingTool = { tool_name: "fs_tool", action };
        showApproval = true;
        return;
      }
      pushToolResponse(res, "executed");
    } catch (e: unknown) {
      chatError = `The file action failed: ${errorMessage(e)}`;
      messages.push({ role: "assistant", content: "The file action failed. See the message above and try again.", ts: Date.now() });
    }
  };

  const runApprovedTool = async (approved: boolean, token?: string) => {
    const pending = pendingTool;
    showApproval = false;
    pendingTool = null;
    if (!pending) return;
    try {
      const res = await api.executeTool({
        tool_name: pending.tool_name,
        action: pending.action,
        approved,
        session_token: token,
      });
      pushToolResponse(res, approved ? "approved" : "denied");
    } catch (e: unknown) {
      chatError = `The file action failed: ${errorMessage(e)}`;
      messages.push({ role: "assistant", content: "The file action failed. See the message above and try again.", ts: Date.now() });
    }
  };

  const pushToolResponse = (res: ToolExecutionResponse, outcome: string) => {
    const out = res.success
      ? `${res.output}${res.truncated ? "\n... [truncated at 1MB]" : ""}`
      : `${res.error ?? "no output"}`;
    messages.push({
      role: "assistant",
      content: `[tool ${outcome}] ${out}`.slice(0, 4200),
      ts: Date.now(),
    });
    persistCurrentThread();
  };

  // Load the chosen model into the engine and make it the chat default.
  const switchModel = async (id: string, name: string) => {
    pickerOpen = false;
    switchingModel = true;
    switchError = null;
    try {
      await api.loadInferenceModel(id);
      activeModelName = name;
      engineLoaded = true;
    } catch (e: unknown) {
      switchError = errorMessage(e);
    } finally {
      switchingModel = false;
    }
  };

  const togglePicker = () => {
    switchError = null;
    pickerOpen = !pickerOpen;
  };

  onMount(() => {
    activeThreadId = `t_${Date.now()}`;
    loadThreads();
  });

  // Escape stops an answer even when the message box does not have focus.
  // The message box and open menus handle Escape first and mark it handled.
  onMount(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && streaming && !event.defaultPrevented) {
        event.preventDefault();
        void stopGenerating();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  // Leaving Chat while an answer streams stops it and keeps what arrived.
  onDestroy(() => {
    if (!streaming) return;
    if (activeJobId) void api.cancelChat(activeJobId).catch(() => undefined);
    streaming = false;
    activeJobId = null;
    void persistCurrentThread();
  });
</script>

<div class="max-w-4xl mx-auto h-full flex flex-col relative">
  <PageHeader
    title="Chat"
    description={cloudChat ? `Online provider: ${cloudChat.label}.` : "Ask questions on this computer. Your messages stay here."}
  >
    {#snippet actions()}
      <Button variant="primary" onclick={newChat}><Icon name="plus" class="h-[18px] w-[18px]" />New chat</Button>
      <div class="relative" use:popover={{ open: showMoreActions, onClose: () => showMoreActions = false }}>
        <Button variant="subtle" ariaLabel="More chat actions" ariaExpanded={showMoreActions} ariaHaspopup="menu" onclick={() => showMoreActions = !showMoreActions}>
          <Icon name="more" class="h-[18px] w-[18px]" />
          More
        </Button>
        {#if showMoreActions}
          <div role="menu" aria-label="More chat actions" class="absolute right-0 top-10 z-30 min-w-44 rounded-md border border-edge bg-ink-900 p-1 shadow-xl">
            <button type="button" role="menuitem" onclick={() => { showSystem = !showSystem; showMoreActions = false; }} class="flex w-full items-center gap-2 rounded px-3 py-2 text-left text-sm text-ink-200 hover:bg-veil/5 focus:bg-veil/5 focus:outline-none">Response style</button>
            <button type="button" role="menuitem" onclick={() => { showMoreActions = false; void exportCurrentThread("markdown"); }} disabled={messages.length === 0 || streaming} class="flex w-full items-center gap-2 rounded px-3 py-2 text-left text-sm text-ink-200 hover:bg-veil/5 focus:bg-veil/5 focus:outline-none disabled:opacity-40">Save chat as a file</button>
            <button type="button" role="menuitem" onclick={() => { showMoreActions = false; void summarizeLast(); }} disabled={streaming || messages.length < 2} class="flex w-full items-center gap-2 rounded px-3 py-2 text-left text-sm text-ink-200 hover:bg-veil/5 focus:bg-veil/5 focus:outline-none disabled:opacity-40">Summarize last answer</button>
          </div>
        {/if}
      </div>
    {/snippet}
  </PageHeader>

  <div class="relative mb-3 flex items-center justify-between gap-3 border-b border-ink-800 pb-3">
    <div class="relative min-w-0" use:popover={{ open: showThreads, onClose: () => showThreads = false }}>
      <button
        type="button"
        aria-expanded={showThreads}
        aria-haspopup="dialog"
        onclick={() => showThreads = !showThreads}
        class="flex min-w-0 items-center gap-2 rounded-md border border-ink-700 bg-ink-900 px-3 py-2 text-sm font-semibold text-ink-200 hover:bg-ink-800"
      >
        <Icon name="chat" class="h-[18px] w-[18px] shrink-0 text-ink-400" />
        <span class="max-w-[180px] truncate">{activeThreadTitle}</span>
        <span class="text-sm text-ink-400">({threads.length})</span>
      </button>

      <!-- Saved chats. Each row has its own open and delete buttons. -->
      {#if showThreads}
        <div
          role="dialog"
          aria-label="Saved chats"
          class="absolute left-0 top-12 z-20 w-72 bg-ink-900 border border-edge rounded-2xl shadow-2xl p-3 space-y-2 max-h-96 overflow-y-auto"
          transition:fly={{ y: -6, duration: dur(120) }}
        >
          <div class="flex items-center justify-between pb-2 border-b border-edge text-xs font-semibold text-ink-300">
            <span>Saved chats</span>
            <button type="button" onclick={() => { newChat(); showThreads = false; }} class="text-xs text-ember-400 hover:underline">+ New</button>
          </div>
          {#if threads.length === 0}
            <div class="text-xs text-ink-400 py-3 text-center">No saved chats yet.</div>
          {:else}
            {#each threads as t (t.id)}
              <div class={`group flex items-center gap-1 rounded-xl text-xs transition-colors ${t.id === activeThreadId ? 'bg-ember-500/10 border border-ember-500/30 text-ink-100' : 'hover:bg-ink-800/50 text-ink-300'}`}>
                <button
                  type="button"
                  data-popover-item
                  aria-current={t.id === activeThreadId ? "true" : undefined}
                  onclick={() => selectThread(t)}
                  class="min-w-0 flex-1 rounded-xl p-2 text-left focus:outline-none focus-visible:ring-1 focus-visible:ring-ember-500"
                >
                  <div class="font-medium truncate">{t.title}</div>
                  <div class="text-sm text-ink-400">{t.message_count} messages</div>
                </button>
                <button
                  type="button"
                  onclick={() => askDeleteThread(t.id)}
                  class="mr-1 shrink-0 rounded p-1 text-ink-400 opacity-0 transition-opacity hover:text-rose-400 focus:opacity-100 group-hover:opacity-100"
                  title="Delete chat"
                  aria-label={`Delete chat ${t.title}`}
                >
                  <Icon name="close" class="w-3 h-3" />
                </button>
              </div>
            {/each}
          {/if}
        </div>
      {/if}
    </div>

    <div class="hidden items-center gap-2 sm:flex">
      {#if cloudChat}
        <span class="text-sm text-amber-300" title="Chat uses your chosen online provider.">Online: {cloudChat.model} · {cloudChat.label}</span>
      {:else if installedModels.length > 0}
        <span class={`h-2 w-2 rounded-full ${engineLoaded ? 'bg-emerald-500' : 'bg-ink-600'}`} aria-hidden="true"></span>
        <div class="relative" use:popover={{ open: pickerOpen, onClose: () => pickerOpen = false }}>
          <button
            type="button"
            aria-expanded={pickerOpen}
            aria-haspopup="menu"
            onclick={togglePicker}
            disabled={switchingModel || engineLoading}
            class="flex items-center gap-1.5 text-sm text-ink-400 hover:text-ink-200 disabled:opacity-50"
            title="Choose the model that answers"
          >
            {#if switchingModel || engineLoading}Loading…{:else if engineLoaded && activeModelName}{activeModelName}{:else}Load a model{/if}
            <Icon name="chevron-right" class={`h-4 w-4 transition-transform ${pickerOpen ? 'rotate-90' : ''}`} />
          </button>
          {#if switchError}
            <div class="absolute right-0 top-9 z-20 flex w-[300px] items-start gap-2 rounded-md border border-rose-500/40 bg-rose-950/90 px-3 py-2 text-sm text-rose-200" role="alert">
              <span class="min-w-0 flex-1 break-words">{switchError}</span>
              <button type="button" aria-label="Dismiss" onclick={() => switchError = null} class="shrink-0 rounded p-0.5 hover:bg-rose-500/20"><Icon name="close" class="h-3.5 w-3.5" /></button>
            </div>
          {:else if pickerOpen}
            <div role="menu" aria-label="Models on this computer" class="absolute right-0 top-9 z-20 max-h-64 min-w-[240px] overflow-y-auto rounded-md border border-edge bg-ink-950 py-1 shadow-xl">
              {#each installedModels as installed (installed.id)}
                {@const current = engineLoaded && activeModelName === installed.name}
                <button
                  type="button"
                  role="menuitemradio"
                  aria-checked={current}
                  data-popover-item
                  onclick={() => void switchModel(installed.id, installed.name)}
                  class={`w-full px-3 py-2 text-left text-sm hover:bg-ink-900 focus:bg-ink-900 focus:outline-none ${current ? 'text-ember-300' : 'text-ink-300'}`}
                >{installed.name}</button>
              {/each}
            </div>
          {/if}
        </div>
      {/if}
    </div>
  </div>

  <!-- Response style / System Prompt Selector -->
  {#if showSystem}
    <div class="p-3 bg-ink-900/60 border-b border-edge text-xs space-y-2">
      <div class="flex items-center gap-2 flex-wrap">
        <span class="text-ink-400">Styles:</span>
        {#each systemPresets as p}
          <button
            onclick={() => { systemPrompt = p.prompt; persona = p.label; void savePrefs(); }}
            class={`px-2 py-1 rounded text-sm transition-colors ${systemPrompt === p.prompt ? 'bg-ember-600 text-on-ember font-medium' : 'bg-ink-800 text-ink-300 hover:bg-ink-700'}`}
          >
            {p.label}
          </button>
        {/each}
        <span class="ml-auto flex items-center gap-2 text-ink-400">
          Creativity
          <input
            type="range"
            min="0"
            max="2"
            step="0.1"
            bind:value={temperature}
            onchange={savePrefs}
            class="w-24 accent-ember-600"
            aria-label="Sampling temperature"
          />
          <span class="text-sm text-ink-300">{temperature.toFixed(1)}</span>
        </span>
      </div>
      <textarea
        bind:value={systemPrompt}
        rows={2}
        onblur={savePrefs}
        class="w-full bg-ink-950 border border-edge rounded-lg p-2 text-ink-200 text-sm leading-6 focus:outline-none focus:border-ink-600"
      ></textarea>
    </div>
  {/if}

  {#if chatError}
    <div class="mb-3"><ErrorBanner message={chatError} onRetry={sendFailed && !streaming ? retryLast : undefined} onDismiss={() => { chatError = null; sendFailed = false; }} /></div>
  {/if}

  <!-- One short status per answer for screen readers. The stream itself is not
       a live region, so tokens are not read out one by one. -->
  <p class="sr-only" aria-live="polite">{liveStatus}</p>

  <!-- Message Stream Container -->
  <div
    bind:this={scrollEl}
    onscroll={handleScroll}
    role="region"
    aria-label="Conversation"
    class="flex-1 overflow-y-auto p-4 space-y-2 relative"
  >
    {#if messages.length === 0}
      <!-- Empty state: says what to do next, based on whether a model is active -->
      <div class="ember-glow h-full flex flex-col justify-center items-center text-center p-6 gap-7 rounded-lg">
        <div class="flex flex-col items-center gap-2 max-w-md" in:fly={{ y: 8, duration: dur(320) }}>
          <div class="relative mb-2">
            <div class="absolute inset-0 -z-10 scale-150 rounded-full bg-ember-500/15 blur-2xl" aria-hidden="true"></div>
            <LogoMark class="w-16 h-16 drop-shadow-lg" />
          </div>
          {#if cloudChat}
            <h3 class="text-xl font-semibold text-ink-50">{greeting} What can I help with?</h3>
            <p class="text-sm text-ink-400">Running {cloudChat.model} via {cloudChat.label}. Messages are sent to that provider.</p>
          {:else if !modelsChecked}
            <h3 class="text-xl font-semibold text-ink-50">Checking for a model…</h3>
          {:else if !activeModelName}
            <h3 class="text-xl font-semibold text-ink-50">Pick a model to start chatting</h3>
            <p class="text-sm text-ink-400">Models run on this computer. Download one in Models, or connect a free online provider in Settings.</p>
            {#if onOpenModels}
              <Button variant="primary" class="mt-2 h-9 px-4" onclick={onOpenModels}><Icon name="models" class="h-4 w-4" />Open Models</Button>
            {/if}
          {:else}
            <h3 class="text-xl font-semibold text-ink-50">{greeting} What can I help with?</h3>
            {#if engineLoading}
              <p class="flex items-center gap-2 text-sm text-ink-400" role="status"><span class="h-3 w-3 animate-spin rounded-full border-2 border-edge border-t-ember-500"></span>Loading {activeModelName}. This can take a minute.</p>
            {:else if !engineLoaded}
              <p class="text-sm text-ink-400">{activeModelName} is not loaded. Choose it in the model menu above.</p>
            {:else}
              <p class="text-sm text-ink-400">Running {activeModelName}. Nothing leaves this computer.</p>
            {/if}
          {/if}
        </div>

        {#if cloudChat || activeModelName}
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-2.5 max-w-xl w-full">
          {#each starterPrompts as sp, i (sp.title)}
            <button
              type="button"
              onclick={() => { input = sp.prompt; send(); }}
              in:fly={stagger(i + 2)}
              class="lift group flex items-start gap-3 text-left p-3.5 rounded-lg border border-ink-700 bg-ink-900/70 hover:border-edge hover:bg-ink-900"
            >
              <span class="grid h-8 w-8 shrink-0 place-items-center rounded-md bg-ink-800 text-ink-300 transition-colors group-hover:bg-ember-500/15 group-hover:text-ember-300"><Icon name={sp.icon} class="h-4 w-4" /></span>
              <span class="min-w-0">
                <span class="block text-sm font-semibold text-ink-100">{sp.title}</span>
                <span class="block text-sm text-ink-400">{sp.prompt}</span>
              </span>
            </button>
          {/each}
        </div>
        {#if skills.length > 0}
          <div class="flex max-w-xl flex-wrap items-center justify-center gap-2" in:fade={{ duration: dur(300), delay: dur(200) }}>
            <span class="text-xs text-ink-500">Your skills:</span>
            {#each skills.slice(0, 5) as skill (skill.path)}
              <button type="button" onclick={() => { input = skill.body; }} class="rounded-full border border-ink-700 px-3 py-1 text-xs text-ink-300 transition-colors hover:border-ember-500/50 hover:text-ember-200">/{skill.name}</button>
            {/each}
          </div>
        {/if}
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
        in:fly={{ y: 8, duration: dur(120) }}
        out:fade={{ duration: dur(100) }}
        class="absolute bottom-28 right-4 z-10 px-3 py-1.5 rounded-full bg-ink-800/90 border border-edge text-xs text-ink-200 shadow-xl hover:bg-ink-700 transition-colors flex items-center gap-1.5"
      >
        <Icon name="arrow-down" class="w-3.5 h-3.5" />
        <span>Scroll to bottom</span>
      </button>
    {/if}
  </div>

  <!-- Input Field -->
  <ChatInput
    bind:input={input}
    bind:toolPath={toolPath}
    bind:toolContent={toolContent}
    bind:showTools={showTools}
    streaming={streaming}
    onSend={send}
    onCancel={stopGenerating}
    onReadFile={readFile}
    onWriteFile={writeFile}
    pendingImages={pendingImages}
    onPickImages={addImages}
    onRemoveImage={removeImage}
    {skills}
    onPickSkill={(body) => { input = body; }}
  />

  <ToolApprovalModal
    show={showApproval}
    toolName={pendingTool?.tool_name ?? ""}
    action={pendingTool?.action ?? null}
    onApproveOnce={() => runApprovedTool(true)}
    onApproveSession={() => runApprovedTool(true, sessionToken)}
    onDeny={() => runApprovedTool(false)}
  />

  {#if pendingDeleteThreadId}
    <ConfirmDialog
      title="Delete this chat?"
      message="This removes the saved conversation from this computer. You cannot undo it."
      confirmLabel="Delete chat"
      onConfirm={() => void confirmDeleteThread()}
      onCancel={() => pendingDeleteThreadId = null}
    />
  {/if}
</div>
