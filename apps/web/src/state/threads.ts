import { api, type ChatThread } from "@goat/client-runtime/localaide";
import { createStore, useStore } from "./store";

/**
 * The list of saved conversations.
 *
 * The backend keeps one JSON file per conversation under the app's home
 * directory; this store holds the list the sidebar draws and refreshes it
 * when a conversation is written, renamed or deleted.
 */
export type ThreadsState = {
  threads: ChatThread[];
  loading: boolean;
  error: string | null;
};

export const threadsStore = createStore<ThreadsState>({
  threads: [],
  loading: false,
  error: null,
});

/** Reads the saved conversations from the backend. */
export async function refreshThreads(): Promise<ChatThread[]> {
  threadsStore.set((state) => ({ ...state, loading: true, error: null }));
  try {
    const threads = await api.listChatThreads();
    threadsStore.set({ threads, loading: false, error: null });
    return threads;
  } catch (error) {
    const message =
      error && typeof error === "object" && "message" in error
        ? String((error as { message?: unknown }).message)
        : "The conversations could not be read.";
    threadsStore.set((state) => ({ ...state, loading: false, error: message }));
    return [];
  }
}

/** Removes a conversation and refreshes the list. */
export async function deleteThread(threadId: string): Promise<void> {
  await api.deleteChatThread(threadId);
  await refreshThreads();
}

export function useThreads(): ChatThread[] {
  return useStore(threadsStore, (state) => state.threads);
}

export function useThreadsLoading(): boolean {
  return useStore(threadsStore, (state) => state.loading);
}

export function useThreadsError(): string | null {
  return useStore(threadsStore, (state) => state.error);
}
