import {
  api,
  type ChatMessage,
  type ChatStreamEvent,
  type MessagePart,
} from "@goat/client-runtime/localaide";
import {
  addQuestion,
  addSubagentStep,
  appendReasoning,
  appendText,
  closeReasoning,
  finishTool,
  setSkillChange,
  setTodos,
  setToolStatus,
  startTool,
  textOf,
} from "~/lib/chatParts";
import { createStore, useStore } from "./store";

/**
 * The conversation on screen.
 *
 * The backend streams a chat turn over a channel: tokens, thinking, tool
 * calls, approval requests and the final word. This store turns that stream
 * into the list of messages the view draws.
 */
export type PendingApproval = {
  readonly approvalId: string;
  readonly toolId: string;
  readonly title: string;
  readonly detail: string;
  readonly preview: string;
};

export type ChatState = {
  threadId: string | null;
  messages: ChatMessage[];
  streaming: boolean;
  jobId: string | null;
  status: string | null;
  error: string | null;
  approval: PendingApproval | null;
};

export const chatStore = createStore<ChatState>({
  threadId: null,
  messages: [],
  streaming: false,
  jobId: null,
  status: null,
  error: null,
  approval: null,
});

/** Replaces the answer that is streaming right now. */
function updateAnswer(mutate: (parts: MessagePart[]) => void): void {
  chatStore.set((state) => {
    const last = state.messages.at(-1);
    if (!last || last.role !== "assistant") return state;
    const parts = [...(last.parts ?? [])];
    mutate(parts);
    const messages = [...state.messages];
    messages[messages.length - 1] = { ...last, parts, content: textOf(parts) };
    return { ...state, messages };
  });
}

/** Reads a saved conversation and shows it. */
export async function loadThread(threadId: string): Promise<void> {
  chatStore.set((state) => ({ ...state, threadId, messages: [], error: null }));
  try {
    const messages = await api.loadChatThread(threadId);
    chatStore.set((state) => ({ ...state, messages }));
  } catch (error) {
    chatStore.set((state) => ({
      ...state,
      error: messageOf(error, "This conversation could not be opened."),
    }));
  }
}

/** Clears the view for a new conversation. */
export function resetChat(threadId: string | null): void {
  chatStore.set({
    threadId,
    messages: [],
    streaming: false,
    jobId: null,
    status: null,
    error: null,
    approval: null,
  });
}

/** What the user asked, and what the model is answering. */
export type SendOptions = {
  readonly model?: string;
  readonly mode?: string;
  readonly approvalMode?: string;
  readonly useWeb?: boolean;
  readonly useRecall?: boolean;
  readonly folder?: string | null;
};

export async function sendMessage(text: string, options: SendOptions = {}): Promise<void> {
  const trimmed = text.trim();
  if (!trimmed) return;
  const state = chatStore.get();
  if (state.streaming) return;

  const threadId = state.threadId ?? createThreadId();
  const history = state.messages;
  const userMessage: ChatMessage = {
    role: "user",
    content: trimmed,
    ts: Date.now(),
  };

  chatStore.set((current) => ({
    ...current,
    threadId,
    messages: [
      ...current.messages,
      userMessage,
      { role: "assistant", content: "", ts: Date.now(), parts: [] },
    ],
    streaming: true,
    status: "Thinking",
    error: null,
    approval: null,
  }));

  try {
    const jobId = await api.streamChat(
      {
        message: trimmed,
        model: options.model ?? "",
        thread_id: threadId,
        history,
        mode: options.mode as never,
        approval_mode: options.approvalMode as never,
        use_web: options.useWeb ?? false,
        use_recall: options.useRecall ?? true,
        folder: options.folder ?? null,
        now: new Date().toLocaleString(),
      },
      (event) => handleEvent(event),
    );
    chatStore.set((current) => ({ ...current, jobId }));
  } catch (error) {
    chatStore.set((current) => ({
      ...current,
      streaming: false,
      jobId: null,
      status: null,
      error: messageOf(error, "That message could not be sent."),
    }));
  }
}

/** Stops the answer that is running. */
export async function cancelAnswer(): Promise<void> {
  const jobId = chatStore.get().jobId;
  if (!jobId) return;
  await api.cancelChat(jobId);
}

