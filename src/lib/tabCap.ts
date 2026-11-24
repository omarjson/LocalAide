/**
 * Pure tab-cap logic, shared by newChat/openChat/restoreTabs.
 *
 * Kept free of Svelte runes and browser globals so it can be exercised by
 * plain Node unit tests (this repo runs `node --test` without a build step).
 * The UI store in chats.svelte.ts owns the actual $state; this module only
 * decides which tabs survive once the strip is full.
 */

/** The most chat tabs we keep open at once. */
export const MAX_TABS = 8;

/** The only shape the cap needs: a stable key and an optional recency stamp. */
export type RecencyTab = {
  key: string;
  /** Higher means used more recently. Absent values fall back to array order. */
  lastUsed?: number;
};

export type CapResult<T extends RecencyTab> = {
  tabs: T[];
  activeKey: string | null;
  /** Keys removed to make room, in removal order. */
  dropped: string[];
};

function recency<T extends RecencyTab>(tab: T, index: number): number {
  return Number.isFinite(tab.lastUsed) ? (tab.lastUsed as number) : index;
}

/**
 * Insert `incoming` (when given) and trim to at most `max` tabs.
 *
 * The incoming/active tab is never dropped. Among the rest, the tab with the
 * lowest `lastUsed` (ties go to the earlier array position) is the least
 * recently used and is removed first. Inputs are never mutated.
 */
export function capTabs<T extends RecencyTab>(
  tabs: readonly T[],
  activeKey: string | null | undefined,
  incoming?: T,
  max: number = MAX_TABS,
): CapResult<T> {
  const next = incoming ? [...tabs, incoming] : [...tabs];
  const dropped: string[] = [];
  const limit = Math.max(1, Math.floor(max));

  while (next.length > limit) {
    let victim = -1;
    for (let i = 0; i < next.length; i++) {
      const candidate = next[i];
      if (candidate.key === activeKey || (incoming && candidate.key === incoming.key)) continue;
      if (victim === -1 || recency(candidate, i) < recency(next[victim], victim)) victim = i;
    }
    // Only the protected tab(s) remain: stop rather than drop the active one.
    if (victim === -1) break;
    dropped.push(next[victim].key);
    next.splice(victim, 1);
  }

  return {
    tabs: next,
    activeKey: activeKey ?? next.at(-1)?.key ?? null,
    dropped,
  };
}
