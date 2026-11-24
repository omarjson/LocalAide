// Approval timers: the choices Settings offers and how a countdown reads.
// 0 always means "no timer" (Off for plans, Wait for me for terminal
// commands). Kept free of Svelte so Node tests can run it.

export interface TimerChoice {
  value: string;
  label: string;
  seconds: number;
}

export const PLAN_TIMER_CHOICES: TimerChoice[] = [
  { value: "0", label: "Off", seconds: 0 },
  { value: "60", label: "1 min", seconds: 60 },
  { value: "300", label: "5 min", seconds: 300 },
];

export const TERMINAL_TIMER_CHOICES: TimerChoice[] = [
  { value: "0", label: "Wait for me", seconds: 0 },
  { value: "30", label: "30 s", seconds: 30 },
  { value: "60", label: "1 min", seconds: 60 },
  { value: "120", label: "2 min", seconds: 120 },
  { value: "300", label: "5 min", seconds: 300 },
];

/** Defaults when config.json has no value: plans wait, commands run after a minute. */
export const DEFAULT_PLAN_SECS = 0;
export const DEFAULT_TERMINAL_SECS = 60;
/** Same bound the backend clamps to. */
export const MAX_TIMER_SECS = 600;

export const clampTimer = (seconds: unknown, fallback: number): number => {
  if (typeof seconds !== "number" || !Number.isFinite(seconds)) return fallback;
  return Math.max(0, Math.min(MAX_TIMER_SECS, Math.round(seconds)));
};

/** 42 → "0:42", 300 → "5:00". Never negative. */
export const formatCountdown = (seconds: number): string => {
  const s = Math.max(0, Math.ceil(seconds));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
};

/** Plain words for a setting value: 0 → "Off", 60 → "1 minute". */
export const describeTimer = (seconds: number, zero: string): string => {
  if (seconds <= 0) return zero;
  if (seconds % 60 === 0) return seconds === 60 ? "1 minute" : `${seconds / 60} minutes`;
  return `${seconds} seconds`;
};
