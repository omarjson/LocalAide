import { prefersReducedMotion } from "svelte/motion";
import { cubicOut, quintOut } from "svelte/easing";

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

/** One set of timings for the whole app, so motion feels like one product. */
export const motion = {
  /** Hover and press feedback. */
  fast: 120,
  /** Menus, dialogs, toasts. */
  base: 180,
  /** Screen and section changes. */
  slow: 260,
  ease: cubicOut,
  enter: quintOut,
} as const;

/** A screen or section entering: switched many times a day (often by
 * keyboard), so only a short, small rise. */
export const enterScreen = () => ({ y: 4, duration: dur(motion.fast), easing: motion.enter });

/** A dialog panel entering. */
export const enterDialog = () => ({ start: 0.96, duration: dur(motion.base), easing: motion.enter });
export const leaveDialog = () => ({ start: 0.98, duration: dur(motion.fast) });

/** Items in a list entering one after another (capped so long lists stay quick). */
export const stagger = (index: number) => ({
  y: 6,
  duration: dur(motion.base),
  delay: dur(Math.min(index, 8) * 30),
  easing: motion.enter,
});

/** Reordering lists (`animate:flip`). */
export const flipOptions = () => ({ duration: dur(motion.base), easing: motion.ease });
