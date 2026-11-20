<script lang="ts">
  import { onDestroy, onMount, tick, untrack } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { api, errorMessage } from "../lib/api/client";
  import { popover } from "../lib/actions/popover";
  import { appendReasoning, appendText, finishTool, partsForSaving, setToolStatus, settleParts, startTool, textOf } from "../lib/chatParts";
  import { dur, motion, stagger } from "../lib/motion";
  import { chatModes, flag, moduleOn } from "../lib/dev.svelte";
  import { isChatMode, MODE_ICONS } from "../lib/chatModes";
  import { requestSettings } from "../lib/nav.svelte";
  import { showToast } from "../lib/toast.svelte";
  import Button from "../components/Button.svelte";
  import ConfirmDialog from "../components/ConfirmDialog.svelte";
  import ErrorBanner from "../components/ErrorBanner.svelte";
  import Slider from "../components/Slider.svelte";
  import ToolApprovalModal from "../components/ToolApprovalModal.svelte";
  import type { ChatAttachment, ChatMessage, ChatModeId, ChatThread, EngineStatusEvent, McpServer, ModelChoice, ModelChoices, Skill, ToolAction, ToolExecutionResponse } from "../lib/api/bindings";
  import MessageBubble from "../components/MessageBubble.svelte";
  import Icon from "../components/Icon.svelte";
  import LogoMark from "../components/LogoMark.svelte";
  import ChatInput from "../components/ChatInput.svelte";
  import ModelPicker from "../components/ModelPicker.svelte";
  import EffortPicker from "../components/EffortPicker.svelte";
  import ContextMeter from "../components/ContextMeter.svelte";
  import { compactionStart, contextUsage, fits } from "../lib/contextUsage";
  import { conversationMarkdown, exportFileName } from "../lib/chatExport";

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

  // The model picker: local models and the models of every online provider
  // the user switched on. The active one answers.
  let choices = $state<ModelChoices | null>(null);
  // Experimental "Wide chat column": messages and the chat box fill the window.
  const chatWidth = $derived(flag("wide_chat") ? "max-w-none" : "max-w-3xl");
  let pickerOpen = $state(false);
  let pickerBusy = $state(false);
  let refreshingProvider = $state<string | null>(null);
  const activeChoice = $derived<ModelChoice | null>(choices?.models.find((m) => m.active) ?? null);
  const onlineChoice = $derived(activeChoice && activeChoice.provider_id !== null ? activeChoice : null);
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

  // Files waiting to go with the next message.
  let pendingFiles = $state<ChatAttachment[]>([]);
  let attaching = $state(false);
  let dragging = $state(false);
  // Tool calls waiting for the user's OK, by tool call id.
  let approvals = $state<Record<string, { approval_id: string; preview: string }>>({});
  // Tools the user allowed for the rest of this chat.
  let allowedTools = $state<string[]>([]);
  let mcpList = $state<McpServer[]>([]);

  let { onOpenModels, newThreadTick = 0 }: { onOpenModels?: () => void; newThreadTick?: number } = $props();

  // Mode and context switches, remembered between sessions.
  const OPTIONS_KEY = "localaide-chat-options";
  const savedOptions = (() => {
    try {
      return JSON.parse(localStorage.getItem(OPTIONS_KEY) ?? "{}") as {
        mode?: unknown;
        web?: unknown;
        recall?: unknown;
        files?: unknown;
        skills?: unknown;
        mcpOff?: unknown;
        efforts?: unknown;
        autoCompactAt?: unknown;
      };
    } catch {
      return {};
    }
  })();
  let mode = $state<ChatModeId>(isChatMode(savedOptions.mode) ? savedOptions.mode : "standard");
  let useWeb = $state(savedOptions.web === true);
  let useRecall = $state(savedOptions.recall !== false);
  let useFiles = $state(savedOptions.files !== false);
  let useSkills = $state(savedOptions.skills !== false);
  // MCP servers switched off in chat (they stay on in Settings).
  let mcpOff = $state<string[]>(Array.isArray(savedOptions.mcpOff) ? savedOptions.mcpOff.filter((v): v is string => typeof v === "string") : []);
  // The thinking effort chosen for each model, by picker key.
  let efforts = $state<Record<string, string>>(
    savedOptions.efforts && typeof savedOptions.efforts === "object" ? (savedOptions.efforts as Record<string, string>) : {},
  );
  const effort = $derived(activeChoice ? efforts[activeChoice.key] ?? "auto" : "auto");
  let autoCompactAt = $state(typeof savedOptions.autoCompactAt === "number" ? Math.max(50, Math.min(95, savedOptions.autoCompactAt)) : 80);
  $effect(() => {
    try {
      localStorage.setItem(
        OPTIONS_KEY,
        JSON.stringify({ mode, web: useWeb, recall: useRecall, files: useFiles, skills: useSkills, mcpOff, efforts, autoCompactAt }),
      );
    } catch {
      // Storage can be unavailable; the switches still work for this session.
    }
  });
  const webAvailable = $derived(moduleOn("web_in_chat"));
  const recallAvailable = $derived(moduleOn("memory"));
  const changeMode = (next: ChatModeId) => {
    mode = next;
    // Autonomous researches on its own, so it turns the web on (the user
    // can switch it off again).
    if (next === "autonomous" && webAvailable) useWeb = true;
  };
  const currentModeInfo = $derived(chatModes().find((m) => m.id === mode));
  let showSettingsPanel = $state(false);
  let threadQuery = $state("");

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
          if (state === "ready") {
            engineLoaded = true;
            void loadChoices();
          }
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

  const loadChoices = async () => {
    try {
      choices = await api.chatModels();
      engineLoaded = choices.local_loaded;
    } catch (e: unknown) {
      chatError = `The model list could not load: ${errorMessage(e)}`;
    } finally {
      modelsChecked = true;
    }
  };

  const loadMcp = async () => {
    if (!moduleOn("mcp")) return;
    try {
      mcpList = (await api.mcpServers()).filter((s) => s.enabled);
    } catch {
      mcpList = [];
    }
  };

  onMount(async () => {
    await loadChoices();
    void loadMcp();
    // Restore the chat preferences from the last session (honest local
    // adaptation: what the user chose, not a profile).
    try {
      const prefs = await api.getChatPrefs();
      if (prefs.system_prompt) systemPrompt = prefs.system_prompt;
      if (prefs.temperature !== null) temperature = prefs.temperature;
      // 512 was the old default, saved on every send; it is too short once
      // a model thinks, so it moves up to the new default.
      if (prefs.max_tokens !== null && prefs.max_tokens !== 512) maxTokens = prefs.max_tokens;
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
  let maxTokens = $state(2048);
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
    pendingFiles = [];
    allowedTools = [];
    approvals = {};
    compactedBefore = 0;
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
    allowedTools = [];
    approvals = {};
    compactedBefore = 0;
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
    const textOnly = $state.snapshot(messages).map(({ images: _images, steps: _steps, stats: _stats, ...rest }) => ({
      ...rest,
      parts: rest.parts && rest.parts.length > 0 ? partsForSaving(rest.parts) : undefined,
      attachments: rest.attachments?.map((a) => ({ ...a, data_url: null })),
    }));
    try {
      await api.saveChatThread(activeThreadId, activeThreadTitle, systemPrompt, textOnly);
      await loadThreads();
    } catch (e: unknown) {
      sendFailed = false;
      chatError = `The chat could not be saved: ${errorMessage(e)}`;
    }
  };

  const chatMarkdown = () =>
    conversationMarkdown($state.snapshot(messages), {
      title: activeThreadTitle === "New Conversation" ? "Chat" : activeThreadTitle,
      systemPrompt,
    });

  const copyCurrentThread = async () => {
    if (messages.length === 0) return;
    try {
      await navigator.clipboard.writeText(chatMarkdown());
      showToast("Conversation copied.");
    } catch {
      chatError = "The conversation could not be copied. Check clipboard permission and try again.";
    }
  };

  const exportCurrentThread = async (format: "markdown" | "json") => {
    if (!activeThreadId || messages.length === 0) return;
    try {
      // Markdown is built here, the same text "Copy full conversation" gives.
      // JSON is the saved file, so save first to include the newest answer.
      await persistCurrentThread();
      const savedTo =
        format === "markdown"
          ? await api.saveChatMarkdown(exportFileName(activeThreadTitle, "md"), chatMarkdown())
          : await api.saveChatExport({ thread_id: activeThreadId, format });
      if (savedTo) showToast(`Chat saved to ${savedTo}`);
    } catch (e: unknown) {
      sendFailed = false;
      chatError = `The chat could not be saved as a file: ${errorMessage(e)}`;
    }
  };

  // Compaction never deletes messages. It moves this cut point: messages
  // before it stay on screen and in the saved chat, but are not sent to the
  // model. It resets with each chat.
  let compactedBefore = $state(0);
  const historyFrom = $derived(Math.min(compactedBefore, messages.length));
  const answerReserve = $derived(maxTokens + (activeChoice?.reasoning && effort !== "off" ? 4096 : 0));
  const usage = $derived(
    contextUsage(messages, activeChoice?.context, { from: historyFrom, draft: input, systemPrompt, reserve: answerReserve }),
  );

  /** Move the cut so the rest fits under `threshold` percent. True when it moved. */
  const compactConversation = (threshold = autoCompactAt, draft = "") => {
    const limit = activeChoice?.context;
    if (!limit) {
      // No known window: keep the last four messages.
      const next = Math.max(historyFrom, messages.length - 4);
      if (next <= historyFrom) return false;
      compactedBefore = next;
      return true;
    }
    const next = compactionStart(messages, limit, threshold, { from: historyFrom, draft, systemPrompt, reserve: answerReserve });
    if (next <= historyFrom) return false;
    compactedBefore = next;
    return true;
  };

  const handleSlashCommand = (command: string) => {
    switch (command) {
      case "clear":
        newChat();
        break;
      case "compact":
        // A manual compact aims for half the window, leaving room to talk.
        if (compactConversation(Math.min(50, autoCompactAt))) {
          showToast("Older messages stay on screen but are no longer sent to the model.");
        } else {
          showToast("This conversation already fits.");
        }
        break;
      case "model":
        pickerOpen = true;
        break;
      case "settings":
        showSettingsPanel = true;
        break;
      case "export":
        void exportCurrentThread("markdown");
        break;
      case "copy":
        void copyCurrentThread();
        break;
      case "by-the-way":
        input = "By the way: ";
        break;
      case "goal":
        input = "My goal is: ";
        break;
      case "scale":
        input = "Answer at the right level of detail. Keep the response concise unless the task needs depth.\n\n";
        break;
      case "remember":
        input = "Remember this: ";
        useRecall = true;
        break;
      case "create-skill":
        input = "Create a reusable skill from this instruction: ";
        useSkills = true;
        break;
      case "web":
        useWeb = true;
        showToast("Web tools enabled for the next message.");
        break;
      case "notes":
        useRecall = true;
        showToast("Notes and document recall enabled.");
        break;
    }
  };
  const nowLabel = () => {
    const d = new Date();
    const zone = Intl.DateTimeFormat().resolvedOptions().timeZone;
    return `${d.toLocaleString(undefined, { weekday: "long", year: "numeric", month: "long", day: "numeric", hour: "2-digit", minute: "2-digit" })}${zone ? ` (${zone})` : ""}`;
  };

  const send = async () => {
    const files = $state.snapshot(pendingFiles).filter((f) => !f.error);
    if ((!input.trim() && pendingImages.length === 0 && files.length === 0) || streaming) return;
    const userMsg = input.trim();
    input = "";
    if (activeChoice?.context && !fits(contextUsage(messages, activeChoice.context, { from: historyFrom, draft: userMsg, systemPrompt, reserve: answerReserve }), autoCompactAt)) {
      if (compactConversation(autoCompactAt, userMsg)) showToast(`The chat reached ${autoCompactAt}% of the context window, so older messages are no longer sent.`);
    }

    // Earlier turns only. The new message is sent separately as `message`.
    // Failed answers are left out, so the model does not repeat them.
    const history = $state
      .snapshot(messages)
      .slice(historyFrom)
      .filter((m) => m.content.trim() && !m.error && !m.content.startsWith("[Chat Error"))
      .map((m) => ({
        role: m.role,
        content: m.content,
        attachments: m.attachments?.filter((a) => a.kind === "text").map((a) => ({ ...a, data_url: null })),
      }));

    const images = $state.snapshot(pendingImages);
    pendingImages = [];
    pendingFiles = [];
    messages.push({ role: "user", content: userMsg, ts: Date.now(), images, attachments: files.length ? files : undefined });
    streaming = true;
    sendFailed = false;
    liveStatus = "LocalAide is answering.";
    userScrolledUp = false;
    void savePrefs();

    const assistantIdx = messages.length;
    messages.push({
      role: "assistant",
      content: "",
      ts: Date.now(),
      steps: [],
      parts: [],
      model: activeChoice ? (activeChoice.provider_id ? `${activeChoice.name} · ${activeChoice.provider_label}` : activeChoice.name) : undefined,
    });
    const started = performance.now();
    let firstTokenAt: number | null = null;
    const answer = () => messages[assistantIdx];
    const parts = () => {
      const msg = answer();
      if (!msg.parts) msg.parts = [];
      return msg.parts;
    };
    const fail = (message: string) => {
      settleParts(parts(), performance.now());
      if (!answer().content.trim()) answer().content = "I could not answer that. Try again.";
      answer().error = true;
      chatError = message;
      sendFailed = true;
      streaming = false;
      activeJobId = null;
      approvals = {};
      liveStatus = "LocalAide could not answer.";
      void persistCurrentThread();
    };

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
          mode,
          use_web: webAvailable && useWeb,
          use_recall: recallAvailable && useRecall,
          use_files: useFiles,
          use_skills: moduleOn("skills") && useSkills,
          mcp_servers: mcpList.filter((m) => !mcpOff.includes(m.id)).map((m) => m.id),
          allowed_tools: [...allowedTools],
          target: activeChoice ? { provider_id: activeChoice.provider_id, model: activeChoice.model } : undefined,
          effort,
          attachments: files,
          now: nowLabel(),
        },
        (event) => {
          const now = performance.now();
          if (event.type === "token") {
            if (firstTokenAt === null) firstTokenAt = now;
            appendText(parts(), event.payload.token, now);
            answer().content = textOf(parts());
          } else if (event.type === "reasoning") {
            if (firstTokenAt === null) firstTokenAt = now;
            appendReasoning(parts(), event.payload.text, now);
          } else if (event.type === "tool_call") {
            startTool(parts(), event.payload, now);
          } else if (event.type === "tool_approval") {
            setToolStatus(parts(), event.payload.id, "waiting");
            approvals[event.payload.id] = { approval_id: event.payload.approval_id, preview: event.payload.preview };
            liveStatus = `LocalAide asks before it can ${event.payload.title.toLowerCase()}.`;
          } else if (event.type === "tool_result") {
            finishTool(parts(), event.payload);
            delete approvals[event.payload.id];
          } else if (event.type === "status") {
            const label = event.payload.label;
            if (label.startsWith("Answering with ")) {
              answer().model = label.slice("Answering with ".length);
            } else if (label !== "Writing the answer") {
              answer().steps = [...(answer().steps ?? []), label];
            }
          } else if (event.type === "citation") {
            const cur = answer().citations || [];
            if (!cur.some((c) => c.index === event.payload.index)) answer().citations = [...cur, event.payload];
          } else if (event.type === "done") {
            settleParts(parts(), now);
            answer().stats = {
              tokens: event.payload.total_tokens,
              inputTokens: contextUsage(messages.slice(0, assistantIdx), null, { from: historyFrom, systemPrompt }).used,
              outputTokens: event.payload.total_tokens,
              ms: now - started,
              firstTokenMs: firstTokenAt === null ? null : firstTokenAt - started,
            };
            streaming = false;
            activeJobId = null;
            approvals = {};
            liveStatus = "Answer ready.";
            void persistCurrentThread();
          } else if (event.type === "error") {
            fail(errorMessage(event.payload.error));
          }
        },
      );
    } catch (e: unknown) {
      fail(errorMessage(e));
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
    approvals = {};
    const last = messages.at(-1);
    if (last?.role === "assistant" && last.parts) settleParts(last.parts, performance.now());
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
    pendingFiles = [...(lastUser.attachments ?? [])];
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

  // Hand a task over to the user's OpenCode server. For an answer, the
  // question that led to it goes along, so OpenCode has the whole task.
  const sendToOpenCode = async (index: number) => {
    const msg = messages[index];
    if (!msg) return;
    let task = msg.content;
    if (msg.role === "assistant") {
      const question = messages.slice(0, index).reverse().find((m) => m.role === "user");
      task = question ? `${question.content}\n\nNotes from LocalAide:\n${msg.content}` : msg.content;
    }
    try {
      const handoff = await api.opencodeSend(task, activeThreadTitle === "New Conversation" ? undefined : activeThreadTitle);
      showToast(`Sent to OpenCode as “${handoff.title}”.`);
    } catch (e: unknown) {
      const err = e as { code?: string };
      if (err?.code === "OPENCODE_NOT_LINKED") {
        showToast("Link OpenCode first: Settings → Integrations.", "error");
        requestSettings("integrations");
      } else {
        showToast(`OpenCode: ${errorMessage(e)}`, "error");
      }
    }
  };

  // Group saved chats by when they were last used.
  const threadGroups = $derived.by(() => {
    const q = threadQuery.trim().toLowerCase();
    const list = threads.filter((t) => !q || t.title.toLowerCase().includes(q));
    const day = 24 * 60 * 60 * 1000;
    const startOfToday = new Date().setHours(0, 0, 0, 0);
    const groups: { label: string; items: ChatThread[] }[] = [
      { label: "Today", items: [] },
      { label: "Yesterday", items: [] },
      { label: "This week", items: [] },
      { label: "Older", items: [] },
    ];
    for (const t of [...list].sort((a, b) => b.updated_at - a.updated_at)) {
      const at = t.updated_at < 1e12 ? t.updated_at * 1000 : t.updated_at;
      const i = at >= startOfToday ? 0 : at >= startOfToday - day ? 1 : at >= startOfToday - 6 * day ? 2 : 3;
      groups[i].items.push(t);
    }
    return groups.filter((g) => g.items.length > 0);
  });

  // Make the chosen model the one that answers. A local model loads first.
  const selectModel = async (choice: ModelChoice) => {
    if (choice.active) return;
    pickerBusy = true;
    try {
      choices = await api.setChatTarget({ provider_id: choice.provider_id, model: choice.model });
      engineLoaded = choices.local_loaded;
    } catch (e: unknown) {
      sendFailed = false;
      chatError = `${choice.name} could not be used: ${errorMessage(e)}`;
    } finally {
      pickerBusy = false;
    }
  };

  const toggleFavorite = async (key: string) => {
    try {
      const starred = await api.toggleFavoriteModel(key);
      if (choices) for (const m of choices.models) m.favorite = starred.includes(m.key);
    } catch (e: unknown) {
      chatError = `The star could not be saved: ${errorMessage(e)}`;
    }
  };

  const refreshProvider = async (providerId: string) => {
    refreshingProvider = providerId;
    try {
      await api.refreshProviderModels(providerId);
      await loadChoices();
    } catch (e: unknown) {
      sendFailed = false;
      chatError = `The model list could not load: ${errorMessage(e)}`;
    } finally {
      refreshingProvider = null;
    }
  };

  const setEffort = (value: string) => {
    if (activeChoice) efforts = { ...efforts, [activeChoice.key]: value };
  };

  const toggleMcp = (id: string) => {
    mcpOff = mcpOff.includes(id) ? mcpOff.filter((x) => x !== id) : [...mcpOff, id];
  };

  // Files from the attach menu or a drop. Images join the image list.
  const addAttachments = (files: ChatAttachment[]) => {
    for (const file of files) {
      if (file.kind === "image" && file.data_url) {
        if (pendingImages.length < MAX_IMAGES) pendingImages.push(file.data_url);
      } else if (!pendingFiles.some((f) => f.name === file.name && f.chars === file.chars)) {
        pendingFiles.push(file);
      }
    }
  };

  const attachFiles = async () => {
    attaching = true;
    try {
      addAttachments(await api.attachFiles());
    } catch (e: unknown) {
      chatError = `The files could not be read: ${errorMessage(e)}`;
    } finally {
      attaching = false;
    }
  };

  const attachDropped = async (paths: string[]) => {
    if (paths.length === 0) return;
    attaching = true;
    try {
      addAttachments(await api.attachDropped(paths));
    } catch (e: unknown) {
      chatError = `The files could not be read: ${errorMessage(e)}`;
    } finally {
      attaching = false;
    }
  };

  // Files dropped on the window attach to the next message.
  onMount(() => {
    let stop: (() => void) | undefined;
    let closed = false;
    void getCurrentWebview()
      .onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === "enter" || p.type === "over") dragging = true;
        else if (p.type === "leave") dragging = false;
        else if (p.type === "drop") {
          dragging = false;
          void attachDropped(p.paths);
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

  // The user's answer to a tool that wants to change something.
  const decideTool = async (toolId: string, decision: "allow" | "allow_chat" | "deny") => {
    const pending = approvals[toolId];
    if (!pending) return;
    const answer = messages.at(-1);
    const part = answer?.parts?.find((p) => p.type === "tool" && p.id === toolId);
    if (answer?.parts) setToolStatus(answer.parts, toolId, decision === "deny" ? "denied" : "running");
    if (decision === "allow_chat" && part?.type === "tool" && !allowedTools.includes(part.name)) {
      allowedTools = [...allowedTools, part.name];
    }
    delete approvals[toolId];
    try {
      const delivered = await api.decideTool(pending.approval_id, decision);
      if (!delivered) showToast("That question was already closed.", "error");
    } catch (e: unknown) {
      chatError = `Your answer could not be sent: ${errorMessage(e)}`;
    }
  };

  onMount(() => {
    activeThreadId = `t_${Date.now()}`;
    loadThreads();
  });

  // Escape stops an answer even when the message box does not have focus.
  // The message box and open menus handle Escape first and mark it handled.
  onMount(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.shiftKey && event.key.toLowerCase() === "m") {
        event.preventDefault();
        pickerOpen = !pickerOpen;
        return;
      }
      if (event.key === "Escape" && (showSettingsPanel || showThreads) && !event.defaultPrevented) {
        showSettingsPanel = false;
        showThreads = false;
        return;
      }
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

<div class="chat-root relative flex h-full min-h-0 min-w-0 flex-col overflow-hidden">
  <!-- Top bar: saved chats, the engine that answers, and chat actions. -->
  <header class="relative z-20 flex h-14 shrink-0 items-center gap-2 border-b border-ink-800 bg-ink-900/85 px-3 backdrop-blur md:px-5">
    <button
      type="button"
      aria-expanded={showThreads}
      aria-controls="chat-history"
      onclick={() => { showThreads = !showThreads; if (showThreads) void loadThreads(); }}
      class="flex min-w-0 items-center gap-2 rounded-lg px-2.5 py-1.5 text-sm text-ink-100 transition-colors hover:bg-veil/5"
      title="Saved chats"
    >
      <Icon name="clock" class="h-[18px] w-[18px] shrink-0 text-ink-400" />
      <span class="max-w-[16rem] truncate font-semibold">{activeThreadTitle === "New Conversation" ? "New chat" : activeThreadTitle}</span>
      <Icon name="chevron-down" class={`h-4 w-4 shrink-0 text-ink-500 transition-transform duration-200 ${showThreads ? "rotate-180" : ""}`} />
    </button>

    <div class="ml-auto flex items-center gap-1.5">
      {#if onlineChoice}
        <span class="inline-flex min-w-0 max-w-[15rem] items-center gap-1.5 rounded-full border border-ember-500/35 bg-ember-500/10 px-2.5 py-1 text-xs font-medium text-ember-200" title={`Messages go to ${onlineChoice.provider_label}.`}>
          <Icon name="cloud" class="h-3.5 w-3.5 shrink-0" /><span class="truncate">Online · {onlineChoice.provider_label}</span>
        </span>
      {:else}
        <span class="inline-flex min-w-0 max-w-[15rem] items-center gap-1.5 rounded-full border border-ember-500/30 bg-ember-500/5 px-2.5 py-1 text-xs font-medium text-ember-200" title="Messages stay on this computer.">
          <span class="h-2 w-2 shrink-0 rounded-full bg-ember-500" aria-hidden="true"></span><span class="truncate">Offline · this computer</span>
        </span>
      {/if}

      <button type="button" onclick={() => showSettingsPanel = !showSettingsPanel} aria-pressed={showSettingsPanel} aria-label="Chat settings" title="Chat settings" class="chat-icon-btn"><Icon name="sliders" class="h-[18px] w-[18px]" /></button>
      <div class="relative" use:popover={{ open: showMoreActions, onClose: () => showMoreActions = false }}>
        <button type="button" aria-label="More chat actions" aria-expanded={showMoreActions} aria-haspopup="menu" onclick={() => showMoreActions = !showMoreActions} class="chat-icon-btn"><Icon name="more" class="h-[18px] w-[18px]" /></button>
        {#if showMoreActions}
          <div role="menu" aria-label="More chat actions" class="absolute right-0 top-10 z-30 min-w-52 rounded-xl border border-edge bg-ink-950 p-1.5 shadow-2xl" in:fly={{ y: -4, duration: dur(motion.base), easing: motion.enter }}>
            <button type="button" role="menuitem" data-popover-item onclick={() => { showMoreActions = false; void exportCurrentThread("markdown"); }} disabled={messages.length === 0 || streaming} class="menu-item"><Icon name="download" class="h-4 w-4" />Export conversation as Markdown</button>
            <button type="button" role="menuitem" data-popover-item onclick={() => { showMoreActions = false; void copyCurrentThread(); }} disabled={messages.length === 0 || streaming} class="menu-item"><Icon name="copy" class="h-4 w-4" />Copy full conversation</button>
            <button type="button" role="menuitem" data-popover-item onclick={() => { showMoreActions = false; void exportCurrentThread("json"); }} disabled={messages.length === 0 || streaming} class="menu-item"><Icon name="file" class="h-4 w-4" />Export conversation as JSON</button>
            <button type="button" role="menuitem" data-popover-item onclick={() => { showMoreActions = false; void summarizeLast(); }} disabled={streaming || messages.length < 2} class="menu-item"><Icon name="file" class="h-4 w-4" />Summarize last answer</button>
          </div>
        {/if}
      </div>
      <button type="button" onclick={newChat} disabled={streaming} class="ml-1 flex items-center gap-1.5 rounded-lg bg-ember-500 px-3 py-1.5 text-sm font-semibold text-on-ember shadow-sm shadow-ember-900/40 transition-[background-color,transform] hover:bg-ember-400 active:scale-[0.97] disabled:opacity-50" title="New chat (Ctrl+N)"><Icon name="plus" class="h-4 w-4" /><span class="hidden sm:inline">New chat</span></button>
    </div>
  </header>

  <!-- Saved chats drawer -->
  {#if showThreads}
    <div role="presentation" class="absolute inset-0 top-14 z-20 bg-black/30" onclick={() => showThreads = false} in:fade={{ duration: dur(motion.fast) }} out:fade={{ duration: dur(motion.fast) }}></div>
    <div
      id="chat-history"
      role="dialog"
      aria-label="Saved chats"
      tabindex="-1"
      onkeydown={(e) => { if (e.key === "Escape") showThreads = false; }}
      class="absolute bottom-0 left-0 top-14 z-30 flex w-80 max-w-[85vw] flex-col border-r border-edge bg-ink-950 shadow-2xl"
      transition:fly={{ x: -24, duration: dur(motion.base), easing: motion.enter }}
    >
      <div class="flex items-center gap-2 border-b border-ink-800 p-3">
        <span class="field-shell flex h-9 flex-1 items-center gap-2 rounded-lg border border-ink-700 bg-ink-900 px-2.5">
          <Icon name="search" class="h-4 w-4 shrink-0 text-ink-500" />
          <input type="search" bind:value={threadQuery} aria-label="Find a chat" placeholder="Find a chat" class="field-bare min-w-0 flex-1 bg-transparent text-sm text-ink-100 placeholder:text-ink-500" />
        </span>
        <button type="button" aria-label="Close saved chats" onclick={() => showThreads = false} class="chat-icon-btn"><Icon name="close" class="h-4 w-4" /></button>
      </div>
      <div class="flex-1 overflow-y-auto p-2">
        {#each threadGroups as group (group.label)}
          <div class="px-2 pb-1 pt-3 text-xs font-semibold text-ink-500">{group.label}</div>
          {#each group.items as t (t.id)}
            <div class={`group flex items-center rounded-lg transition-colors ${t.id === activeThreadId ? "bg-ember-500/10 text-ink-50" : "text-ink-300 hover:bg-veil/5"}`}>
              <button type="button" aria-current={t.id === activeThreadId ? "true" : undefined} onclick={() => selectThread(t)} class="min-w-0 flex-1 px-2.5 py-2 text-left">
                <div class="truncate text-sm font-medium">{t.title}</div>
                <div class="text-xs text-ink-500">{t.message_count} messages</div>
              </button>
              <button type="button" onclick={() => askDeleteThread(t.id)} class="mr-1 rounded p-1.5 text-ink-500 opacity-0 transition-opacity hover:text-rose-400 focus:opacity-100 group-hover:opacity-100" aria-label={`Delete chat ${t.title}`} title="Delete chat"><Icon name="trash" class="h-4 w-4" /></button>
            </div>
          {/each}
        {:else}
          <p class="px-3 py-8 text-center text-sm text-ink-500">{threadQuery ? "No chat matches." : "No saved chats yet. Chats save themselves after the first answer."}</p>
        {/each}
      </div>
    </div>
  {/if}

  <!-- Chat settings sheet -->
  {#if showSettingsPanel}
    <div
      role="dialog"
      aria-label="Chat settings"
      tabindex="-1"
      onkeydown={(e) => { if (e.key === "Escape") showSettingsPanel = false; }}
      class="absolute bottom-0 right-0 top-14 z-30 flex w-96 max-w-[90vw] flex-col border-l border-edge bg-ink-950 shadow-2xl"
      transition:fly={{ x: 24, duration: dur(motion.base), easing: motion.enter }}
    >
      <div class="flex items-center justify-between border-b border-ink-800 px-4 py-3">
        <h2 class="font-semibold text-ink-50">Chat settings</h2>
        <button type="button" aria-label="Close chat settings" onclick={() => showSettingsPanel = false} class="chat-icon-btn"><Icon name="close" class="h-4 w-4" /></button>
      </div>
      <div class="flex-1 space-y-6 overflow-y-auto p-4">
        <section>
          <h3 class="mb-2 text-xs font-semibold uppercase tracking-wide text-ink-500">Response style</h3>
          <div class="grid grid-cols-2 gap-2">
            {#each systemPresets as p (p.label)}
              <button type="button" onclick={() => { systemPrompt = p.prompt; persona = p.label; void savePrefs(); }} aria-pressed={systemPrompt === p.prompt} class={`rounded-lg border px-3 py-2 text-left text-sm transition-colors ${systemPrompt === p.prompt ? "border-ember-500/50 bg-ember-500/10 text-ink-50" : "border-ink-700 text-ink-300 hover:border-edge hover:text-ink-50"}`}>{p.label}</button>
            {/each}
          </div>
        </section>

        <section>
          <h3 class="mb-2 text-xs font-semibold uppercase tracking-wide text-ink-500">Mode</h3>
          <div class="space-y-1.5">
            {#each chatModes() as m (m.id)}
              <button type="button" onclick={() => changeMode(m.id)} aria-pressed={mode === m.id} class={`flex w-full items-start gap-3 rounded-lg border px-3 py-2.5 text-left transition-colors ${mode === m.id ? "border-ember-500/50 bg-ember-500/10" : "border-ink-700 hover:border-edge"}`}>
                <Icon name={MODE_ICONS[m.id]} class={`mt-0.5 h-4 w-4 shrink-0 ${mode === m.id ? "text-ember-300" : "text-ink-400"}`} />
                <span><span class="block text-sm font-semibold text-ink-100">{m.label}</span><span class="block text-xs leading-5 text-ink-400">{m.description}</span></span>
              </button>
            {/each}
          </div>
        </section>

        {#if flag("sampling_controls")}
          <section>
            <h3 class="mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-ink-500"><Icon name="flask" class="h-3.5 w-3.5" />Sampling</h3>
            <label class="block text-sm text-ink-300">
              <span class="flex justify-between"><span>Creativity (temperature)</span><span class="font-mono text-ink-100">{temperature.toFixed(1)}{mode === "strict" ? " · capped at 0.3" : ""}</span></span>
              <Slider min={0} max={2} step={0.1} bind:value={temperature} onchange={savePrefs} class="mt-1 w-full" label="Temperature" />
            </label>
            <label class="mt-3 block text-sm text-ink-300">
              <span class="flex justify-between"><span>Longest answer</span><span class="font-mono text-ink-100">{maxTokens} tokens</span></span>
              <Slider min={64} max={8192} step={64} bind:value={maxTokens} onchange={savePrefs} class="mt-1 w-full" label="Longest answer in tokens" />
            </label>
          </section>
        {/if}

        <section>
          <h3 class="mb-2 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-ink-500"><Icon name="layers" class="h-3.5 w-3.5" />Context window</h3>
          <label class="block text-sm text-ink-300">
            <span class="flex justify-between"><span>Auto-compact at</span><span class="font-mono text-ink-100">{autoCompactAt}%</span></span>
            <Slider min={50} max={95} step={5} bind:value={autoCompactAt} class="mt-1 w-full" label="Auto-compact threshold" />
          </label>
          <p class="mt-2 text-xs leading-5 text-ink-500">Older turns are shortened before a new answer when the context meter reaches this level. Use the circle beside the model selector to see estimated input and output usage.</p>
        </section>

        {#if flag("system_prompt_editor")}
          <section>
            <textarea bind:value={systemPrompt} rows={6} onblur={savePrefs} aria-label="System prompt" class="w-full resize-y rounded-lg border border-ink-700 bg-ink-900 p-3 font-mono text-[13px] leading-6 text-ink-100"></textarea>
            {#if currentModeInfo?.instruction}
              <p class="mb-1 mt-3 text-xs font-semibold text-ink-400">{currentModeInfo.label} mode adds:</p>
              <p class="rounded-lg border border-ink-800 bg-ink-900/60 p-3 font-mono text-[12px] leading-5 text-ink-400">{currentModeInfo.instruction}</p>
            {/if}
          </section>
        {:else}
          <p class="text-xs leading-5 text-ink-500">Turn on developer mode in Settings to write your own system prompt and adjust sampling.</p>
        {/if}
      </div>
    </div>
  {/if}

  <!-- One short status per answer for screen readers. The stream itself is not
       a live region, so tokens are not read out one by one. -->
  <p class="sr-only" aria-live="polite">{liveStatus}</p>

  <div bind:this={scrollEl} onscroll={handleScroll} role="region" aria-label="Conversation" class="relative min-h-0 flex-1 overflow-y-auto">
    {#if messages.length === 0}
      <div class={`ember-glow mx-auto flex min-h-full ${chatWidth} flex-col items-center justify-center gap-8 px-5 py-10 text-center`}>
        <div class="flex max-w-lg flex-col items-center gap-2" in:fly={{ y: 10, duration: dur(360), easing: motion.enter }}>
          <div class="relative mb-3">
            <div class="absolute inset-0 -z-10 scale-[1.8] rounded-full bg-ember-500/20 blur-3xl" aria-hidden="true"></div>
            <LogoMark class="h-16 w-16 drop-shadow-xl" />
          </div>
          {#if !modelsChecked}
            <h1 class="text-2xl font-semibold tracking-tight text-ink-50">Getting ready…</h1>
          {:else if !activeChoice}
            <h1 class="text-2xl font-semibold tracking-tight text-ink-50">Pick a model to start</h1>
            <p class="text-[15px] leading-6 text-ink-400">{choices && choices.models.length > 0 ? "Choose one in the model menu under the message box." : "Models run on this computer. Download one, or connect a free online provider such as Kilo Code in Settings."}</p>
            <div class="mt-3 flex flex-wrap justify-center gap-2">
              {#if onOpenModels}<Button variant="primary" class="h-9 px-4" onclick={onOpenModels}><Icon name="models" class="h-4 w-4" />Open Models</Button>{/if}
              {#if moduleOn("online")}<Button variant="secondary" class="h-9 px-4" onclick={() => requestSettings("online")}><Icon name="cloud" class="h-4 w-4" />Use a free provider</Button>{/if}
            </div>
          {:else}
            <h1 class="text-[28px] font-semibold tracking-tight text-ink-50">{greeting} What can I help with?</h1>
            {#if onlineChoice}
              <p class="text-[15px] text-ink-400">Answers come from {onlineChoice.name} via {onlineChoice.provider_label}. Messages are sent to that provider.</p>
            {:else if engineLoading || pickerBusy}
              <p class="flex items-center gap-2 text-[15px] text-ink-400" role="status"><span class="h-3.5 w-3.5 animate-spin rounded-full border-2 border-edge border-t-ember-500"></span>Loading {activeChoice.name}. This can take a minute.</p>
            {:else if !engineLoaded}
              <p class="text-[15px] text-ink-400">{activeChoice.name} is not loaded yet. Choose it in the model menu under the message box.</p>
            {:else}
              <p class="text-[15px] text-ink-400">Running {activeChoice.name} on this computer. Nothing you type leaves it{useWeb && webAvailable ? ", except web searches the model makes" : ""}.</p>
            {/if}
            {#if mode !== "standard" && currentModeInfo}
              <span class="mt-2 inline-flex items-center gap-1.5 rounded-full border border-ember-500/40 bg-ember-500/10 px-3 py-1 text-xs font-medium text-ember-200"><Icon name={MODE_ICONS[mode]} class="h-3.5 w-3.5" />{currentModeInfo.label} mode</span>
            {/if}
          {/if}
        </div>

        {#if activeChoice}
          <div class="grid w-full max-w-2xl grid-cols-1 gap-2.5 sm:grid-cols-2">
            {#each starterPrompts as sp, i (sp.title)}
              <button type="button" onclick={() => { input = sp.prompt; send(); }} in:fly={stagger(i + 2)} class="lift group flex items-start gap-3 rounded-xl border border-ink-700 bg-ink-900/80 p-3.5 text-left hover:border-edge hover:bg-ink-900">
                <span class="grid h-9 w-9 shrink-0 place-items-center rounded-lg bg-ink-800 text-ink-300 transition-colors group-hover:bg-ember-500/15 group-hover:text-ember-300"><Icon name={sp.icon} class="h-[18px] w-[18px]" /></span>
                <span class="min-w-0">
                  <span class="block text-sm font-semibold text-ink-100">{sp.title}</span>
                  <span class="block text-sm text-ink-400">{sp.prompt}</span>
                </span>
              </button>
            {/each}
          </div>
          {#if skills.length > 0 && moduleOn("skills")}
            <div class="flex max-w-2xl flex-wrap items-center justify-center gap-2" in:fade={{ duration: dur(300), delay: dur(200) }}>
              <span class="text-xs text-ink-500">Your skills:</span>
              {#each skills.slice(0, 6) as skill (skill.path)}
                <button type="button" onclick={() => { input = skill.body; }} class="rounded-full border border-ink-700 px-3 py-1 text-xs text-ink-300 transition-colors hover:border-ember-500/50 hover:text-ember-200">/{skill.name}</button>
              {/each}
            </div>
          {/if}
        {/if}
      </div>
    {:else}
      <div class={`mx-auto w-full ${chatWidth} px-4 py-6 md:px-6 ${flag("compact") ? "space-y-4" : "space-y-7"}`}>
        {#each messages as msg, idx (idx)}
          {#if idx === historyFrom && historyFrom > 0}
            <div class="flex items-center gap-3 text-xs text-ink-500" role="separator">
              <span class="h-px flex-1 bg-ink-800"></span>
              <span class="flex items-center gap-1.5"><Icon name="layers" class="h-3.5 w-3.5" />Earlier messages are not sent to the model</span>
              <button type="button" class="rounded px-1.5 py-0.5 text-ember-300 underline underline-offset-2 hover:text-ember-200" onclick={() => (compactedBefore = 0)}>Send them again</button>
              <span class="h-px flex-1 bg-ink-800"></span>
            </div>
          {/if}
          <MessageBubble
            role={msg.role}
            content={msg.content}
            images={msg.images}
            ts={msg.ts}
            streaming={streaming && idx === messages.length - 1}
            citations={msg.citations}
            steps={msg.steps}
            parts={msg.parts}
            attachments={msg.attachments}
            model={msg.model}
            approvals={idx === messages.length - 1 ? approvals : {}}
            onDecide={(toolId, decision) => void decideTool(toolId, decision)}
            stats={flag("answer_stats") ? msg.stats ?? null : null}
            allowRaw={flag("raw_messages")}
            isLast={idx === messages.length - 1}
            onRetry={msg.role === "assistant" && idx === messages.length - 1 ? retryLast : undefined}
            onEdit={msg.role === "user" ? (newText) => editTurn(newText, idx) : undefined}
            onSendToOpenCode={moduleOn("opencode") ? () => void sendToOpenCode(idx) : undefined}
          />
        {/each}
      </div>
    {/if}
  </div>

  {#if userScrolledUp}
    <button
      type="button"
      onclick={() => scrollToBottom(true)}
      aria-label="Scroll to the newest message"
      in:fly={{ y: 8, duration: dur(motion.base) }}
      out:fade={{ duration: dur(100) }}
      class="absolute bottom-40 left-1/2 z-10 grid h-9 w-9 -translate-x-1/2 place-items-center rounded-full border border-edge bg-ink-800/95 text-ink-100 shadow-xl transition-colors hover:bg-ink-700"
    ><Icon name="arrow-down" class="h-4 w-4" /></button>
  {/if}

  <div class="shrink-0 bg-gradient-to-t from-ink-900 via-ink-900 to-ink-900/0 px-3 pb-3 pt-2 md:px-6">
    <div class={`mx-auto ${chatWidth}`}>
      {#if chatError}
        <div class="mb-2"><ErrorBanner message={chatError} onRetry={sendFailed && !streaming ? retryLast : undefined} onDismiss={() => { chatError = null; sendFailed = false; }} /></div>
      {/if}
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
        skills={moduleOn("skills") ? skills : []}
        onPickSkill={(body) => { input = body; }}
        onSlashCommand={handleSlashCommand}
        {mode}
        modes={chatModes()}
        onModeChange={changeMode}
        {useWeb}
        {webAvailable}
        onToggleWeb={() => useWeb = !useWeb}
        {useRecall}
        {recallAvailable}
        onToggleRecall={() => useRecall = !useRecall}
        attachments={pendingFiles}
        {attaching}
        onAttachFiles={() => void attachFiles()}
        onRemoveAttachment={(i) => pendingFiles.splice(i, 1)}
        {useFiles}
        onToggleFiles={() => useFiles = !useFiles}
        {useSkills}
        skillsAvailable={moduleOn("skills")}
        onToggleSkills={() => useSkills = !useSkills}
        mcpServers={mcpList.map((m) => ({ id: m.id, name: m.name, on: !mcpOff.includes(m.id) }))}
        mcpAvailable={moduleOn("mcp")}
        onToggleMcp={toggleMcp}
        onManageTools={() => requestSettings("tools")}
      >
        {#snippet trailing()}
          <ContextMeter {usage} {streaming} {autoCompactAt} compacted={historyFrom > 0} onCompact={() => handleSlashCommand("compact")} />
          {#if activeChoice && activeChoice.efforts.length > 0}
            <EffortPicker efforts={activeChoice.efforts} value={effort} onChange={setEffort} />
          {/if}
          <ModelPicker
            {choices}
            bind:open={pickerOpen}
            busy={pickerBusy || engineLoading}
            onSelect={(m) => void selectModel(m)}
            onToggleFavorite={(key) => void toggleFavorite(key)}
            onRefresh={(id) => void refreshProvider(id)}
            refreshing={refreshingProvider}
            onManageProviders={moduleOn("online") ? () => requestSettings("online") : undefined}
            onGetModels={onOpenModels}
          />
        {/snippet}
      </ChatInput>
      <p class="mt-2 text-center text-xs text-ink-500">LocalAide can make mistakes. Check important facts{onlineChoice ? `. Messages go to ${onlineChoice.provider_label}` : ""}.</p>
    </div>
  </div>

  {#if dragging}
    <div class="pointer-events-none absolute inset-3 z-40 grid place-items-center rounded-2xl border-2 border-dashed border-ember-500/60 bg-ink-950/80" in:fade={{ duration: dur(motion.fast) }} out:fade={{ duration: dur(motion.fast) }}>
      <div class="flex flex-col items-center gap-2 text-center">
        <Icon name="paperclip" class="h-8 w-8 text-ember-300" />
        <p class="text-base font-semibold text-ink-50">Drop files to attach them</p>
        <p class="text-sm text-ink-400">Text, code, PDF and images. The model reads them with your message.</p>
      </div>
    </div>
  {/if}

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

<style>
  .chat-icon-btn {
    display: grid;
    height: 2.25rem;
    width: 2.25rem;
    place-items: center;
    border-radius: 0.6rem;
    color: var(--color-ink-400);
    transition: background-color 150ms ease-out, color 150ms ease-out;
  }
  .chat-icon-btn:hover,
  .chat-icon-btn[aria-pressed="true"] {
    background: color-mix(in oklab, var(--color-veil) 6%, transparent);
    color: var(--color-ink-50);
  }
  .menu-item {
    display: flex;
    width: 100%;
    align-items: center;
    gap: 0.6rem;
    border-radius: 0.5rem;
    padding: 0.5rem 0.65rem;
    text-align: left;
    font-size: 0.875rem;
    color: var(--color-ink-200);
  }
  .menu-item:hover,
  .menu-item:focus {
    background: color-mix(in oklab, var(--color-veil) 6%, transparent);
    outline: none;
  }
  .menu-item:disabled {
    opacity: 0.4;
  }
</style>
