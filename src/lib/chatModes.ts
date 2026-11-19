import type { ChatModeId } from "./api/bindings";

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
  ms: number;
  firstTokenMs: number | null;
}