/** Answers a tool approval request. */
export async function answerApproval(decision: "allow" | "allow_chat" | "deny"): Promise<void> {
  const approval = chatStore.get().approval;
  if (!approval) return;
  chatStore.set((state) => ({ ...state, approval: null }));
  updateAnswer((parts) => setToolStatus(parts, approval.toolId, "done"));
  await api.decideTool(approval.approvalId, decision);
}

/** Saves the conversation so it survives a restart. */
export async function saveThread(title: string): Promise<void> {
  const state = chatStore.get();
  if (!state.threadId || state.messages.length === 0) return;
  await api.saveChatThread(
    state.threadId,
    title,
    null,
    state.messages.map((message) => ({
      role: message.role,
      content: message.content,
      ts: message.ts,
      parts: message.parts,
      citations: message.citations,
      images: message.images,
      steps: message.steps,
      stats: message.stats,
      attachments: message.attachments,
      model: message.model,
      error: message.error ?? false,
      steer: message.steer ?? false,
    })),
    null,
  );
}

function handleEvent(event: ChatStreamEvent): void {
  const now = performance.now();
  switch (event.type) {
    case "token":
      updateAnswer((parts) => appendText(parts, event.payload.token, now));
      break;
    case "reasoning":
      updateAnswer((parts) => appendReasoning(parts, event.payload.text, now));
      break;
    case "tool_call":
      updateAnswer((parts) => startTool(parts, event.payload, now));
      break;
    case "tool_approval":
      updateAnswer((parts) => setToolStatus(parts, event.payload.id, "waiting"));
      chatStore.set((state) => ({
        ...state,
        approval: {
          approvalId: event.payload.approval_id,
          toolId: event.payload.id,
          title: event.payload.title,
          detail: event.payload.detail,
          preview: event.payload.preview,
        },
        status: `LocalAide asks before it can ${event.payload.title.toLowerCase()}.`,
      }));
      break;
    case "tool_result":
      updateAnswer((parts) => finishTool(parts, event.payload));
      break;
    case "question":
      updateAnswer((parts) => addQuestion(parts, event.payload, now));
      chatStore.set((state) => ({
        ...state,
        status: `LocalAide asks: ${event.payload.question}`,
      }));
      break;
    case "todos":
      updateAnswer((parts) => setTodos(parts, event.payload.items, now));
      break;
    case "subagent_step":
      updateAnswer((parts) =>
        addSubagentStep(parts, event.payload.id, event.payload.step, event.payload.tokens ?? null),
      );
      break;
    case "skill_change":
      updateAnswer((parts) => setSkillChange(parts, event.payload));
      break;
    case "citation":
      updateAnswer((parts) => {
        const last = parts.at(-1);
        if (last?.type === "text") return;
        parts.push({ type: "text", text: "" });
      });
      break;
    case "status":
      chatStore.set((state) => ({ ...state, status: event.payload.label }));
      break;
    case "steered":
      chatStore.set((state) => ({ ...state, status: "Reading your note" }));
      break;
    case "error":
      chatStore.set((state) => ({
        ...state,
        streaming: false,
        jobId: null,
        status: null,
        error: event.payload.error,
      }));
      break;
    case "done":
      updateAnswer((parts) => closeReasoning(parts, now));
      chatStore.set((state) => ({ ...state, streaming: false, jobId: null, status: null }));
      break;
  }
}

function messageOf(error: unknown, fallback: string): string {
  if (error && typeof error === "object" && "message" in error) {
    const message = String((error as { message?: unknown }).message ?? "");
    if (message) return message;
  }
  return fallback;
}

/** A conversation id the backend accepts: letters, digits, dash and underscore. */
export function createThreadId(): string {
  return `t-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

export function useChatMessages(): ChatMessage[] {
  return useStore(chatStore, (state) => state.messages);
}

export function useChatStreaming(): boolean {
  return useStore(chatStore, (state) => state.streaming);
}

export function useChatStatus(): string | null {
  return useStore(chatStore, (state) => state.status);
}

export function useChatError(): string | null {
  return useStore(chatStore, (state) => state.error);
}

export function useChatApproval(): PendingApproval | null {
  return useStore(chatStore, (state) => state.approval);
}

export function useChatThreadId(): string | null {
  return useStore(chatStore, (state) => state.threadId);
}
