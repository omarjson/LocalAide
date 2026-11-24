<script lang="ts">
  import { onDestroy, onMount, tick, untrack } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { api, errorMessage } from "../lib/api/client";
  import { addQuestion, addSubagentStep, appendReasoning, appendText, effortLabel, finishTool, partsForSaving, setSkillChange, setTodos, setToolStatus, settleParts, startTool, textOf } from "../lib/chatParts";
  import PanelRail from "../components/PanelRail.svelte";
  import { chats, loadThreads as loadSavedChats } from "../lib/chats.svelte";
  import { dur, motion } from "../lib/motion";
  import { chatModes, dev, flag, moduleOn } from "../lib/dev.svelte";
  import { isChatMode, MODE_ICONS } from "../lib/chatModes";
  import { requestSettings } from "../lib/nav.svelte";
  import { showToast } from "../lib/toast.svelte";
  import { branchCount as branchCountAt, saveBranch, swapBranch } from "../lib/branches.svelte";
  import Slider from "../components/Slider.svelte";
  import ToolApprovalModal from "../components/ToolApprovalModal.svelte";
  import type { ApprovalMode, ChatAttachment, CloudConfigStatus, ChatMessage, ChatModeId, ChatThread, EngineStatusEvent, McpServer, MessagePart, ModelChoice, ModelChoices, Skill, Starter, ToolAction, ToolExecutionResponse } from "../lib/api/bindings";
  import MessageBubble from "../components/MessageBubble.svelte";
  import ChatHeader from "../components/ChatHeader.svelte";
  import ChatEmptyState from "../components/ChatEmptyState.svelte";
  import ChatComposer from "../components/ChatComposer.svelte";
  import Icon from "../components/Icon.svelte";
  import Modal from "../components/Modal.svelte";
  import SubagentTranscript from "../components/SubagentTranscript.svelte";
  import { A2uiChat, provideA2ui } from "../lib/a2ui.svelte";
  import ProviderForm from "../components/ProviderForm.svelte";
  import { calibration, compactionStart, contextSegments, contextUsage, fits, forcedCompactionStart, thinkingReserve, type MeasuredPrompt } from "../lib/contextUsage";
  import { conversationMarkdown, exportFileName } from "../lib/chatExport";
  import { greetingAt, workedToday } from "../lib/greeting";
  import { loadLastUsedModel, resolveNewChatModel, saveLastUsedModel } from "../lib/lastModel";
  import { resolveChoreModel } from "../lib/modelPicker";
  import { cleanTitle, isGoodTitle } from "../lib/chatTitle";
  import { createVirtualizer } from "@tanstack/svelte-virtual";
  import { ChatRunState } from "../lib/chatRun.svelte";

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
  let activeThreadId = $state<string | null>(null);
  let activeThreadTitle = $state<string>("New Conversation");
  // The project folder this chat works in; null for a plain chat in the
  // shared workspace.
  let activeFolder = $state<string | null>(null);

  // The streaming run and the message queue for this tab's chat. One
  // instance per ChatScreen, so two tabs never share a run.
  const run = new ChatRunState();
  let input = $state("");

  let model = $state("Local model");

  // The model picker: local models and the models of every online provider
  // the user switched on. The active one answers.
  let choices = $state<ModelChoices | null>(null);
  // Reading width: Comfortable (default), Wider, or Full. Kept across restarts.
  // The old experimental "wide_chat" flag migrates to Full.
  const WIDTH_KEY = "localaide-chat-width";
  let chatWidthChoice = $state<"cozy" | "wide" | "full">((() => {
    try {
      const saved = localStorage.getItem(WIDTH_KEY);
      if (saved === "cozy" || saved === "wide" || saved === "full") return saved;
      if (flag("wide_chat")) return "full";
    } catch { /* storage unavailable */ }
    return "cozy";
  })());
  const setChatWidth = (choice: "cozy" | "wide" | "full") => {
    chatWidthChoice = choice;
    try { localStorage.setItem(WIDTH_KEY, choice); } catch { /* storage unavailable */ }
  };
  const chatWidth = $derived(
    chatWidthChoice === "cozy" ? "max-w-3xl" : chatWidthChoice === "wide" ? "max-w-5xl" : "max-w-none",
  );

  // Follow answers while streaming: keep the newest text in view unless you
  // scrolled up to read. A switch in Chat settings turns it off.
  const FOLLOW_KEY = "localaide-follow-stream";
  let followStreaming = $state((() => {
    try { return localStorage.getItem(FOLLOW_KEY) !== "off"; } catch { return true; }
  })());
  const toggleFollowStreaming = () => {
    followStreaming = !followStreaming;
    try { localStorage.setItem(FOLLOW_KEY, followStreaming ? "on" : "off"); } catch { /* storage unavailable */ }
    if (followStreaming) void scrollToBottom(true);
  };

  // Collapse questions longer than a few lines until you expand them.
  const COLLAPSE_KEY = "localaide-collapse-questions";
  let collapseQuestions = $state((() => {
    try { return localStorage.getItem(COLLAPSE_KEY) === "on"; } catch { return false; }
  })());
  const toggleCollapseQuestions = () => {
    collapseQuestions = !collapseQuestions;
    try { localStorage.setItem(COLLAPSE_KEY, collapseQuestions ? "on" : "off"); } catch { /* storage unavailable */ }
  };
  let pickerOpen = $state(false);
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
        shell?: unknown;
        approvals?: unknown;
        mcpOff?: unknown;
        efforts?: unknown;
        autoCompactAt?: unknown;
        learnSkills?: unknown;
        suggestNext?: unknown;
        visuals?: unknown;
        subagents?: unknown;
        buildMode?: unknown;
        chatModels?: unknown;
      };
    } catch {
      return {};
    }
  })();

  // Each chat keeps its own model: switching mid-chat keeps the whole
  // thread, and a new chat follows the app-wide default again. Crush-style.
  let chatModels = $state<Record<string, string>>(
    savedOptions.chatModels && typeof savedOptions.chatModels === "object"
      ? (savedOptions.chatModels as Record<string, string>)
      : {},
  );

  // "New chats start on": the model used last, remembered across restarts.
  // A new chat takes it when it is still usable; when it is not, the chat
  // falls back to the active model and says so in one honest line.
  let lastUsedKey = $state<string | null>(loadLastUsedModel());
  let modelNotice = $state<string | null>(null);

  let pickerBusy = $state(false);
  let refreshingProvider = $state<string | null>(null);
  // What a new chat starts on: the last-used model when it is still
  // usable, else the active model. Null for a chat that already has its
  // own model saved.
  const newChatModel = $derived.by(() => {
    if (activeThreadId && chatModels[activeThreadId]) return null;
    if (!choices) return null;
    return resolveNewChatModel(lastUsedKey, choices.models, choices.local_loaded);
  });
  const activeChoice = $derived.by(() => {
    const key = activeThreadId ? chatModels[activeThreadId] : undefined;
    return (
      (key ? choices?.models.find((m) => m.key === key) : null) ??
      (newChatModel?.key ? choices?.models.find((m) => m.key === newChatModel.key) : null) ??
      choices?.models.find((m) => m.active) ??
      null
    );
  });
  // Name the fallback in one line, on the empty chat only — never an
  // error, never a silent switch.
  $effect(() => {
    if (newChatModel?.fellBack && messages.length === 0) {
      modelNotice = activeChoice
        ? `Your last model isn't available, so this chat starts on ${activeChoice.name}.`
        : "Your last model isn't available.";
    } else if (!newChatModel?.fellBack) {
      modelNotice = null;
    }
  });
  const onlineChoice = $derived(activeChoice && activeChoice.provider_id !== null ? activeChoice : null);
  // Whether a local model is loaded in the engine right now.
  let engineLoaded = $state(false);
  let chatError = $state<string | null>(null);
  // True when chatError came from a failed answer, so Retry resends it.
  let sendFailed = $state(false);
  // Read by screen readers once per answer, instead of every streamed token.
  let liveStatus = $state("");
  let showMoreActions = $state(false);
  let modelsChecked = $state(false);

  // What this computer holds, for the empty chat's quiet stats row.
  let localDocs = $state<number | null>(null);
  let localNotes = $state<number | null>(null);
  // Titles of pinned notes, shown as chips above the composer while Recall is on.
  let pinnedTitles = $state<string[]>([]);
  // The chat's title can come from the model, be regenerated, or be edited
  // by hand; the first-line fallback only applies before any of those.
  let aiTitle = $state(false);
  let titleBusy = $state(false);
  let editingTitle = $state(false);
  let titleDraft = $state("");

  const generateTitle = async () => {
    if (run.streaming || titleBusy || messages.length < 2) return;
    const question = messages.find((m) => m.role === "user")?.content ?? "";
    const answer = [...messages].reverse().find((m) => m.role === "assistant" && !m.error)?.content ?? "";
    if (!question.trim()) return;
    titleBusy = true;
    try {
      // Chores run on the loaded local model when there is one (N5):
      // free, and never blocking the chat. Otherwise the chat's model.
      const chore = resolveChoreModel(choices);
      const target = chore ?? (activeChoice ? { provider_id: activeChoice.provider_id, model: activeChoice.model } : undefined);
      // One retry on failure: a transient model or network error should
      // not leave the tab titled with the raw first message.
      let title: string | null = null;
      for (let attempt = 0; attempt < 2; attempt++) {
        try {
          title = await api.chatTitle(question.slice(0, 400), answer.slice(-1200), target);
          break;
        } catch {
          if (attempt === 1) return;
        }
      }
      const clean = cleanTitle(title);
      // A title that is empty or merely echoes the question is dropped,
      // not saved: aiTitle stays false, so the next completed answer
      // tries again (`if (!aiTitle) void generateTitle()` on done).
      // Titles are a nicety; the first-line fallback stays meanwhile.
      if (clean && isGoodTitle(title, question) && clean !== activeThreadTitle) {
        activeThreadTitle = clean;
        aiTitle = true;
        void persistCurrentThread();
      }
    } finally {
      titleBusy = false;
    }
  };

  const startTitleEdit = () => {
    titleDraft = activeThreadTitle === "New Conversation" ? "" : activeThreadTitle;
    editingTitle = true;
  };
  // The title is a button: one click opens the chat menu, a double-click
  // renames. A click waits one beat, so the double-click can cancel it
  // instead of opening the menu under the rename box.
  let titleClickTimer: ReturnType<typeof setTimeout> | undefined;
  const onTitleClick = () => {
    clearTimeout(titleClickTimer);
    titleClickTimer = setTimeout(() => (showMoreActions = true), 250);
  };
  const onTitleDblClick = () => {
    clearTimeout(titleClickTimer);
    startTitleEdit();
  };
  const saveTitleEdit = () => {
    if (!editingTitle) return;
    editingTitle = false;
    const next = titleDraft.trim();
    if (next && next !== activeThreadTitle) {
      activeThreadTitle = next;
      aiTitle = true;
      void persistCurrentThread();
    }
  };
  // The side rail with the live cards. Remembered for the next chats.
  let railOpen = $state((() => {
    try {
      return localStorage.getItem("localaide-rail") !== "off";
    } catch {
      return true;
    }
  })());
  const toggleRail = () => {
    railOpen = !railOpen;
    try {
      localStorage.setItem("localaide-rail", railOpen ? "on" : "off");
    } catch {
      // Storage can be unavailable; the choice still applies now.
    }
  };
  // Sources of the most recent answer that listed any.
  const lastCitations = $derived(
    [...messages].reverse().find((m) => (m.citations?.length ?? 0) > 0)?.citations ?? [],
  );
  const askFromRail = (text: string) => {
    if (run.streaming) return;
    input = text;
    void send();
  };
  const skillFromRail = (name: string) => {
    input = `/${name} `;
    focusComposer();
  };
  let localBudgetGb = $state<number | null>(null);
  // RAM figures for the memory chip under the composer. Null when the
  // probe failed — the chip hides, it never blocks the chat.
  let localRam = $state<{ totalGb: number; availableGb: number } | null>(null);
  const loadLocalStats = async () => {
    try {
      const probeResult = await api.systemProbe();
      localBudgetGb = Math.round(probeResult.budget_gb);
      localRam = { totalGb: probeResult.total_ram_gb, availableGb: probeResult.available_ram_gb };
    } catch {
      localBudgetGb = null;
      localRam = null;
    }
    try {
      localDocs = (await api.getIndexerStats()).doc_count;
    } catch {
      localDocs = null;
    }
    if (moduleOn("memory")) {
      try {
        const memories = await api.listMemories();
        localNotes = memories.length;
        pinnedTitles = memories.filter((n) => n.pinned === true).map((n) => n.title);
      } catch {
        localNotes = null;
      }
    }
  };

  // Files waiting to go with the next message.
  let pendingFiles = $state<ChatAttachment[]>([]);
  let attaching = $state(false);
  let dragging = $state(false);
  // Tool calls waiting for the user's OK, by tool call id.
  let approvals = $state<Record<string, { approval_id: string; preview: string }>>({});
  // Tools the user allowed for the rest of this chat.
  let allowedTools = $state<string[]>([]);
  let mcpList = $state<McpServer[]>([]);

  let {
    onOpenModels,
    active = true,
    startThread = null,
    onThreadStarted,
    onTitleChanged,
    onBusyChanged,
    onNewChatRequested,
  }: {
    onOpenModels?: () => void;
    active?: boolean;
    /** The chat this tab opens with: a saved thread, or a fresh chat. */
    startThread?: { id: string | null; folder: string | null } | null;
    onThreadStarted?: (id: string) => void;
    onTitleChanged?: (title: string) => void;
    onBusyChanged?: (busy: boolean) => void;
    onNewChatRequested?: () => void;
  } = $props();
  // The starting chat is consumed once, on mount; later changes are the
  // tab strip's own bookkeeping and are ignored.
  let startHandled = $state(false);

  let mode = $state<ChatModeId>(isChatMode(savedOptions.mode) ? savedOptions.mode : "standard");
  let useWeb = $state(savedOptions.web === true);
  let useRecall = $state(savedOptions.recall !== false);
  let useFiles = $state(savedOptions.files !== false);
  let useSkills = $state(savedOptions.skills !== false);
  let useShell = $state(savedOptions.shell !== false);
  // The permission icon in the chat box: Plan mode, Ask before changes,
  // Autonomous or Full access. Remembered for every chat.
  const isMode = (v: unknown): v is ApprovalMode => v === "plan" || v === "ask" || v === "smart" || v === "auto";
  let approvalMode = $state<ApprovalMode>(isMode(savedOptions.approvals) ? savedOptions.approvals : "ask");
  // The mode Build switches back to after a plan.
  let buildMode = $state<ApprovalMode>(isMode(savedOptions.buildMode) && savedOptions.buildMode !== "plan" ? savedOptions.buildMode : "ask");
  const setApprovalMode = (next: ApprovalMode) => {
    approvalMode = next;
    if (next !== "plan") buildMode = next;
  };
  let learnSkills = $state(savedOptions.learnSkills !== false);
  let suggestNext = $state(savedOptions.suggestNext !== false);
  let visuals = $state(savedOptions.visuals !== false);
  let useSubagents = $state(savedOptions.subagents !== false);
  // MCP servers switched off in chat (they stay on in Settings).
  let mcpOff = $state<string[]>(Array.isArray(savedOptions.mcpOff) ? savedOptions.mcpOff.filter((v): v is string => typeof v === "string") : []);
  // The thinking effort chosen for each model, by picker key. A saved "auto"
  // counts as unset, so the default below applies.
  let efforts = $state<Record<string, string>>(
    savedOptions.efforts && typeof savedOptions.efforts === "object" ? (savedOptions.efforts as Record<string, string>) : {},
  );
  // Medium is the default rung for a model whose ladder has it; other models
  // keep "auto" (nothing is sent, the template's own default applies).
  const effort = $derived.by(() => {
    if (!activeChoice) return "auto";
    const saved = efforts[activeChoice.key];
    if (saved && saved !== "auto") return saved;
    return activeChoice.efforts.includes("medium") ? "medium" : "auto";
  });
  let autoCompactAt = $state(typeof savedOptions.autoCompactAt === "number" ? Math.max(50, Math.min(95, savedOptions.autoCompactAt)) : 95);
  $effect(() => {
    try {
      localStorage.setItem(
        OPTIONS_KEY,
        JSON.stringify({ mode, web: useWeb, recall: useRecall, files: useFiles, skills: useSkills, shell: useShell, approvals: approvalMode, buildMode, mcpOff, efforts, autoCompactAt, learnSkills, suggestNext, visuals, subagents: useSubagents, chatModels }),
      );
    } catch {
      // Storage can be unavailable; the switches still work for this session.
    }
  });
  const webAvailable = $derived(moduleOn("web_in_chat"));
  const recallAvailable = $derived(moduleOn("memory"));
  const shellAvailable = $derived(moduleOn("terminal"));
  const changeMode = (next: ChatModeId) => {
    mode = next;
    // Autonomous researches on its own, so it turns the web on (the user
    // can switch it off again).
    if (next === "autonomous" && webAvailable) useWeb = true;
  };
  const currentModeInfo = $derived(chatModes().find((m) => m.id === mode));
  let showSettingsPanel = $state(false);

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
    void loadLocalStats();
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
  // Answer statistics for everyone: tokens, time and speed under each
  // answer. A normal switch in Chat settings; developer mode only seeds
  // the default until the user picks — the switch is the control, so a
  // developer flag can never force the stats back on after it is off.
  const STATS_KEY = "localaide-answer-stats";
  let answerStats = $state((() => {
    try {
      return localStorage.getItem(STATS_KEY) === "on";
    } catch {
      return false;
    }
  })());
  const toggleAnswerStats = () => {
    answerStats = !answerStats;
    try {
      localStorage.setItem(STATS_KEY, answerStats ? "on" : "off");
    } catch {
      // Storage can be unavailable; the choice still applies now.
    }
  };
  // Developer settings arrive after this screen mounts. Until the user
  // has chosen, the flag is the default; afterwards their choice stands.
  $effect(() => {
    if (!dev.settings) return;
    try {
      if (localStorage.getItem(STATS_KEY) === null) answerStats = flag("answer_stats");
    } catch {
      // Without storage there is nothing to seed from; the switch stays.
    }
  });
  const showStats = $derived(answerStats);
  let skills = $state<Skill[]>([]);
  let showSystem = $state(false);
  let showTools = $state(false);
  /** The subagent whose work the read-only side view is showing. */
  let transcriptAgent = $state<ToolPart | null>(null);
  let scrollEl = $state<HTMLDivElement>();
  // Virtualized message list: only the visible window (plus overscan) is
  // in the DOM, so a long agent chat does not keep the whole thread
  // rendered. Rows are measured per element (ResizeObserver), so the
  // streaming answer re-measures as it grows. The gap and padding match
  // the old full list (space-y-7 / space-y-4, py-6).
  const messageVirtualizer = createVirtualizer({
    count: 0,
    getScrollElement: () => scrollEl ?? null,
    estimateSize: () => 240,
    overscan: 5,
    gap: 28,
    paddingStart: 24,
    paddingEnd: 24,
  });
  // Keep the virtualizer's count and gap in sync with the thread. The
  // guard stops the store update from re-triggering this effect.
  let virtualCount = -1;
  let virtualGap = -1;
  $effect(() => {
    const count = messages.length;
    const gap = flag("compact") ? 16 : 28;
    if (count !== virtualCount || gap !== virtualGap) {
      virtualCount = count;
      virtualGap = gap;
      $messageVirtualizer.setOptions({ count, gap });
    }
  });
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

  const defaultStarters: { title: string; prompt: string; icon: "clock" | "chat" | "file" | "sliders" }[] = [
    { title: "Plan my week", prompt: "Help me plan my week.", icon: "clock" },
    { title: "Write an email", prompt: "Draft a polite email about a late payment.", icon: "chat" },
    { title: "Summarize", prompt: "Summarize this note in five short points.", icon: "file" },
    { title: "Compare options", prompt: "Compare these two choices and explain the trade-offs.", icon: "sliders" },
  ];
  // Learned from how the user's saved chats begin (on this computer); the
  // defaults fill the rest. Learned starters earn their spot: a phrase long
  // enough to read, no URLs or code, and only the two most frequent — a bare
  // link or a truncated fragment makes the whole empty state look broken.
  const LEARNED_STARTER_MAX = 2;
  let learnedStarters = $state<Starter[]>([]);
  const loadStarters = async () => {
    try {
      learnedStarters = await api.chatStarters();
    } catch {
      learnedStarters = [];
    }
  };
  const usableLearned = $derived(
    learnedStarters
      .filter((s) => {
        const t = s.prompt.trim();
        if (t.length < 12 || t.length > 120) return false;
        if (/https?:\/\/|www\.|\/[^\s]+\/|^\s*\w+\.\w{2,}\s/i.test(t)) return false;
        if (/[{}<>`\\[\]]/.test(t)) return false;
        return true;
      })
      .slice(0, LEARNED_STARTER_MAX),
  );
  const starterPrompts = $derived([
    ...usableLearned.map((s) => ({ title: s.title, prompt: s.prompt, icon: "sparkle" as const, learned: true })),
    ...defaultStarters.filter((d) => !usableLearned.some((l) => l.prompt === d.prompt)).map((d) => ({ ...d, learned: false })),
  ].slice(0, 4));

  // Time-of-day greeting, re-evaluated at the next hour boundary; the
  // evening variant says "nice work today" only when a chat had a turn
  // today (see src/lib/greeting.ts).
  let greetingNow = $state(new Date());
  const greeting = $derived(greetingAt(greetingNow, workedToday(chats.threads, greetingNow)));
  onMount(() => {
    const ms = 3_600_000 - (Date.now() % 3_600_000) + 1000;
    const timer = setTimeout(() => (greetingNow = new Date()), ms);
    return () => clearTimeout(timer);
  });

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

  // After a jump to the bottom of an estimated list, the real row heights
  // land a frame later and can move the bottom; re-pin once they have.
  // Only when the user is still following — never yanks a reader who
  // scrolled up meanwhile.
  const repinAfterMeasure = () => {
    setTimeout(() => {
      if (followStreaming && !userScrolledUp) void scrollToBottom(true);
    }, 300);
  };

  // Follow the answer while it streams: the last message grows without the
  // message count changing, so its length is tracked too. Progress also
  // moves the view: tool calls, subagent steps, to-dos and reasoning change
  // the parts, not the last message's text, so a fingerprint of them is
  // tracked as well. Off means you steer the scroll yourself; the pill
  // below still jumps to the end.
  $effect(() => {
    if (!followStreaming) return;
    const lastLength = messages.at(-1)?.content.length ?? 0;
    const progress = messages
      .map((m) =>
        (m.parts ?? [])
          .map((p) => {
            if (p.type === "tool") return `tool:${p.id}:${p.status}:${p.steps?.length ?? 0}`;
            if (p.type === "todos") return `todos:${p.items.length}`;
            if (p.type === "text" || p.type === "reasoning") return `${p.type}:${p.text.length}`;
            if (p.type === "question") return `q:${p.answer ?? ""}`;
            return "other";
          })
          .join(","),
      )
      .join("|");
    if (messages.length > 0 && lastLength >= 0 && progress.length >= 0) {
      scrollToBottom();
    }
  });

  const loadThreads = () => loadSavedChats();

  const newChat = (folder: string | null = null) => {
    if (run.streaming) return;
    activeThreadId = `t_${Date.now()}`;
    onThreadStarted?.(activeThreadId);
    activeThreadTitle = "New Conversation";
    aiTitle = false;
    editingTitle = false;
    activeFolder = folder;
    messages = [];
    pendingFiles = [];
    allowedTools = [];
    approvals = {};
    compactedBefore = 0;
    compactionSummary = "";
    run.stoppedAnswer = false;
    resetMeasurements();
    suggestions = [];
    void loadStarters();
  };

  // Each tab's chat screen owns one chat. Only the active tab speaks for
  // the app (sidebar highlight, global busy), and its title and id flow
  // back up so the tab strip can show them.
  $effect(() => {
    if (!active) return;
    chats.activeId = messages.length > 0 ? activeThreadId : null;
    chats.activeFolder = activeFolder;
    chats.busy = run.streaming;
  });
  $effect(() => {
    onBusyChanged?.(run.streaming);
  });
  $effect(() => {
    onTitleChanged?.(activeThreadTitle);
  });

  // The tab's starting chat, set once by the tab strip when this screen
  // was created: either a saved thread or a fresh chat.
  $effect(() => {
    const start = startThread;
    if (!start || startHandled) return;
    startHandled = true;
    if (start.id) {
      void loadThreadId(start.id, start.folder);
    } else {
      activeFolder = start.folder;
    }
  });

  const loadThreadId = async (id: string, folder: string | null) => {
    activeThreadId = id;
    activeThreadTitle = chats.threads.find((t) => t.id === id)?.title ?? "Chat";
    aiTitle = true;
    editingTitle = false;
    activeFolder = folder;
    suggestions = [];
    allowedTools = [];
    approvals = {};
    compactedBefore = 0;
    compactionSummary = "";
    run.stoppedAnswer = false;
    resetMeasurements();
    void refreshLedgerCacheRate();
    systemPrompt = chats.threads.find((t) => t.id === id)?.system_prompt || "You are a helpful local AI assistant.";
    try {
      messages = await api.loadChatThread(id);
    } catch (e: unknown) {
      messages = [];
      chatError = `That chat could not load: ${errorMessage(e)}`;
    }
    // The thread list may have landed after this tab started opening.
    const saved = chats.threads.find((t) => t.id === id);
    if (saved) {
      activeThreadTitle = saved.title;
      systemPrompt = saved.system_prompt || "You are a helpful local AI assistant.";
    }
    scrollToBottom(true);
    repinAfterMeasure();
  };

  const persistCurrentThread = async () => {
    if (!activeThreadId) {
      activeThreadId = `t_${Date.now()}`;
      onThreadStarted?.(activeThreadId);
    }
    if (messages.length === 0) return;

    if (!aiTitle && activeThreadTitle === "New Conversation" && messages[0]) {
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
      await api.saveChatThread(activeThreadId, activeThreadTitle, systemPrompt, textOnly, activeFolder);
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
  // Anchored summary (N4): when auto-compact cuts the history, the cut
  // part is summarised — goal, decisions, progress, next steps — and the
  // summary rides on the system prompt of later sends, so the model keeps
  // the thread. It is session-only, like the cut itself: the full history
  // stays on screen and in the saved chat, nothing is deleted. Empty
  // when nothing was ever auto-compacted (or summarising failed).
  let compactionSummary = $state("");

  /**
   * Summarise what auto-compact just cut, folding in the previous
   * summary so the anchor covers the whole cut span. Fire-and-forget:
   * the summary runs on the loaded local model for free
   * (`summarize`); when no local model is loaded, or it errors, the
   * summary stays as it was and the plain cut still stands.
   */
  const requestCompactionSummary = (cut: ChatMessage[]) => {
    const lines: string[] = [];
    if (compactionSummary) lines.push(`Previous summary:\n${compactionSummary}`);
    const rendered = cut
      .filter((m) => m.content.trim() && !m.steer)
      .map((m) => `${m.role}: ${m.content.slice(0, 2000)}`)
      .join("\n");
    if (!rendered && !compactionSummary) return;
    const text = [
      "Summarize the conversation below for the AI that will continue it. Cover: the goal, decisions made, progress so far, and next steps. A few short lines, no preamble.",
      rendered,
    ]
      .filter(Boolean)
      .join("\n\n")
      .slice(0, 8000);
    void api
      .summarize({ text })
      .then((res) => {
        if (res.summary.trim()) compactionSummary = res.summary.trim();
      })
      .catch(() => {
        // Summarising is a nicety; the cut stands without it.
      });
  };
  // The same thinking room the backend keeps for this effort.
  const answerReserve = $derived(maxTokens + (activeChoice?.reasoning ? thinkingReserve(effort) : 0));
  // The engine's own count for the first request of the last answer, and
  // where in the chat it was taken. The meter starts from it and estimates
  // only what came after. Cleared with each new or loaded chat.
  let measuredPrompt = $state<MeasuredPrompt | null>(null);
  // The model the count came from: another model tokenizes differently.
  let measuredModel = $state<string | null>(null);
  // How far the text estimate was off last time, for when there is no
  // fresh count (after a compaction, or for a model that reports none).
  let estimateRatio = $state(1);
  // Instructions and tool schemas as the backend counted them last time.
  let reportedSystem = $state<number | null>(null);
  let reportedTools = $state<number | null>(null);
  // Cache hit rate of the last answer, when the provider reports cached tokens.
  let lastCacheRate = $state<number | null>(null);
  // The same rate averaged over the usage log, for models that never
  // report cached tokens (the local engine on older builds): the meter
  // shows this when the last answer has no rate of its own.
  let ledgerCacheRate = $state<number | null>(null);
  const refreshLedgerCacheRate = async () => {
    try {
      const summary = await api.usageSummary();
      ledgerCacheRate = summary.input_tokens > 0 && summary.cache_tokens > 0 ? (summary.cache_tokens / summary.input_tokens) * 100 : null;
    } catch {
      // The meter keeps the last answer's rate, or hides the row.
    }
  };
  // A2UI forms in this chat's answers: built once per finished block, in
  // chat order, so a later answer can update a form an earlier one drew.
  const a2ui = provideA2ui(new A2uiChat());
  $effect(() => {
    const texts = messages.filter((m) => m.role === "assistant").map((m) => m.content);
    untrack(() => a2ui.sync(texts));
  });
  // A pressed form button is the user's next message; while an answer
  // runs it waits in the queue.
  const sendA2uiAction = (message: string) => {
    if (run.streaming) run.queueNote(message);
    else void send(false, message);
  };

  const resetMeasurements = () => {
    measuredPrompt = null;
    measuredModel = null;
    lastCacheRate = null;
  };
  // Steering notes show in the transcript but are never sent to the model,
  // so they do not count toward the window, the meter or the fit checks.
  // (Compaction itself keeps the real array: its cut is an index into it.)
  const sentMessages = $derived(messages.filter((m) => !m.steer));
  // A stopped answer waits for Resume. Set when the user stops it (or the
  // run reports cancelled); any new send clears it. Lives in `run`.

  const usageOptions = (draft: string) => {
    const sameModel = measuredModel !== null && measuredModel === (activeChoice?.key ?? null);
    return {
      from: historyFrom,
      draft,
      systemPrompt,
      reserve: answerReserve,
      overhead: reportedSystem !== null ? reportedSystem + (reportedTools ?? 0) : null,
      measured: sameModel ? measuredPrompt : null,
      ratio: sameModel ? estimateRatio : 1,
    };
  };
  const usage = $derived(contextUsage(sentMessages, activeChoice?.context, usageOptions(input)));
  // What the window is made of, for the hover breakdown: history, tool
  // schemas, meta context, skills and instructions, scaled to the meter.
  const segments = $derived(
    contextSegments(messages, {
      from: historyFrom,
      draft: input,
      systemPrompt,
      tools: allowedTools.length,
      mcpServers: mcpList.length,
      pinnedNotes: pinnedTitles,
      skills,
      skillsOn: useSkills && moduleOn("skills"),
      systemTokens: reportedSystem,
      toolTokens: reportedTools,
      total: usage.used,
    }),
  );

  /** Move the cut so the rest fits under `threshold` percent. True when it moved. */
  const compactConversation = (threshold = autoCompactAt, draft = "", force = false) => {
    const limit = activeChoice?.context;
    if (!limit) {
      // No known window: keep the last four messages.
      const next = Math.max(historyFrom, messages.length - 4);
      if (next <= historyFrom) return false;
      compactedBefore = next;
      return true;
    }
    if (force) {
      // An explicit compact always cuts, even when the conversation already
      // fits: keep only the last exchange in what is sent.
      const next = forcedCompactionStart(messages, historyFrom);
      if (next <= historyFrom) return false;
      compactedBefore = next;
      return true;
    }
    const next = compactionStart(messages, limit, threshold, usageOptions(draft));
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
        // A manual compact always cuts, even when the conversation already
        // fits the window: it aims for half the window, leaving room to talk.
        if (compactConversation(Math.min(50, autoCompactAt), "", true)) {
          showToast("Older messages stay on screen but are no longer sent to the model.");
        } else {
          showToast("Nothing left to compact — only the last exchange is being sent.");
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

  // A tapped answer is sent as the user's reply. An earlier question the
  // user moved past can still be answered; it goes as a new message.
  const findQuestion = (questionId: string) => {
    for (let i = messages.length - 1; i >= 0; i--) {
      const part = messages[i].parts?.find((p) => p.type === "question" && p.id === questionId);
      if (part && part.type === "question") return part;
    }
    return undefined;
  };
  const answerQuestion = (text: string, questionId: string | null) => {
    if (run.streaming || !text.trim()) return;
    const part = questionId ? findQuestion(questionId) : undefined;
    if (part) part.answer = text;
    input = text;
    void send();
  };
  const dismissQuestion = (questionId: string) => {
    const part = findQuestion(questionId);
    if (part) {
      part.dismissed = true;
      void persistCurrentThread();
    }
  };
  // Undo a skill the model saved or improved on its own.
  const undoSkill = async (toolId: string) => {
    for (const m of messages) {
      const part = m.parts?.find((p) => p.type === "tool" && p.id === toolId);
      if (part?.type === "tool" && part.skill && !part.skill.undone) {
        try {
          const name = await api.skillsUndo(part.skill.change_id);
          part.skill.undone = true;
          showToast(`Undone: “${name}” is as it was before.`);
          void persistCurrentThread();
          skills = await api.skillsList().catch(() => skills);
        } catch (e: unknown) {
          showToast(`The skill could not be undone: ${errorMessage(e)}`, "error");
        }
        return;
      }
    }
  };

  // Suggested next messages, after an answer (Settings: chat settings).
  let suggestions = $state<string[]>([]);
  let suggestTicket = 0;
  const askSuggestion = async (question: string, answer: string) => {
    const ticket = ++suggestTicket;
    try {
      // Same chore rule as titles (N5): the loaded local model first.
      const chore = resolveChoreModel(choices);
      const next = await api.suggestNext(question, answer, chore ?? (activeChoice ? { provider_id: activeChoice.provider_id, model: activeChoice.model } : undefined));
      if (ticket === suggestTicket && !run.streaming && !input.trim()) suggestions = next;
    } catch {
      // A suggestion is a nicety; nothing to report.
    }
  };

  // Rewrite the draft into a clearer prompt with the chat's model.
  let enhancing = $state(false);
  const enhancePrompt = async () => {
    const draft = input.trim();
    if (!draft || enhancing) return;
    enhancing = true;
    try {
      const better = await api.enhancePrompt(draft, activeChoice ? { provider_id: activeChoice.provider_id, model: activeChoice.model } : undefined);
      if (input.trim() === draft) {
        previousDraft = draft;
        input = better;
        showToast("Prompt improved. Press Ctrl+Z in the box, or Undo here, to go back.");
      }
    } catch (e: unknown) {
      showToast(`The prompt could not be improved: ${errorMessage(e)}`, "error");
    } finally {
      enhancing = false;
    }
  };
  let previousDraft = $state<string | null>(null);

  // A finished plan (Plan mode) can be carried out with one press.
  let planReady = $state(false);
  const buildPlan = () => {
    planReady = false;
    setApprovalMode(buildMode === "plan" ? "ask" : buildMode);
    input = "Go ahead and carry out the plan above.";
    void send();
  };
  const focusComposer = () => {
    document.querySelector<HTMLTextAreaElement>(".composer textarea")?.focus();
  };

  // Messages written while an answer runs. A queued one is sent when the
  // answer ends; a steering note goes to the running answer and the model
  // reads it before its next step. A note the answer had no step left for
  // joins the queue, so nothing typed is lost. The state lives in `run`
  // (src/lib/chatRun.svelte.ts); the send loop below runs on it.
  const queueDraft = () => {
    const text = input.trim();
    if (!text) return;
    run.queueNote(text);
    input = "";
    showToast("Queued — it sends when this answer ends.");
  };
  const steerText = async (text: string) => {
    const job = run.activeJobId;
    if (!job || !run.streaming) {
      run.queueNote(text);
      return;
    }
    // Listed first, so a "steered" event that arrives before the reply
    // still finds it.
    const note = run.steerNote(text);
    // The note also lands in the transcript, in full, before the answer it
    // steers — so the chat shows what was asked, not just the model's
    // truncated "Read your note" step. It is never sent back as history.
    // (Safe to insert mid-stream: the running answer is held by object in
    // send(), not by index, so this does not shift what it appends to.)
    let aiIdx = -1;
    for (let i = messages.length - 1; i >= 0; i--) {
      if (messages[i].role === "assistant") {
        aiIdx = i;
        break;
      }
    }
    const steerMsg = { role: "user", content: text, ts: Date.now(), steer: true };
    if (aiIdx >= 0) messages.splice(aiIdx, 0, steerMsg);
    else messages.push(steerMsg);
    let taken = false;
    try {
      taken = await api.steerChat(job, text);
    } catch (e: unknown) {
      console.warn("The note could not reach the answer", e);
    }
    if (!taken) {
      if (run.moveToQueue(note.id)) {
        showToast("The answer had already finished — your note is run.queued as the next message.");
      }
    }
  };
  const steerDraft = () => {
    const text = input.trim();
    if (!text) return;
    input = "";
    void steerText(text);
  };
  const sendNextQueued = () => {
    if (run.streaming) return;
    const next = run.shiftNext();
    if (next) void send(false, next.text);
  };
  const sendQueuedNow = (id: number) => {
    const note = run.takeNote(id);
    if (!note) return;
    if (run.streaming) void steerText(note.text);
    else void send(false, note.text);
  };


  // `queuedText` sends a queued message: its text only, leaving the draft
  // and any files waiting in the box alone.
  // The first parameter is a strict boolean on purpose: passing this
  // function bare as a click handler once fed it a MouseEvent, which is
  // truthy, and every Send click resent the previous message instead of
  // the text just typed. Only an explicit `true` resends.
  const send = async (resendArg = false, queuedText?: string, resumeArg = false) => {
    const resend = resendArg === true;
    const resume = resumeArg === true;
    const fromQueue = !resend && !resume && queuedText !== undefined;
    const lastUser = resend ? messages.at(-1) : undefined;
    const files = resend
      ? (lastUser?.attachments ?? []).filter((f) => !f.error)
      : fromQueue
        ? []
        : $state.snapshot(pendingFiles).filter((f) => !f.error);
    const userMsg = resume ? "" : resend ? (lastUser?.content ?? "") : fromQueue ? queuedText.trim() : input.trim();
    const resendImages = resend ? (lastUser?.images ?? []) : [];
    if (run.streaming) return;
    if (!resume && !userMsg && (resend || fromQueue ? resendImages.length === 0 : pendingImages.length === 0 && files.length === 0)) return;
    if (!resend && !fromQueue && !resume) {
      input = "";
    }
    suggestions = [];
    previousDraft = null;
    planReady = false;
    lastCacheRate = null;
    run.stoppedAnswer = false;
    const sentInMode = approvalMode;
    // Questions left open count as moved past; they stay answerable.
    for (const m of messages) {
      for (const p of m.parts ?? []) {
        if (p.type === "question" && !p.answer && !p.dismissed) p.deferred = true;
      }
    }
    if (activeChoice?.context && !fits(contextUsage(sentMessages, activeChoice.context, usageOptions(userMsg)), autoCompactAt)) {
      const cutFrom = historyFrom;
      if (compactConversation(autoCompactAt, userMsg)) {
        requestCompactionSummary($state.snapshot(messages).slice(cutFrom, historyFrom));
        showToast(`The chat reached ${autoCompactAt}% of the context window, so older messages are no longer sent.`);
      }
    }

    // Earlier turns only. The new message is sent separately as `message`.
    // Failed answers are left out, so the model does not repeat them.
    // A resend keeps the question in the transcript, so it is excluded here.
    const priorMessages = resend ? messages.slice(0, -1) : messages;
    const history = $state
      .snapshot(priorMessages)
      .slice(historyFrom)
      .filter((m) => m.content.trim() && !m.error && !m.steer && !m.content.startsWith("[Chat Error"))
      .map((m) => ({
        role: m.role,
        content: m.content,
        attachments: m.attachments?.filter((a) => a.kind === "text").map((a) => ({ ...a, data_url: null })),
        // Tool parts ride along so the backend can prune old tool
        // outputs instead of dropping them all unseen (N4).
        parts: m.parts,
      }));

    const images = resend ? resendImages : fromQueue ? [] : $state.snapshot(pendingImages);
    if (!resend && !resume) {
      if (!fromQueue) {
        pendingImages = [];
        pendingFiles = [];
      }
      messages.push({ role: "user", content: userMsg, ts: Date.now(), images, attachments: files.length ? files : undefined });
    }
    run.streaming = true;
    run.runStartedAt = Date.now();
    sendFailed = false;
    liveStatus = resume ? "Continuing the answer." : "LocalAide is answering.";
    userScrolledUp = false;
    void savePrefs();

    // Where the history this request carries starts, for the engine's count.
    const sentFrom = historyFrom;
    const sentModel = activeChoice?.key ?? null;
    // A resume keeps the partial answer in place and appends to it; a
    // fresh send adds a new empty answer.
    const assistantIdx = resume ? messages.length - 1 : messages.length;
    if (!resume) {
      messages.push({
        role: "assistant",
        content: "",
        ts: Date.now(),
        steps: [],
        parts: [],
        model: activeChoice?.name ?? undefined,
      });
    }
    const started = performance.now();
    let firstTokenAt: number | null = null;
    // The anchored summary rides on the system prompt, not the history:
    // the context meter counts history by message index, and the summary
    // is not a message. It is only sent while part of the history is cut.
    const requestSystemPrompt =
      historyFrom > 0 && compactionSummary
        ? `${systemPrompt}\n\n[Summary of the earlier part of this conversation — those messages are no longer sent in full: ${compactionSummary}]`
        : systemPrompt;
    // Captured by object, not index: a steering note inserted before this
    // answer must not shift what the stream appends to.
    const assistantMsg = messages[assistantIdx];
    const answer = () => assistantMsg;
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
      run.streaming = false;
      run.activeJobId = null;
      approvals = {};
      liveStatus = "LocalAide could not answer.";
      run.parkSteers();
      void persistCurrentThread();
    };

    try {
      run.activeJobId = await api.streamChat(
        {
          message: userMsg,
          resume,
          history,
          images,
          model,
          thread_id: activeThreadId,
          system_prompt: requestSystemPrompt,
          temperature,
          max_tokens: maxTokens,
          mode,
          use_web: webAvailable && useWeb,
          use_recall: recallAvailable && useRecall,
          use_files: useFiles,
          use_skills: moduleOn("skills") && useSkills,
          mcp_servers: mcpList.filter((m) => !mcpOff.includes(m.id)).map((m) => m.id),
          allowed_tools: [...allowedTools],
          use_shell: shellAvailable && useShell,
          approval_mode: approvalMode,
          target: activeChoice ? { provider_id: activeChoice.provider_id, model: activeChoice.model } : undefined,
          effort,
          attachments: files,
          now: nowLabel(),
          folder: activeFolder,
          learn_skills: learnSkills,
          use_subagents: useSubagents,
          rich_output: visuals,
        },
        (event) => {
          const now = performance.now();
          if (event.type === "token") {
            if (firstTokenAt === null) firstTokenAt = now;
            appendText(parts(), event.payload.token, now);
            answer().content = textOf(parts());
          } else if (event.type === "steered") {
            const text = event.payload.text;
            const at = run.steering.findIndex((n) => n.text === text);
            if (at >= 0) run.steering.splice(at, 1);
            const short = text.length > 80 ? `${text.slice(0, 80)}…` : text;
            answer().steps = [...(answer().steps ?? []), `Read your note: “${short}”`];
          } else if (event.type === "reasoning") {
            if (firstTokenAt === null) firstTokenAt = now;
            appendReasoning(parts(), event.payload.text, now);
          } else if (event.type === "tool_call") {
            startTool(parts(), event.payload, now);
          } else if (event.type === "tool_approval") {
            setToolStatus(parts(), event.payload.id, "waiting");
            approvals[event.payload.id] = { approval_id: event.payload.approval_id, preview: event.payload.preview };
            liveStatus = `LocalAide asks before it can ${event.payload.title.toLowerCase()}.`;
          } else if (event.type === "question") {
            addQuestion(parts(), event.payload, now);
            liveStatus = `LocalAide asks: ${event.payload.question}`;
          } else if (event.type === "tool_result") {
            finishTool(parts(), event.payload);
            delete approvals[event.payload.id];
          } else if (event.type === "todos") {
            setTodos(parts(), event.payload.items);
          } else if (event.type === "subagent_step") {
            addSubagentStep(parts(), event.payload.id, event.payload.step, event.payload.tokens ?? null);
          } else if (event.type === "skill_change") {
            setSkillChange(parts(), event.payload);
            void api.skillsList().then((list) => (skills = list)).catch(() => undefined);
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
            lastCacheRate = event.payload.prompt_tokens && event.payload.cache_tokens
              ? Math.min(100, (event.payload.cache_tokens / event.payload.prompt_tokens) * 100)
              : null;
            void refreshLedgerCacheRate();
            if (typeof event.payload.system_tokens === "number" && event.payload.system_tokens > 0) {
              reportedSystem = event.payload.system_tokens;
              reportedTools = event.payload.tool_tokens ?? 0;
            }
            // The question as sent, estimated the way the meter estimates,
            // against the engine's own count of it. The answer's index is
            // read fresh: a steering note inserted mid-stream moves it.
            const answerIdx = messages.indexOf(answer());
            const sentEstimate = contextUsage(messages.slice(0, answerIdx).filter((m) => !m.steer), null, {
              from: sentFrom,
              systemPrompt,
              overhead: reportedSystem !== null ? reportedSystem + (reportedTools ?? 0) : null,
            }).used;
            const first = event.payload.first_prompt_tokens ?? null;
            if (first && first > 0 && answerIdx > 0) {
              measuredPrompt = { tokens: first, upTo: answerIdx, from: sentFrom, anchor: messages[answerIdx - 1]?.ts };
              measuredModel = sentModel;
              estimateRatio = calibration(first, sentEstimate);
            } else if (sentModel !== measuredModel) {
              measuredPrompt = null;
              measuredModel = null;
            }
            answer().stats = {
              tokens: event.payload.total_tokens,
              inputTokens: event.payload.prompt_tokens ?? sentEstimate,
              outputTokens: event.payload.total_tokens,
              ms: now - started,
              firstTokenMs: firstTokenAt === null ? null : firstTokenAt - started,
            };
            run.streaming = false;
            run.activeJobId = null;
            approvals = {};
            liveStatus = "Answer ready.";
            if (event.payload.finish_reason === "cancelled") {
              // A stop lands here too (the run reports back after the
              // cancel): the partial answer waits for Resume.
              run.stoppedAnswer = true;
              liveStatus = "Answer stopped.";
            }
            run.parkSteers();
            // The next queued message goes once this answer is saved, unless
            // the user stopped it or the model is waiting on a question.
            const finish = event.payload.finish_reason;
            if (run.queued.length > 0 && finish !== "cancelled" && finish !== "question") {
              void tick().then(() => sendNextQueued());
            }
            if (!aiTitle) void generateTitle();
            void persistCurrentThread().then(() => {
              if (messages.length <= 2) void loadStarters();
            });
            if (sentInMode === "plan" && answer().content.trim() && event.payload.finish_reason !== "cancelled") planReady = true;
            if (suggestNext && event.payload.finish_reason !== "question" && event.payload.finish_reason !== "cancelled" && answer().content.trim()) {
              void askSuggestion(userMsg, answer().content);
            }
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
    if (!run.streaming) return;
    if (run.activeJobId) {
      try {
        await api.cancelChat(run.activeJobId);
      } catch (e: unknown) {
        chatError = `The response could not stop: ${errorMessage(e)}`;
      }
    }
    run.streaming = false;
    run.activeJobId = null;
    approvals = {};
    const last = messages.at(-1);
    if (last?.role === "assistant" && last.parts) settleParts(last.parts, performance.now());
    liveStatus = "Answer stopped.";
    run.stoppedAnswer = true;
    persistCurrentThread();
  };

  // Continue a stopped answer where it left off: the partial answer stays
  // and the model is told to pick it up, instead of starting over.
  const resumeGenerating = () => {
    if (run.streaming) return;
    const last = messages.at(-1);
    if (last?.role !== "assistant" || !last.content.trim()) return;
    void send(false, undefined, true);
  };

  // Answer the last question again, in place: the question stays in the
  // transcript with its attachments and only the old answer is replaced,
  // so the chat continues instead of starting over.
  const retryLast = () => {
    if (run.streaming) return;
    let userIdx = messages.length - 1;
    while (userIdx >= 0 && messages[userIdx].role !== "user") userIdx -= 1;
    if (userIdx < 0) return;
    chatError = null;
    sendFailed = false;
    messages = messages.slice(0, userIdx + 1);
    void send(true);
  };

  const editTurn = (newText: string, index: number) => {
    if (run.streaming) return;
    // The discarded future is parked as a branch, not thrown away.
    if (activeThreadId && messages.length > index + 1) {
      saveBranch(activeThreadId, index, messages.slice(index + 1));
    }
    messages = messages.slice(0, index);
    input = newText;
    send();
  };

  const restoreBranch = (index: number) => {
    if (run.streaming || !activeThreadId) return;
    const tail = swapBranch(activeThreadId, index, messages.slice(index + 1));
    if (tail && tail.length > 0) {
      messages = [...messages.slice(0, index + 1), ...tail];
      void persistCurrentThread();
      scrollToBottom(true);
      repinAfterMeasure();
    }
  };

  const summarizeLast = async () => {
    const last = messages.filter(m => m.role === "assistant").at(-1);
    if (!last || run.streaming) return;
    run.streaming = true;
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
      run.streaming = false;
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

  // Make the chosen model the one that answers. A local model loads first.
  const selectModel = async (choice: ModelChoice) => {
    if (activeThreadId && chatModels[activeThreadId] === choice.key) return;
    pickerBusy = true;
    try {
      choices = await api.setChatTarget({ provider_id: choice.provider_id, model: choice.model });
      engineLoaded = choices.local_loaded;
      // The choice belongs to this chat from now on.
      if (activeThreadId) chatModels = { ...chatModels, [activeThreadId]: choice.key };
      // And new chats start on it from now on.
      lastUsedKey = choice.key;
      saveLastUsedModel(choice.key);
      modelNotice = null;
    } catch (e: unknown) {
      sendFailed = false;
      chatError = `${choice.name} could not be used: ${errorMessage(e)}`;
    } finally {
      pickerBusy = false;
    }
  };

  /** Unload the local model from memory (the chip under the composer). */
  const unloadLocalModel = async () => {
    try {
      await api.unloadInferenceModel();
      engineLoaded = false;
    } catch (e: unknown) {
      chatError = `The model could not be unloaded: ${errorMessage(e)}`;
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

  // Adding an online provider from the chat: the same form as Settings,
  // in a dialog, and the chat switches to the new provider's model once it
  // is saved, so there is no trip through Settings.
  let addingProvider = $state(false);
  let providersBefore: string[] = [];
  const openAddProvider = async () => {
    try {
      providersBefore = (await api.getCloudConfig()).providers.map((p) => p.id);
    } catch {
      providersBefore = [];
    }
    addingProvider = true;
  };
  const providerSaved = async (status: CloudConfigStatus) => {
    addingProvider = false;
    const added = status.providers.find((p) => !providersBefore.includes(p.id)) ?? null;
    await loadChoices();
    if (!added) return;
    const pick =
      choices?.models.find((m) => m.provider_id === added.id && m.model === added.model) ??
      choices?.models.find((m) => m.provider_id === added.id) ??
      null;
    if (pick) {
      await selectModel(pick);
      showToast(`${added.label} is ready. This chat now uses ${pick.name}; switch back any time in the model menu.`);
    } else {
      showToast(`${added.label} is saved. Pick one of its models in the model menu.`);
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
  // Ctrl+T steps through the active model's thinking levels.
  const cycleEffort = () => {
    if (!activeChoice || activeChoice.efforts.length === 0) return;
    const levels = activeChoice.efforts.filter((e) => e !== "auto");
    if (levels.length === 0) return;
    const next = levels[(levels.indexOf(effort) + 1) % levels.length] ?? levels[0];
    setEffort(next);
    showToast(`Thinking effort: ${effortLabel(next)}`);
  };

  // Work status: the newest to-do list, and every subagent in this chat.
  type ToolPart = Extract<MessagePart, { type: "tool" }>;
  const workTodos = $derived.by(() => {
    // Only the newest answer's plan: a new question starts a new task.
    const last = messages.at(-1);
    const t = last?.role === "assistant" ? last.parts?.find((p) => p.type === "todos") : undefined;
    return t?.type === "todos" ? t.items : null;
  });
  const workAgents = $derived(messages.flatMap((m) => (m.parts ?? []).filter((p): p is ToolPart => p.type === "tool" && p.group === "subagent")));
  // The run timeline above the chat box: when this answer started and how
  // many tool rounds it has used, so an autonomous run shows its pace.
  // The start time lives in `run`.
  const runToolRounds = $derived.by(() => {
    const last = messages.at(-1);
    return last?.role === "assistant" ? (last.parts ?? []).filter((p) => p.type === "tool").length : 0;
  });

  // In-chat search (Ctrl+F): find a message, jump between the matches.
  let chatSearchOpen = $state(false);
  let chatSearchQuery = $state("");
  let chatSearchIndex = $state(0);
  let chatSearchInput = $state<HTMLInputElement>();
  const chatMatches = $derived.by(() => {
    const q = chatSearchQuery.trim().toLowerCase();
    if (!q) return [] as number[];
    const hits: number[] = [];
    messages.forEach((m, idx) => {
      const hay = [m.content, ...(m.parts ?? []).map((p) => {
        if (p.type === "text" || p.type === "reasoning") return p.text;
        if (p.type === "tool") return `${p.title} ${p.detail} ${p.output}`;
        return "";
      })].join("\n").toLowerCase();
      if (hay.includes(q)) hits.push(idx);
    });
    return hits;
  });
  const openChatSearch = () => {
    chatSearchOpen = true;
    void tick().then(() => chatSearchInput?.focus());
  };
  const closeChatSearch = () => {
    chatSearchOpen = false;
    chatSearchQuery = "";
    chatSearchIndex = 0;
  };
  const jumpToMatch = (idx: number) => {
    if (!scrollEl) return;
    // The match may not be in the DOM (virtualized): scroll it into view
    // through the virtualizer, then highlight the row once rendered.
    $messageVirtualizer.scrollToIndex(idx, { align: "center", behavior: "auto" });
    void tick().then(() => {
      setTimeout(() => {
        const target = scrollEl?.querySelector<HTMLElement>(`[data-index="${idx}"]`);
        if (!target) return;
        target.style.transition = "outline-color 200ms ease-out";
        target.style.outline = "2px solid color-mix(in oklab, var(--color-ember-500) 65%, transparent)";
        target.style.outlineOffset = "6px";
        target.style.borderRadius = "12px";
        setTimeout(() => {
          target.style.outline = "none";
        }, 1400);
      }, 120);
    });
  };
  const searchKeydown = (e: KeyboardEvent) => {
    const count = chatMatches.length;
    if (e.key === "Escape") {
      e.preventDefault();
      closeChatSearch();
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (count === 0) return;
      chatSearchIndex = e.shiftKey
        ? (chatSearchIndex - 1 + count) % count
        : (chatSearchIndex + 1) % count;
      jumpToMatch(chatMatches[chatSearchIndex]);
    }
  };
  // Ctrl+F (Cmd+F on a Mac) opens the search while the chat screen is up.
  $effect(() => {
    if (!active) return;
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "f") {
        e.preventDefault();
        openChatSearch();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

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
        if (!active) return;
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
    // A fresh chat gets its id now and tells the tab strip; a restored
    // tab gets its id from the thread it opens below.
    if (!startThread?.id) {
      activeThreadId = `t_${Date.now()}`;
      onThreadStarted?.(activeThreadId);
    }
    loadThreads();
    void loadStarters();
  });

  // Escape stops an answer even when the message box does not have focus.
  // The message box and open menus handle Escape first and mark it handled.
  onMount(() => {
    const onKey = (event: KeyboardEvent) => {
      // The chat stays loaded while other screens show; it only listens
      // while it is on screen.
      if (!active) return;
      if ((event.metaKey || event.ctrlKey) && event.shiftKey && event.key.toLowerCase() === "m") {
        event.preventDefault();
        pickerOpen = !pickerOpen;
        return;
      }
      if ((event.metaKey || event.ctrlKey) && !event.shiftKey && !event.altKey && event.key.toLowerCase() === "t") {
        event.preventDefault();
        cycleEffort();
        return;
      }
      if (event.key === "Escape" && showSettingsPanel && !event.defaultPrevented) {
        showSettingsPanel = false;
        return;
      }
      if (event.key === "Escape" && run.streaming && !event.defaultPrevented) {
        event.preventDefault();
        void stopGenerating();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  // Leaving Chat while an answer streams stops it and keeps what arrived.
  onDestroy(() => {
    if (!run.streaming) return;
    if (run.activeJobId) void api.cancelChat(run.activeJobId).catch(() => undefined);
    run.streaming = false;
    run.activeJobId = null;
    void persistCurrentThread();
  });
</script>

<div class="chat-root relative flex h-full min-h-0 min-w-0 flex-col overflow-hidden">
  <ChatHeader
    title={activeThreadTitle}
    bind:editingTitle
    bind:titleDraft
    canRename={messages.length > 0 && !run.streaming}
    {titleBusy}
    folder={activeFolder}
    onlineLabel={onlineChoice?.provider_label ?? null}
    settingsOpen={showSettingsPanel}
    {railOpen}
    moreOpen={showMoreActions}
    hasMessages={messages.length > 0}
    hasTwoMessages={messages.length >= 2}
    streaming={run.streaming}
    onTitleClick={onTitleClick}
    onTitleDblClick={onTitleDblClick}
    onStartTitleEdit={startTitleEdit}
    onSaveTitleEdit={saveTitleEdit}
    onGenerateTitle={() => void generateTitle()}
    onToggleSettings={() => (showSettingsPanel = !showSettingsPanel)}
    onOpenSearch={() => (chatSearchOpen = true)}
    onToggleRail={toggleRail}
    onToggleMore={() => (showMoreActions = !showMoreActions)}
    onCloseMore={() => (showMoreActions = false)}
    onExportMarkdown={() => void exportCurrentThread("markdown")}
    onCopy={() => void copyCurrentThread()}
    onExportJson={() => void exportCurrentThread("json")}
    onSummarize={() => void summarizeLast()}
    onRevealFolder={() => void api.revealFile("", activeFolder).catch((e: unknown) => showToast(errorMessage(e), "error"))}
    onNewChat={() => (onNewChatRequested ? onNewChatRequested() : newChat(activeFolder))}
  />

  {#if railOpen}
    <PanelRail
      sources={lastCitations}
      skills={skills}
      todos={workTodos}
      subagents={workAgents}
      startedAt={run.runStartedAt}
      toolRounds={runToolRounds}
      live={run.streaming}
      {usage}
      {segments}
      cacheRate={lastCacheRate}
      cacheRateAverage={ledgerCacheRate}
      {autoCompactAt}
      onAsk={askFromRail}
      onSkill={skillFromRail}
      onCompact={() => handleSlashCommand("compact")}
      onClose={toggleRail}
    />
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
          <h3 class="mb-2 text-[13px] font-semibold text-ink-300">Reading</h3>
          <div class="mb-3">
            <span class="mb-1.5 block text-sm text-ink-100">Chat width</span>
            <div class="grid grid-cols-3 gap-1 rounded-lg border border-ink-700 bg-ink-900 p-1" role="group" aria-label="Chat width">
              {#each [["cozy", "Comfortable"], ["wide", "Wider"], ["full", "Full"]] as [value, label] (value)}
                <button type="button" onclick={() => setChatWidth(value as "cozy" | "wide" | "full")} aria-pressed={chatWidthChoice === value} class={`rounded-md px-2 py-1.5 text-[13px] transition-colors ${chatWidthChoice === value ? "bg-ember-500/15 font-semibold text-ember-200" : "text-ink-300 hover:bg-veil/5"}`}>{label}</button>
              {/each}
            </div>
          </div>
          <div class="space-y-1">
            {#each [
              { label: "Follow answers while run.streaming", hint: "Keep the newest text in view instead of fighting the scrollbar.", on: followStreaming, toggle: toggleFollowStreaming },
              { label: "Collapse long questions", hint: "Show the first lines of a long prompt; click to expand.", on: collapseQuestions, toggle: toggleCollapseQuestions },
            ] as o (o.label)}
              <button type="button" role="switch" aria-checked={o.on} onclick={o.toggle} class="flex w-full items-start gap-3 rounded-lg px-2 py-2 text-left hover:bg-veil/5">
                <span class="min-w-0 flex-1"><span class="block text-sm text-ink-100">{o.label}</span><span class="block text-xs leading-5 text-ink-400">{o.hint}</span></span>
                <span class={`sheet-switch ${o.on ? "sheet-switch-on" : ""}`} aria-hidden="true"><span></span></span>
              </button>
            {/each}
          </div>
        </section>

        <section>
          <h3 class="mb-2 text-[13px] font-semibold text-ink-300">Response style</h3>
          <div class="grid grid-cols-2 gap-2">
            {#each systemPresets as p (p.label)}
              <button type="button" onclick={() => { systemPrompt = p.prompt; persona = p.label; void savePrefs(); }} aria-pressed={systemPrompt === p.prompt} class={`rounded-lg border px-3 py-2 text-left text-sm transition-colors ${systemPrompt === p.prompt ? "border-ember-500/50 bg-ember-500/10 text-ink-50" : "border-ink-700 text-ink-300 hover:border-edge hover:text-ink-50"}`}>{p.label}</button>
            {/each}
          </div>
        </section>

        <section>
          <h3 class="mb-2 text-[13px] font-semibold text-ink-300">While it works</h3>
          <div class="space-y-1">
            {#each [
              { label: "Learn skills from chats", hint: "Saves and improves skills on its own, with a note and Undo.", on: learnSkills, toggle: () => (learnSkills = !learnSkills), show: moduleOn("skills") },
              { label: "Suggest a next prompt", hint: "One short suggestion after each answer. Tab takes it.", on: suggestNext, toggle: () => (suggestNext = !suggestNext), show: true },
              { label: "Answer statistics", hint: "Tokens, time and speed under each answer.", on: answerStats, toggle: toggleAnswerStats, show: true },
              { label: "Charts, diagrams and mockups", hint: "Draws them in the chat, sandboxed from your files.", on: visuals, toggle: () => (visuals = !visuals), show: true },
              { label: "Subagents", hint: "Hands parts of a task to helpers that only read.", on: useSubagents, toggle: () => (useSubagents = !useSubagents), show: true },
            ].filter((o) => o.show) as o (o.label)}
              <button type="button" role="switch" aria-checked={o.on} onclick={o.toggle} class="flex w-full items-start gap-3 rounded-lg px-2 py-2 text-left hover:bg-veil/5">
                <span class="min-w-0 flex-1"><span class="block text-sm text-ink-100">{o.label}</span><span class="block text-xs leading-5 text-ink-400">{o.hint}</span></span>
                <span class={`sheet-switch ${o.on ? "sheet-switch-on" : ""}`} aria-hidden="true"><span></span></span>
              </button>
            {/each}
          </div>
        </section>

        <section>
          <h3 class="mb-2 text-[13px] font-semibold text-ink-300">How LocalAide answers</h3>
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
            <h3 class="mb-2 flex items-center gap-1.5 text-[13px] font-semibold text-ink-300"><Icon name="flask" class="h-3.5 w-3.5" />Sampling</h3>
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
          <h3 class="mb-2 flex items-center gap-1.5 text-[13px] font-semibold text-ink-300"><Icon name="layers" class="h-3.5 w-3.5" />Context window</h3>
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

  <div bind:this={scrollEl} onscroll={handleScroll} role="region" aria-label="Conversation" class={`relative min-h-0 flex-1 overflow-y-auto ${railOpen ? "lg:pr-72" : ""}`}>
    {#if messages.length === 0}
      <ChatEmptyState
        {chatWidth}
        {modelsChecked}
        {activeChoice}
        {choices}
        {onOpenModels}
        {moduleOn}
        onOpenAddProvider={openAddProvider}
        {greeting}
        {modelNotice}
        {onlineChoice}
        {engineLoading}
        {pickerBusy}
        {engineLoaded}
        {useWeb}
        {webAvailable}
        {mode}
        modeLabel={currentModeInfo?.label ?? null}
        {localDocs}
        {localNotes}
        {localBudgetGb}
        {starterPrompts}
        {skills}
        onPickStarter={(prompt) => { input = prompt; send(); }}
        onPickSkill={(body) => { input = body; }}
      />
    {:else}
      <div class={`mx-auto w-full ${chatWidth} px-4 md:px-6`}>
        <div style={`height: ${$messageVirtualizer.getTotalSize()}px; position: relative;`}>
          {#each $messageVirtualizer.getVirtualItems() as vItem (vItem.key)}
            {@const msg = messages[vItem.index]}
            {@const idx = vItem.index}
            <div
              data-index={idx}
              use:$messageVirtualizer.measureElement
              style={`position: absolute; top: 0; left: 0; width: 100%; transform: translateY(${vItem.start}px);`}
            >
              {#if msg}
                {#if idx === historyFrom && historyFrom > 0}
                  <div class={`${flag("compact") ? "mb-4" : "mb-7"} flex items-center gap-3 text-xs text-ink-500`} role="separator">
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
                  streaming={run.streaming && idx === messages.length - 1}
                  citations={msg.citations}
                  steps={msg.steps}
                  parts={msg.parts}
                  attachments={msg.attachments}
                  model={msg.model}
                  steer={msg.steer}
                  approvals={idx === messages.length - 1 ? approvals : {}}
                  onDecide={(toolId, decision) => void decideTool(toolId, decision)}
                  onAnswer={msg.role === "assistant" && idx === messages.length - 1 && !run.streaming ? answerQuestion : undefined}
                  onA2uiAction={msg.role === "assistant" ? sendA2uiAction : undefined}
                  onAnswerDeferred={!run.streaming ? (text, id) => answerQuestion(text, id) : undefined}
                  onDismissQuestion={dismissQuestion}
                  onUndoSkill={(id) => void undoSkill(id)}
                  folder={activeFolder}
                  onOther={focusComposer}
                  stats={showStats ? msg.stats ?? null : null}
                  allowRaw={flag("raw_messages")}
                  isLast={idx === messages.length - 1}
                  branchCount={activeThreadId ? branchCountAt(activeThreadId, idx) : 0}
                  onRestoreBranch={() => restoreBranch(idx)}
                  onRetry={msg.role === "assistant" && idx === messages.length - 1 ? retryLast : undefined}
                  onEdit={msg.role === "user" && !msg.steer ? (newText) => editTurn(newText, idx) : undefined}
                  onSendToOpenCode={moduleOn("opencode") ? () => void sendToOpenCode(idx) : undefined}
                  onViewSubagent={(part) => (transcriptAgent = part)}
                  collapseLong={collapseQuestions}
                />
              {/if}
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>

  {#if chatSearchOpen}
    <div
      class="absolute left-1/2 top-3 z-30 flex w-[min(30rem,90%)] -translate-x-1/2 items-center gap-1 rounded-xl border border-ink-700 bg-ink-900 px-2 py-1.5 shadow-[var(--shadow-elev)]"
      role="search"
      in:fly={{ y: -6, duration: dur(motion.base), easing: motion.enter }}
    >
      <Icon name="search" class="h-4 w-4 shrink-0 text-ink-400" />
      <input
        bind:this={chatSearchInput}
        bind:value={chatSearchQuery}
        onkeydown={searchKeydown}
        aria-label="Search in this chat"
        placeholder="Search in this chat"
        class="field-bare h-7 min-w-0 flex-1 bg-transparent text-sm text-ink-50 placeholder:text-ink-500"
      />
      {#if chatSearchQuery.trim()}
        <span class="shrink-0 text-xs tabular-nums text-ink-500" aria-live="polite">
          {chatMatches.length === 0 ? "No matches" : `${chatSearchIndex + 1} of ${chatMatches.length}`}
        </span>
      {/if}
      <button type="button" aria-label="Previous match" disabled={chatMatches.length === 0} onclick={() => { if (chatMatches.length) { chatSearchIndex = (chatSearchIndex - 1 + chatMatches.length) % chatMatches.length; jumpToMatch(chatMatches[chatSearchIndex]); } }} class="chat-icon-btn h-7 w-7 disabled:opacity-40"><Icon name="chevron-down" class="h-4 w-4 rotate-180" /></button>
      <button type="button" aria-label="Next match" disabled={chatMatches.length === 0} onclick={() => { if (chatMatches.length) { chatSearchIndex = (chatSearchIndex + 1) % chatMatches.length; jumpToMatch(chatMatches[chatSearchIndex]); } }} class="chat-icon-btn h-7 w-7 disabled:opacity-40"><Icon name="chevron-down" class="h-4 w-4" /></button>
      <button type="button" aria-label="Close search" onclick={closeChatSearch} class="chat-icon-btn h-7 w-7"><Icon name="close" class="h-4 w-4" /></button>
    </div>
  {/if}



  <ChatComposer
    {run}
    {railOpen}
    {chatWidth}
    {userScrolledUp}
    bind:chatError
    bind:sendFailed
    {workTodos}
    {workAgents}
    {runToolRounds}
    bind:planReady
    {buildMode}
    {pinnedTitles}
    bind:suggestions
    bind:input
    bind:previousDraft
    {usage}
    {autoCompactAt}
    {historyFrom}
    {segments}
    {lastCacheRate}
    {ledgerCacheRate}
    {approvalMode}
    {effort}
    {choices}
    {activeChoice}
    bind:pickerOpen
    {pickerBusy}
    {engineLoading}
    {refreshingProvider}
    bind:toolPath
    bind:toolContent
    bind:showTools
    {pendingImages}
    {pendingFiles}
    {attaching}
    {enhancing}
    memory={activeChoice?.local && localRam ? { usedGb: localRam.totalGb - localRam.availableGb, totalGb: localRam.totalGb, loaded: engineLoaded, modelName: activeChoice.name } : null}
    inputSkills={moduleOn("skills") ? skills : []}
    bind:useWeb
    {webAvailable}
    bind:useRecall
    {recallAvailable}
    bind:useFiles
    bind:useShell
    {shellAvailable}
    bind:useSkills
    skillsAvailable={moduleOn("skills")}
    mcpServers={mcpList.map((m) => ({ id: m.id, name: m.name, on: !mcpOff.includes(m.id) }))}
    mcpAvailable={moduleOn("mcp")}
    onManageProviders={moduleOn("online") ? openAddProvider : undefined}
    onGetModels={onOpenModels}
    onScrollToBottom={() => scrollToBottom(true)}
    onRetryLast={retryLast}
    onBuildPlan={buildPlan}
    onSend={send}
    onCancel={stopGenerating}
    onResume={resumeGenerating}
    onQueue={queueDraft}
    onSteer={steerDraft}
    onReadFile={readFile}
    onWriteFile={writeFile}
    onPickImages={addImages}
    onRemoveImage={removeImage}
    onPickSkill={(body) => { input = body; }}
    onSlashCommand={handleSlashCommand}
    onEnhance={() => void enhancePrompt()}
    onLoadModel={activeChoice?.local && !engineLoaded ? () => void selectModel(activeChoice) : undefined}
    onUnloadModel={activeChoice?.local && engineLoaded ? () => void unloadLocalModel() : undefined}
    onAttachFiles={() => void attachFiles()}
    onRemoveAttachment={(i) => pendingFiles.splice(i, 1)}
    onToggleMcp={toggleMcp}
    onSelectModel={(m) => void selectModel(m)}
    onToggleFavorite={(key) => void toggleFavorite(key)}
    onRefreshProvider={(id) => void refreshProvider(id)}
    onChangeApprovalMode={setApprovalMode}
    onEffortChange={setEffort}
    onCompact={() => handleSlashCommand("compact")}
    onFocusComposer={focusComposer}
    onSendQueuedNow={sendQueuedNow}
    onOpenAddProvider={openAddProvider}
  />

  {#if addingProvider}
    <Modal title="Add an online provider" size="lg" onClose={() => (addingProvider = false)}>
      <p class="mb-3 text-sm leading-6 text-ink-400">Your messages go to this provider only while the chat uses one of its models. A key is kept on this computer and sent only to the provider's own address.</p>
      <ProviderForm editing={null} onSaved={(status) => void providerSaved(status)} onCancel={() => (addingProvider = false)} />
    </Modal>
  {/if}

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

  <SubagentTranscript agent={transcriptAgent} onClose={() => (transcriptAgent = null)} />

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
  .sheet-switch {
    position: relative;
    margin-top: 0.2rem;
    height: 1.25rem;
    width: 2.25rem;
    flex-shrink: 0;
    border-radius: 9999px;
    background: var(--color-ink-700);
    transition: background-color 150ms ease-out;
  }
  .sheet-switch span {
    position: absolute;
    left: 0.15rem;
    top: 0.15rem;
    height: 0.95rem;
    width: 0.95rem;
    border-radius: 9999px;
    background: white;
    transition: transform 160ms cubic-bezier(0.23, 1, 0.32, 1);
  }
  .sheet-switch-on {
    background: var(--color-ember-500);
  }
  .sheet-switch-on span {
    transform: translateX(1rem);
  }
</style>
