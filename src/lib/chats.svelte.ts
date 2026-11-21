// Saved chats, shared by the sidebar and the chat screen. The sidebar
// lists them (grouped by project folder) and asks the chat screen to open
// one or start a new one through `chatRequest`.
import { api, errorMessage } from "./api/client";
import type { ChatThread } from "./api/bindings";

export type ChatAction = { kind: "open"; thread: ChatThread } | { kind: "new"; folder: string | null } | { kind: "deleted"; id: string };

export const chats = $state({
  threads: [] as ChatThread[],
  /** The chat on screen, and the folder it works in. */
  activeId: null as string | null,
  activeFolder: null as string | null,
  /** An answer is streaming, so switching chats waits. */
  busy: false,
  error: null as string | null,
  /** Folders picked for chats, newest first, even before their first message. */
  folders: loadFolders(),
});

export const chatRequest = $state({ tick: 0, action: null as ChatAction | null });

const FOLDERS_KEY = "localaide-chat-folders";

function loadFolders(): string[] {
  try {
    const value = JSON.parse(localStorage.getItem(FOLDERS_KEY) ?? "[]") as unknown;
    return Array.isArray(value) ? value.filter((v): v is string => typeof v === "string").slice(0, 20) : [];
  } catch {
    return [];
  }
}

function rememberFolder(folder: string): void {
  chats.folders = [folder, ...chats.folders.filter((f) => f !== folder)].slice(0, 20);
  try {
    localStorage.setItem(FOLDERS_KEY, JSON.stringify(chats.folders));
  } catch {
    // Optional; the folder still shows while its chats exist.
  }
}

export function forgetFolder(folder: string): void {
  chats.folders = chats.folders.filter((f) => f !== folder);
  try {
    localStorage.setItem(FOLDERS_KEY, JSON.stringify(chats.folders));
  } catch {
    // Optional.
  }
}

export async function loadThreads(): Promise<void> {
  try {
    chats.threads = await api.listChatThreads();
    chats.error = null;
  } catch (e: unknown) {
    chats.error = `Saved chats could not load: ${errorMessage(e)}`;
  }
}

const ask = (action: ChatAction) => {
  chatRequest.action = action;
  chatRequest.tick += 1;
};

export function openChat(thread: ChatThread): void {
  ask({ kind: "open", thread });
}

export function newChat(folder: string | null = null): void {
  if (folder) rememberFolder(folder);
  ask({ kind: "new", folder });
}

/** Ask for a project folder, then start a chat that works in it. */
export async function newChatInFolder(): Promise<boolean> {
  const folder = await api.pickFolder();
  if (!folder) return false;
  newChat(folder);
  return true;
}

export async function deleteChat(id: string): Promise<void> {
  await api.deleteChatThread(id);
  chats.threads = chats.threads.filter((t) => t.id !== id);
  ask({ kind: "deleted", id });
}

export { folderName, groupChats, type FolderGroup } from "./chatGroups";
