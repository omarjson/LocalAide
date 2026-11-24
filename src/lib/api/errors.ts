// Readable messages for errors from the backend. No window or Tauri
// dependency, so unit tests run under plain Node.

/** Plain words for error codes whose backend text is too technical. */
const FRIENDLY_ERRORS: Record<string, string> = {
  CHECKSUM_MISMATCH: "The download was damaged. Download it again.",
  UPDATE_CHECK_FAILED: "Could not reach the update server. Check your internet connection and try again.",
};

/** The cause class of a failure. Never a generic "something went wrong":
 * the class names what happened, and ERROR_FIXES names the one action
 * that fixes it. */
export type ErrorCause =
  | "network"
  | "not-found"
  | "permission"
  | "auth"
  | "unavailable"
  | "invalid"
  | "cancelled"
  | "unknown";

/** One fixing action per cause class, exhaustive by typecheck: adding a
 * cause without its fix is a type error, not a silent "Unknown" at runtime. */
export const ERROR_FIXES = {
  network: "Check your internet connection and try again.",
  "not-found": "Check the name or path, then try again.",
  permission: "Check the file or folder permissions, then try again.",
  auth: "Check the provider key in Settings, then try again.",
  unavailable: "Start the model or service, then try again.",
  invalid: "Check what you entered, then try again.",
  cancelled: "Nothing to fix — the action was stopped.",
  unknown: "Try again; if it keeps happening, copy the diagnostics in Settings → About.",
} as const satisfies Record<ErrorCause, string>;

/** The one action that fixes this cause class. */
export function errorFix(cause: ErrorCause): string {
  return ERROR_FIXES[cause];
}

/** What kind of failure this is, from the backend code or the message. */
export function classifyError(e: unknown): ErrorCause {
  const code = typeof e === "object" && e !== null && "code" in e && typeof e.code === "string" ? e.code : "";
  const text = [
    code,
    typeof e === "string" ? e : "",
    e instanceof Error ? e.message : "",
    typeof e === "object" && e !== null && "message" in e && typeof e.message === "string" ? e.message : "",
  ].join(" ");
  if (/cancel/i.test(text)) return "cancelled";
  if (/NOT_FOUND|not found|404|no such|does not exist/i.test(text)) return "not-found";
  if (/PERMISSION|EACCES|permission denied|not allowed/i.test(text)) return "permission";
  if (/401|403|UNAUTHORIZED|invalid (api )?key|authentication|auth/i.test(text)) return "auth";
  if (/INVALID_INPUT|invalid/i.test(text)) return "invalid";
  if (/not (loaded|running)|unavailable|503|not started|not ready/i.test(text)) return "unavailable";
  if (/network|ECONN|ENOTFOUND|ETIMEDOUT|timed? ?out|fetch failed|could not (reach|connect)|offline|connection/i.test(text)) return "network";
  return "unknown";
}

/** Cause class, readable message, and the one fix — the typed error. */
export function describeError(e: unknown): { cause: ErrorCause; message: string; fix: string } {
  const cause = classifyError(e);
  return { cause, message: errorMessage(e), fix: ERROR_FIXES[cause] };
}

/** Text made from a Rust error reads "[CODE] message". Keep the message. */
const withoutCode = (text: string): string => text.replace(/^\[[A-Z0-9_]+\]\s*/, "");

/**
 * Extract a readable message from a Tauri rejection, which can be a string,
 * an Error, or the serialized AppError object from the backend.
 */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return withoutCode(e) || "Unknown error — LocalAide couldn't tell what went wrong. Try again.";
  if (e instanceof Error) return withoutCode(e.message) || "Unknown error — LocalAide couldn't tell what went wrong. Try again.";
  if (typeof e === "object" && e !== null) {
    if ("code" in e && typeof e.code === "string" && FRIENDLY_ERRORS[e.code]) {
      return FRIENDLY_ERRORS[e.code];
    }
    if ("message" in e && typeof e.message === "string" && e.message.trim()) {
      return withoutCode(e.message);
    }
  }
  return "Unknown error — LocalAide couldn't tell what went wrong. Try again.";
}
