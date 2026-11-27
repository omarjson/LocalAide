// One error shape for every command. The frontend reads code + message, and
// errorMessage() in the UI already turns codes into plain words.

export class AppError extends Error {
  constructor(code, message, { detail = null, retryable = false } = {}) {
    super(message);
    this.name = "AppError";
    this.code = code;
    this.detail = detail;
    this.retryable = retryable;
  }
  static invalidInput(message) {
    return new AppError("INVALID_INPUT", message);
  }
  static internal(detail) {
    return new AppError("INTERNAL", "Something in LocalAide failed. Try again.", { detail: String(detail) });
  }
  static notFound(message) {
    return new AppError("NOT_FOUND", message);
  }
}

/** Wraps a handler so a thrown error crosses IPC as {code, message}. */
export const guard = (fn) => async (args, ctx) => {
  try {
    return await fn(args ?? {}, ctx);
  } catch (e) {
    if (e instanceof AppError) throw e;
    if (e && typeof e === "object" && "code" in e && "message" in e) throw e;
    throw AppError.internal(e?.stack || e);
  }
};

