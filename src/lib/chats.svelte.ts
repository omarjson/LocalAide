// Saved chats and open chat tabs, shared by the sidebar, the tab strip and
// the chat screen. A tab is one chat being used; several can stream at once
// because each tab mounts its own chat screen. The sidebar lists saved
// threads (grouped by project folder) and asks for a tab through openChat
// and newChat.
import { api, errorMessage } from "./api/client";
import type { ChatThread } from "./api/bindings";

/** One open chat. `id` is null until the chat's first answer is saved. */
export type ChatTab = {
  key: string;
  id: string | null;
  title: string;
  folder: string | null;
  /** The tab's own chat is streaming, shown as a dot on the tab. */
  busy: boolean;
};

let tabCounter = 0;
const nextTabKey = () => `tab_${Date.now()}_${++tabCounter}`;

const emptyTab = (folder: string | null = null): ChatTab => ({
  key: nextTabKey(),
  id: null,
  title: "New chat",
  folder,
  busy: false,
});

export const chats = $state({
  threads: [] as ChatThread[],
  /** The chat on screen, and the folder it works in. */
  activeId: null as string | null,
  activeFolder: null as string | null,
  /** The active tab's chat is streaming. Switching never waits: tabs are independent. */
  busy: false,
  error: null as string | null,
  /** Folders picked for chats, newest first, even before their first message. */
  folders: loadFolders(),
  /** Open chats, left to right, each rendered by its own chat screen. */
  tabs: [] as ChatTab[],
  activeTabKey: null as string | null,
});

const TABS_KEY = "localaide-chat-tabs";

const persistTabs = (): void => {
  try {
    localStorage.setItem(
      TABS_KEY,
      JSON.stringify({
        tabs: chats.tabs.map(({ key, id, title, folder }) => ({ key, id, title, folder })),
        active: chats.activeTabKey,
      }),
    );
  } catch {
    // Storage full or unavailable: tabs are a nicety, not a promise.
  }
};

/** Reopens the tabs of the last session, if any survived. */
export function restoreTabs(): boolean {
  try {
    const saved = JSON.parse(localStorage.getItem(TABS_KEY) ?? "{}") as {
      tabs?: { key?: string; id: string | null; title?: string; folder: string | null }[];
      active?: string | null;
    };
    const tabs = (saved.tabs ?? []).slice(0, 8).map((t) => ({
      key: typeof t.key === "string" && t.key ? t.key : nextTabKey(),
      id: t.id ?? null,
      title: t.title ?? "New chat",
      folder: t.folder ?? null,
      busy: false,
    }));
    if (tabs.length === 0) return false;
    chats.tabs = tabs;
    chats.activeTabKey = tabs.some((t) => t.key === saved.active) ? (saved.active as string) : tabs[0].key;
    return true;
  } catch {
    return false;
  }
}

/** One empty tab so the chat area is never bare. */
export function seedTab(folder: string | null = null): void {
  if (chats.tabs.length > 0) return;
  const tab = emptyTab(folder);
  chats.tabs = [tab];
  chats.activeTabKey = tab.key;
  persistTabs();
}

const rememberFolder = (folder: string): void => {
  chats.folders = [folder, ...chats.folders.filter((f) => f !== folder)].slice(0, 20);
  try {
    localStorage.setItem(FOLDERS_KEY, JSON.stringify(chats.folders));
  } catch {
    // Optional; the folder still shows while its chats exist.
  }
};

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

const syncActive = (): void => {
  const tab = chats.tabs.find((t) => t.key === chats.activeTabKey);
  chats.activeId = tab?.id ?? null;
  chats.activeFolder = tab?.folder ?? null;
};

/** Bring a tab forward. */
export function focusTab(key: string): void {
  if (!chats.tabs.some((t) => t.key === key)) return;
  chats.activeTabKey = key;
  syncActive();
  persistTabs();
}

/** Open a chat: its existing tab comes forward, else it gets one. */
export function openChat(thread: ChatThread): void {
  const existing = chats.tabs.find((t) => t.id === thread.id);
  if (existing) {
    if (thread.title) existing.title = thread.title;
    focusTab(existing.key);
    return;
  }
  const tab: ChatTab = {
    key: nextTabKey(),
    id: thread.id,
    title: thread.title || "Chat",
    folder: thread.folder ?? null,
    busy: false,
  };
  chats.tabs = [...chats.tabs, tab];
  chats.activeTabKey = tab.key;
  if (thread.folder) rememberFolder(thread.folder);
  syncActive();
  persistTabs();
}

/** Start a chat in its own new tab; it can stream while other tabs do. */
export function newChat(folder: string | null = null): void {
  if (folder) rememberFolder(folder);
  const tab = emptyTab(folder);
  chats.tabs = [...chats.tabs, tab];
  chats.activeTabKey = tab.key;
  syncActive();
  persistTabs();
}

/** The chat screen reports its first saved id (and later re-mints). */
export function tabThreadStarted(key: string, id: string): void {
  const tab = chats.tabs.find((t) => t.key === key);
  if (!tab) return;
  tab.id = id;
  if (tab.title === "New chat") tab.title = "Chat";
  syncActive();
  persistTabs();
}

export function tabTitleChanged(key: string, title: string): void {
  const tab = chats.tabs.find((t) => t.key === key);
  if (!tab || !title || title === "New Conversation") return;
  tab.title = title;
  persistTabs();
}

export function tabBusyChanged(key: string, busy: boolean): void {
  const tab = chats.tabs.find((t) => t.key === key);
  if (!tab) return;
  tab.busy = busy;
  syncActive();
}

/** Close a tab. The chat itself stays saved; closing is not deleting. */
export function closeTab(key: string): void {
  const index = chats.tabs.findIndex((t) => t.key === key);
  if (index < 0) return;
  chats.tabs = chats.tabs.filter((t) => t.key !== key);
  if (chats.activeTabKey === key) {
    const next = chats.tabs[Math.min(index, chats.tabs.length - 1)];
    chats.activeTabKey = next?.key ?? null;
  }
  syncActive();
  persistTabs();
}

/** Chats were deleted; their tabs go with them. */
export function closeTabsForThreads(ids: string[]): void {
  if (ids.length === 0) return;
  chats.tabs = chats.tabs.filter((t) => t.id === null || !ids.includes(t.id));
  if (!chats.tabs.some((t) => t.key === chats.activeTabKey)) {
    chats.activeTabKey = chats.tabs.at(-1)?.key ?? null;
  }
  syncActive();
  persistTabs();
}

export async function deleteChat(threadId: string): Promise<void> {
  try {
    await api.deleteChatThread(threadId);
    closeTabsForThreads([threadId]);
  } catch (e: unknown) {
    chats.error = `The chat could not be deleted: ${errorMessage(e)}`;
  }
}

/** Ask for a project folder, then start a chat that works in it. */
export async function newChatInFolder(): Promise<boolean> {
  const folder = await api.pickFolder();
  if (!folder) return false;
  newChat(folder);
  return true;
}

export { folderName, groupChats, type FolderGroup } from "./chatGroups";

const FOLDERS_KEY = "localaide-chat-folders";

function loadFolders(): string[] {
  try {
    const value = JSON.parse(localStorage.getItem(FOLDERS_KEY) ?? "[]") as unknown;
    return Array.isArray(value) ? value.filter((v): v is string => typeof v === "string").slice(0, 20) : [];
  } catch {
    return [];
  }
}
