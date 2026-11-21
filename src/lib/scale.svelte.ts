// The interface scale (Settings → Appearance), 70% to 250%. The app zooms
// the whole window natively, so text, icons and hit areas grow together;
// Ctrl + and Ctrl - step it, Ctrl 0 resets it.
import { api } from "./api/client";
import { showToast } from "./toast.svelte";

export const SCALE_MIN = 0.7;
export const SCALE_MAX = 2.5;

export const scale = $state({ value: 1 });

const clamp = (v: number) => Math.round(Math.min(SCALE_MAX, Math.max(SCALE_MIN, v)) * 100) / 100;

/** Read the saved scale (the window is already zoomed at startup). */
export async function applyScale(): Promise<void> {
  try {
    scale.value = clamp(await api.uiScale());
  } catch {
    scale.value = 1;
  }
}

export async function setScale(value: number): Promise<void> {
  const next = clamp(value);
  scale.value = next;
  try {
    scale.value = await api.setUiScale(next);
  } catch (e: unknown) {
    showToast(`The scale could not be changed: ${e instanceof Error ? e.message : String(e)}`, "error");
  }
}

/** Step by `delta`; 0 resets to 100%. */
export async function nudgeScale(delta: number): Promise<void> {
  const next = delta === 0 ? 1 : clamp(scale.value + delta);
  if (next === scale.value) return;
  await setScale(next);
  showToast(`Interface scale ${Math.round(next * 100)}%`);
}
