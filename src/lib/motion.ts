import { prefersReducedMotion } from "svelte/motion";
import { cubicOut, quintOut } from "svelte/easing";

/** Minimal shape Svelte expects from a transition; declared locally so this
 *  helper stays importable in plain Node tests without the Svelte type tree. */
type TransitionConfig = {
  duration: number;
  easing?: (t: number) => number;
  css?: (t: number, u: number) => string;
};

/**
 * Duration helper for Svelte's JS-driven transitions (fade/fly/scale/slide).
 * The global reduced-motion CSS block in main.css cannot touch Web Animations
 * API transitions, so every `in:`/`out:`/`transition:` duration goes through
 * this: reduced-motion users get instant (0 ms) instead of animated.
 */
export function dur(ms: number): number {
  const off = typeof document !== "undefined" && document.documentElement.dataset.motion === "off";
  return prefersReducedMotion.current || off ? 0 : ms;
}

// The same three tokens are declared in main.css as --dur-fast/base/slow and
// drive CSS transitions. Reading them here keeps JS and CSS on one clock.
const FALLBACK = { fast: 120, base: 180, slow: 260 } as const;
type DurationToken = keyof typeof FALLBACK;
let tokenCache: Record<DurationToken, number> | null = null;

function readToken(name: string, fallback: number): number {
  if (typeof document === "undefined" || typeof getComputedStyle === "undefined") return fallback;
  const raw = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  if (!raw) return fallback;
  const ms = raw.endsWith("ms") ? parseFloat(raw) : raw.endsWith("s") ? parseFloat(raw) * 1000 : parseFloat(raw);
  return Number.isFinite(ms) && ms >= 0 ? ms : fallback;
}

function tokens(): Record<DurationToken, number> {
  if (!tokenCache) {
    tokenCache = {
      fast: readToken("--dur-fast", FALLBACK.fast),
      base: readToken("--dur-base", FALLBACK.base),
      slow: readToken("--dur-slow", FALLBACK.slow),
    };
  }
  return tokenCache;
}

/** One set of timings for the whole app, so motion feels like one product. */
export const motion = {
  /** Hover and press feedback. */
  get fast() { return tokens().fast; },
  /** Menus, dialogs, toasts. */
  get base() { return tokens().base; },
  /** Screen and section changes. */
  get slow() { return tokens().slow; },
  ease: cubicOut,
  enter: quintOut,
};

/** A screen or section entering: switched many times a day (often by
 * keyboard), so only a short, small rise. */
export const enterScreen = () => ({ y: 4, duration: dur(motion.fast), easing: motion.enter });

/** A dialog panel entering. */
export const enterDialog = () => ({ start: 0.96, duration: dur(motion.base), easing: motion.enter });
export const leaveDialog = () => ({ start: 0.98, duration: dur(motion.fast) });

/** Combined opacity + scale, for panels and popovers that appear in place.
 *  Use as `in:fadeScale={{ start: 0.95 }}` or bare `in:fadeScale`. */
export function fadeScale(_node: Element, params: { start?: number; duration?: number } = {}): TransitionConfig {
  const start = params.start ?? 0.96;
  const d = dur(params.duration ?? motion.base);
  if (!d) return { duration: 0, css: () => "" };
  return {
    duration: d,
    easing: motion.enter,
    css: (t: number) => `opacity:${t};transform:scale(${start + (1 - start) * t})`,
  };
}

/** Small upward entrance, for sheets and menus that rise into view. */
export function slideUp(_node: Element, params: { y?: number; duration?: number } = {}): TransitionConfig {
  const y = params.y ?? 12;
  const d = dur(params.duration ?? motion.base);
  if (!d) return { duration: 0, css: () => "" };
  return {
    duration: d,
    easing: motion.enter,
    css: (t: number, u: number) => `opacity:${t};transform:translateY(${u * y}px)`,
  };
}

/** Quick lateral cross-fade, for swapping tabs or panes. */
export function tabSwap(_node: Element, params: { x?: number; duration?: number } = {}): TransitionConfig {
  const x = params.x ?? 6;
  const d = dur(params.duration ?? motion.fast);
  if (!d) return { duration: 0, css: () => "" };
  return {
    duration: d,
    easing: motion.ease,
    css: (t: number, u: number) => `opacity:${t};transform:translateX(${u * x}px)`,
  };
}

/** Items in a list entering one after another (capped so long lists stay quick). */
export const stagger = (index: number) => ({
  y: 6,
  duration: dur(motion.base),
  delay: dur(Math.min(index, 8) * (motion.fast / 4)),
  easing: motion.enter,
});

/** Shared panel entrance, mirroring the `.surface-enter` CSS preset. */
export const surfaceEnter = () => ({ y: 4, duration: dur(motion.base), easing: motion.enter });

/** Reordering lists (`animate:flip`). */
export const flipOptions = () => ({ duration: dur(motion.base), easing: motion.ease });
