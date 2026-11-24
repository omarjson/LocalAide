// Local-only notifications and a short chime. Only browser APIs are used:
// the Web Notification API and a WebAudio oscillator. Nothing here makes a
// network call, and nothing fires until the user opts in. Preferences live in
// localStorage next to the other UI-only settings (sidebar, tabs).
//
// This module is deliberately free of Svelte runes and imports so it can also
// be exercised by the plain Node unit tests. Components keep their own $state
// draft and call saveNotificationPrefs() when the user commits a change.
//
// The small labelled-answer formatter at the bottom is kept here for the same
// reason: it is pure, browser-only UI logic shared by the fusion dialog and
// its unit test, and must not pull in the Tauri client (which cannot load in
// Node).

export type NotificationKind = "goal" | "turn" | "approval" | "question";

export interface NotificationPrefs {
  /** Master switch. No notification or sound fires while this is false. */
  enabled: boolean;
  desktop: boolean;
  sound: boolean;
  goal: boolean;
  turn: boolean;
  approval: boolean;
  question: boolean;
}

export const NOTIFICATION_KEY = "localaide-notifications";

export const DEFAULT_NOTIFICATION_PREFS: NotificationPrefs = {
  enabled: false,
  desktop: true,
  sound: true,
  goal: true,
  turn: true,
  approval: true,
  question: true,
};

const BOOLEAN_KEYS: (keyof NotificationPrefs)[] = [
  "enabled", "desktop", "sound", "goal", "turn", "approval", "question",
];

const KIND_PREF: Record<NotificationKind, keyof NotificationPrefs> = {
  goal: "goal",
  turn: "turn",
  approval: "approval",
  question: "question",
};

/** Coerce anything read from storage into a complete, boolean-only preference set. */
export function normalizeNotificationPrefs(raw: unknown): NotificationPrefs {
  const out: NotificationPrefs = { ...DEFAULT_NOTIFICATION_PREFS };
  if (!raw || typeof raw !== "object") return out;
  const record = raw as Record<string, unknown>;
  for (const key of BOOLEAN_KEYS) {
    if (typeof record[key] === "boolean") out[key] = record[key] as boolean;
  }
  return out;
}

/** True only when the master switch, this event's switch and an output are all on. */
export function shouldNotify(prefs: NotificationPrefs, kind: NotificationKind): boolean {
  return (
    prefs.enabled === true &&
    prefs[KIND_PREF[kind]] === true &&
    (prefs.desktop === true || prefs.sound === true)
  );
}

function storage(): Storage | null {
  try {
    return typeof localStorage === "undefined" ? null : localStorage;
  } catch {
    return null;
  }
}

export function loadNotificationPrefs(): NotificationPrefs {
  const store = storage();
  if (!store) return { ...DEFAULT_NOTIFICATION_PREFS };
  try {
    const raw = store.getItem(NOTIFICATION_KEY);
    return raw ? normalizeNotificationPrefs(JSON.parse(raw)) : { ...DEFAULT_NOTIFICATION_PREFS };
  } catch {
    return { ...DEFAULT_NOTIFICATION_PREFS };
  }
}

export function saveNotificationPrefs(prefs: NotificationPrefs): NotificationPrefs {
  const clean = normalizeNotificationPrefs(prefs);
  const store = storage();
  try {
    store?.setItem(NOTIFICATION_KEY, JSON.stringify(clean));
  } catch {
    // Storage can be full or disabled; the choice still applies this session.
  }
  return clean;
}

/** Permission state for the desktop half, or "unsupported" on a non-browser host. */
export function notificationPermission(): NotificationPermission | "unsupported" {
  return typeof Notification === "undefined" ? "unsupported" : Notification.permission;
}

export async function requestNotificationPermission(): Promise<NotificationPermission | "unsupported"> {
  if (typeof Notification === "undefined") return "unsupported";
  try {
    return await Notification.requestPermission();
  } catch {
    return notificationPermission();
  }
}

let audio: AudioContext | null = null;

/** One short, quiet two-note chime. Best effort; audio is never a requirement. */
export function playChime(): boolean {
  if (typeof window === "undefined") return false;
  try {
    const Ctx = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!Ctx) return false;
    audio ??= new Ctx();
    const ctx: AudioContext = audio;
    const at = ctx.currentTime;
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = "sine";
    osc.frequency.setValueAtTime(660, at);
    osc.frequency.setValueAtTime(880, at + 0.09);
    gain.gain.setValueAtTime(0.0001, at);
    gain.gain.exponentialRampToValueAtTime(0.09, at + 0.015);
    gain.gain.exponentialRampToValueAtTime(0.0001, at + 0.22);
    osc.connect(gain);
    gain.connect(ctx.destination);
    osc.start(at);
    osc.stop(at + 0.24);
    return true;
  } catch {
    return false;
  }
}

/**
 * The single fire path. Returns true when at least one channel was actually
 * used, so the settings panel can show an honest confirmation.
 */
export function fireNotification(kind: NotificationKind, title: string, body: string): boolean {
  const prefs = loadNotificationPrefs();
  if (!shouldNotify(prefs, kind)) return false;
  let fired = false;
  if (
    prefs.desktop &&
    typeof Notification !== "undefined" &&
    Notification.permission === "granted"
  ) {
    try {
      new Notification(title, { body, tag: `localaide-${kind}` });
      fired = true;
    } catch {
      // Some webviews expose the API but reject construction; fall through.
    }
  }
  if (prefs.sound && playChime()) fired = true;
  return fired;
}

export function notifyGoalComplete(detail?: string): boolean {
  return fireNotification("goal", "Goal complete", detail ?? "LocalAide finished the goal.");
}

export function notifyTurnFinished(detail?: string): boolean {
  return fireNotification("turn", "Answer ready", detail ?? "LocalAide finished an answer while you were away.");
}

export function notifyToolApproval(detail?: string): boolean {
  return fireNotification("approval", "Approval needed", detail ?? "LocalAide is waiting for a tool approval.");
}

export function notifyQuestion(detail?: string): boolean {
  return fireNotification("question", "A question for you", detail ?? "LocalAide asked you a question.");
}

// --- Client-side answer fusion ------------------------------------------------

/** Structural shape of a multi-run answer (matches bindings.RunAnswer). */
export interface LabelledAnswer {
  label: string;
  text: string;
  error?: string | null;
}

/**
 * Join labelled answers into one document with clear headings and rules.
 * Failed or empty targets are skipped — the user already sees their errors in
 * the per-target cards, and folding them into the fused text would be noise.
 */
export function fuseRunAnswers(answers: LabelledAnswer[]): string {
  const kept = (answers ?? []).filter((a) => a && !a.error && typeof a.text === "string" && a.text.trim());
  if (kept.length === 0) return "";
  return kept
    .map((a) => `## ${a.label.trim() || "Unnamed target"}\n\n${a.text.trim()}`)
    .join("\n\n---\n\n");
}
