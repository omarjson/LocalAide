import { useSyncExternalStore } from "react";

/**
 * The composer's unsent text.
 *
 * Vendored from GOAT (`apps/web/src/composerDraftStore.ts`, MIT). GOAT kept a
 * draft per thread and per draft session, with queues, handoffs and file
 * uploads; LocalAide keeps one draft per thread, so the store is small.
 */

export type ComposerDraft = {
  readonly text: string;
  readonly updatedAt: number;
};

export type ComposerDraftStoreState = {
  /** The draft for a thread, or null when the thread has no unsent text. */
  getDraft: (threadId: string) => ComposerDraft | null;
  /** The draft session for a draft id. LocalAide has no drafts. */
  getDraftSession: (draftId: string) => null;
  setDraft: (threadId: string, text: string) => void;
  clearDraft: (threadId: string) => void;
};

const drafts = new Map<string, ComposerDraft>();
const listeners = new Set<() => void>();

const publish = () => {
  for (const listener of listeners) listener();
};

const store: ComposerDraftStoreState = {
  getDraft: (threadId) => drafts.get(threadId) ?? null,
  getDraftSession: () => null,
  setDraft: (threadId, text) => {
    if (!text.trim()) {
      drafts.delete(threadId);
    } else {
      drafts.set(threadId, { text, updatedAt: Date.now() });
    }
    publish();
  },
  clearDraft: (threadId) => {
    drafts.delete(threadId);
    publish();
  },
};

const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
};

const snapshot = () => drafts;

/** Reads the draft store. The selector runs on every change. */
export function useComposerDraftStore<T>(selector: (store: ComposerDraftStoreState) => T): T {
  useSyncExternalStore(
    subscribe,
    () => snapshot(),
    () => snapshot(),
  );
  return selector(store);
}

/** The draft text for a thread. */
export function useComposerDraft(threadId: string | null): string {
  return useComposerDraftStore((state) => (threadId ? state.getDraft(threadId)?.text ?? "" : ""));
}

/** Stores the draft text for a thread. */
export function setComposerDraft(threadId: string, text: string): void {
  store.setDraft(threadId, text);
}
