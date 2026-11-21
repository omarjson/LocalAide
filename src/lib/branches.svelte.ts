// Conversation branches. Editing an old message no longer throws its
// future away: the discarded tail is parked here and a checkpoint chip on
// that message can swap it back in — again and again, like flipping a
// switch between the two paths. The store is reactive (so the checkpoint
// chips appear the moment a branch exists) and mirrors to local storage,
// so branches survive a restart. Message indices are the chat's own, so a
// compaction can shift where a branch sits; the tail itself stays intact.

import type { ChatMessage } from "./api/bindings";

const KEY = "localaide-branches";
// One branch tail beyond this is skipped: local storage is a few
// megabytes at best, and a lost branch is better than a full store.
const MAX_TAIL_CHARS = 400_000;

type BranchStore = Record<string, Record<string, { messages: ChatMessage[] }>>;

const readFromDisk = (): BranchStore => {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "{}") as BranchStore;
  } catch {
    return {};
  }
};

let store = $state<BranchStore>(readFromDisk());

const syncToDisk = (): void => {
  try {
    localStorage.setItem(KEY, JSON.stringify(store));
  } catch {
    // Storage full or unavailable: branches are a nicety, not a promise.
  }
};

/** Parks the tail that an edit is about to discard, for later restoring. */
export function saveBranch(threadId: string, index: number, tail: ChatMessage[]): void {
  if (!threadId || tail.length === 0) return;
  if (JSON.stringify(tail).length > MAX_TAIL_CHARS) return;
  if (!store[threadId]) store[threadId] = {};
  store[threadId][String(index)] = { messages: tail };
  syncToDisk();
}

/** How many later messages are parked at this message's checkpoint. */
export function branchCount(threadId: string, index: number): number {
  if (!threadId) return 0;
  return store[threadId]?.[String(index)]?.messages.length ?? 0;
}

/**
 * Swaps the branch in: the parked tail comes back, and the chat's current
 * tail takes its place — so flipping restores the old path and keeps the
 * new one a click away.
 */
export function swapBranch(threadId: string, index: number, currentTail: ChatMessage[]): ChatMessage[] | null {
  if (!threadId) return null;
  const parked = store[threadId]?.[String(index)];
  if (!parked || parked.messages.length === 0) return null;
  if (JSON.stringify(currentTail).length <= MAX_TAIL_CHARS) {
    store[threadId][String(index)] = { messages: currentTail };
  }
  syncToDisk();
  return parked.messages;
}

export function clearThreadBranches(threadId: string): void {
  if (!threadId) return;
  if (store[threadId]) {
    delete store[threadId];
    syncToDisk();
  }
}
