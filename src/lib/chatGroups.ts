// Grouping saved chats for the sidebar. Plain functions, so they can be tested.
import type { ChatThread } from "./api/bindings";

/** The last part of a path, for labels. */
export function folderName(path: string): string {
  const parts = path.replace(/[\\/]+$/, "").split(/[\\/]/);
  return parts.at(-1) || path;
}

export interface FolderGroup {
  path: string;
  name: string;
  threads: ChatThread[];
  /** Newest chat time, for sorting. */
  at: number;
}

const ms = (t: number) => (t < 1e12 ? t * 1000 : t);

/** Chats in project folders (one group per folder, newest first) and plain chats. */
export function groupChats(threads: ChatThread[], extraFolders: string[] = [], query = ""): { folders: FolderGroup[]; plain: ChatThread[] } {
  const q = query.trim().toLowerCase();
  const list = [...threads].filter((t) => !q || t.title.toLowerCase().includes(q)).sort((a, b) => ms(b.updated_at) - ms(a.updated_at));
  const byFolder = new Map<string, FolderGroup>();
  const plain: ChatThread[] = [];
  for (const t of list) {
    if (!t.folder) {
      plain.push(t);
      continue;
    }
    const group = byFolder.get(t.folder) ?? { path: t.folder, name: folderName(t.folder), threads: [], at: 0 };
    group.threads.push(t);
    group.at = Math.max(group.at, ms(t.updated_at));
    byFolder.set(t.folder, group);
  }
  if (!q) {
    for (const f of extraFolders) {
      if (!byFolder.has(f)) byFolder.set(f, { path: f, name: folderName(f), threads: [], at: 0 });
    }
  }
  const folders = [...byFolder.values()].sort((a, b) => b.at - a.at || a.name.localeCompare(b.name));
  return { folders, plain };
}
