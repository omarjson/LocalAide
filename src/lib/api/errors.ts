// Readable messages for errors from the backend. No window or Tauri
// dependency, so unit tests run under plain Node.

/** Plain words for error codes whose backend text is too technical. */
const FRIENDLY_ERRORS: Record<string, string> = {
  CHECKSUM_MISMATCH: "The download was damaged. Download it again.",
  UPDATE_CHECK_FAILED: "Could not reach the update server. Check your internet connection and try again.",
};

/** Text made from a Rust error reads "[CODE] message". Keep the message. */
const withoutCode = (text: string): string => text.replace(/^\[[A-Z0-9_]+\]\s*/, "");

/**
 * Extract a readable message from a Tauri rejection, which can be a string,
 * an Error, or the serialized AppError object from the backend.
 */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return withoutCode(e) || "Something went wrong. Try again.";
  if (e instanceof Error) return withoutCode(e.message) || "Something went wrong. Try again.";
  if (typeof e === "object" && e !== null) {
    if ("code" in e && typeof e.code === "string" && FRIENDLY_ERRORS[e.code]) {
      return FRIENDLY_ERRORS[e.code];
    }
    if ("message" in e && typeof e.message === "string" && e.message.trim()) {
      return withoutCode(e.message);
    }
  }
  return "Something went wrong. Try again.";
}
