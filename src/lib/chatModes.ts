import type { ApprovalMode, ChatModeId } from "./api/bindings";
import type { IconName } from "../components/Icon.svelte";

/** One icon per chat mode, used by the mode picker and messages. */
export const MODE_ICONS: Record<ChatModeId, "sparkle" | "shield" | "unlock" | "bolt"> = {
  standard: "sparkle",
  strict: "shield",
  unrestricted: "unlock",
  autonomous: "bolt",
};

export const isChatMode = (value: unknown): value is ChatModeId =>
  value === "standard" || value === "strict" || value === "unrestricted" || value === "autonomous";

/** Answer statistics shown with the developer flag "Answer statistics". */
export interface AnswerStats {
  tokens: number;
  inputTokens?: number;
  outputTokens?: number;
  ms: number;
  firstTokenMs: number | null;
}

// ---- Permission mode (the chat box's permission icon) ----
//
// One list for the permission menu, the approval prompts and Settings, so
// a mode is never named two ways. Blocked commands never run in any mode.

export interface PermissionModeInfo {
  id: Exclude<ApprovalMode, "chat">;
  label: string;
  hint: string;
  icon: IconName;
}

export const PERMISSION_MODES: PermissionModeInfo[] = [
  { id: "plan", label: "Plan mode", hint: "Plan before editing.", icon: "lightbulb" },
  { id: "ask", label: "Ask before changes", hint: "Ask before file changes.", icon: "hand" },
  { id: "smart", label: "Autonomous mode", hint: "Ask when there's risk.", icon: "shield" },
  { id: "auto", label: "Full access", hint: "Run with fewer confirmations.", icon: "shield-check" },
];

export const isPermissionMode = (v: unknown): v is PermissionModeInfo["id"] =>
  v === "plan" || v === "ask" || v === "smart" || v === "auto";

/** The menu entry for a mode. The legacy "chat" value reads as Ask. */
export const permissionInfo = (mode: ApprovalMode | null | undefined): PermissionModeInfo =>
  PERMISSION_MODES.find((m) => m.id === mode) ?? PERMISSION_MODES[1];

export const permissionLabel = (mode: ApprovalMode | null | undefined): string => permissionInfo(mode).label;

/** A tool call waiting for the user's OK. */
export interface PendingApproval {
  approval_id: string;
  preview: string;
  /** The permission mode the answer was sent in. */
  mode?: ApprovalMode;
  /** Terminal requests: seconds until the command runs (0 = wait for the user). */
  countdown?: number;
}

/** Where every chat keeps its switches, the permission mode included. */
export const CHAT_OPTIONS_KEY = "localaide-chat-options";
/** Sent on `window` when Settings changes the permission mode. */
export const APPROVAL_MODE_EVENT = "localaide:approval-mode";

type StorageLike = Pick<Storage, "getItem" | "setItem">;
type TargetLike = Pick<EventTarget, "addEventListener" | "removeEventListener" | "dispatchEvent">;

const defaultStorage = (): StorageLike | null => {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
};
const defaultTarget = (): TargetLike | null => (typeof window === "undefined" ? null : window);

const readOptions = (storage: StorageLike | null): Record<string, unknown> => {
  try {
    const parsed: unknown = JSON.parse(storage?.getItem(CHAT_OPTIONS_KEY) ?? "{}");
    return parsed && typeof parsed === "object" ? (parsed as Record<string, unknown>) : {};
  } catch {
    return {};
  }
};

/** The saved permission mode, or null when none is saved. */
export const readSavedApprovalMode = (storage: StorageLike | null = defaultStorage()): PermissionModeInfo["id"] | null => {
  const saved = readOptions(storage).approvals;
  return isPermissionMode(saved) ? saved : null;
};

/**
 * Save the permission mode for every chat and tell the open ones. Writes
 * the same key and fields the chat box does (`approvals`, and `buildMode`
 * when the mode is not Plan), so there is one saved state.
 */
export const shareApprovalMode = (
  mode: PermissionModeInfo["id"],
  storage: StorageLike | null = defaultStorage(),
  target: TargetLike | null = defaultTarget(),
): void => {
  const options = readOptions(storage);
  options.approvals = mode;
  if (mode !== "plan") options.buildMode = mode;
  try {
    storage?.setItem(CHAT_OPTIONS_KEY, JSON.stringify(options));
  } catch {
    // Storage can be unavailable; open chats still switch below.
  }
  target?.dispatchEvent(new CustomEvent(APPROVAL_MODE_EVENT, { detail: mode }));
};

/** Follow mode changes made in Settings. Returns the unsubscribe. */
export const onSharedApprovalMode = (
  callback: (mode: PermissionModeInfo["id"]) => void,
  target: TargetLike | null = defaultTarget(),
): (() => void) => {
  const handler = (event: Event) => {
    const mode = (event as CustomEvent<unknown>).detail;
    if (isPermissionMode(mode)) callback(mode);
  };
  target?.addEventListener(APPROVAL_MODE_EVENT, handler);
  return () => target?.removeEventListener(APPROVAL_MODE_EVENT, handler);
};
