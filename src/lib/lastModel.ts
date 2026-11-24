// "New chats start on": the model the user used last, remembered across
// restarts. A new chat takes it when it is still usable; when it is not
// (unloaded local model, provider key removed), the chat falls back to the
// active model and says so in one line instead of erroring or switching
// silently. Kept out of the chat screen so it can be tested.

const LAST_MODEL_KEY = "localaide-last-model";

/** The picker key of the last model used, or null when none was ever picked. */
export function loadLastUsedModel(): string | null {
  try {
    const value = localStorage.getItem(LAST_MODEL_KEY);
    return value ? value : null;
  } catch {
    return null;
  }
}

/** Remember a picked model. Storage failing is not the chat's problem. */
export function saveLastUsedModel(key: string): void {
  try {
    localStorage.setItem(LAST_MODEL_KEY, key);
  } catch {
    // Optional; the chat still starts on the active model.
  }
}

export interface NewChatModelChoice {
  key: string;
  local: boolean;
  active: boolean;
}

/**
 * Resolve the model a new chat starts on. `fellBack` is true only when a
 * last-used model existed but cannot be used now — that is the case the
 * UI must name in one honest line. A local last-used model counts as
 * usable only while the engine has it loaded; an unloaded one would
 * leave the empty chat saying "not loaded yet".
 */
export function resolveNewChatModel(
  lastUsed: string | null,
  models: NewChatModelChoice[],
  localLoaded: boolean,
): { key: string | null; fellBack: boolean } {
  if (lastUsed) {
    const found = models.find((m) => m.key === lastUsed);
    if (found && (!found.local || localLoaded)) return { key: found.key, fellBack: false };
  }
  const active = models.find((m) => m.active);
  return { key: active?.key ?? null, fellBack: lastUsed !== null };
}
