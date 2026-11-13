import { prefersReducedMotion } from "svelte/motion";

/**
 * Duration helper for Svelte's JS-driven transitions (fade/fly/scale/slide).
 * The global reduced-motion CSS block in main.css cannot touch Web Animations
 * API transitions, so every `in:`/`out:`/`transition:` duration goes through
 * this: reduced-motion users get instant (0 ms) instead of animated.
 */
export function dur(ms: number): number {
  return prefersReducedMotion.current ? 0 : ms;
}
