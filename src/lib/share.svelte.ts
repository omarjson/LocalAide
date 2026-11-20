// Whether the assistant is shared right now, for the "Sharing is on" state
// shown everywhere in the app while a link works.
import { api } from "./api/client";
import type { ShareStatus } from "./api/bindings";

export const share = $state<{ status: ShareStatus | null }>({ status: null });

let timer: ReturnType<typeof setInterval> | null = null;

export async function refreshShare(): Promise<void> {
  try {
    share.status = await api.shareStatus();
  } catch (e: unknown) {
    console.error("Share status could not load", e);
  }
  // Poll only while sharing, to keep request counts current.
  if (share.status?.on && !timer) timer = setInterval(() => void refreshShare(), 5000);
  if (!share.status?.on && timer) {
    clearInterval(timer);
    timer = null;
  }
}

export function setShare(status: ShareStatus): void {
  share.status = status;
  void refreshShare();
}
