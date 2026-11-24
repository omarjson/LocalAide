// Approval timers, shared by Settings (writes) and every chat (reads), so a
// change applies to the next prompt without a restart. Saved in config.json.
import { api } from "./api/client";
import { clampTimer, DEFAULT_PLAN_SECS, DEFAULT_TERMINAL_SECS } from "./countdown";

export const approvalTimers = $state<{ plan: number; terminal: number; loaded: boolean }>({
  plan: DEFAULT_PLAN_SECS,
  terminal: DEFAULT_TERMINAL_SECS,
  loaded: false,
});

let loading: Promise<void> | null = null;

/** Load once; on failure the defaults stay (plans wait, commands after 60 s). */
export function loadApprovalTimers(force = false): Promise<void> {
  if (loading && !force) return loading;
  loading = (async () => {
    try {
      const t = await api.getApprovalTimers();
      approvalTimers.plan = clampTimer(t.plan_auto_accept_secs, DEFAULT_PLAN_SECS);
      approvalTimers.terminal = clampTimer(t.terminal_approval_secs, DEFAULT_TERMINAL_SECS);
    } catch (e: unknown) {
      console.warn("Approval timers could not load; using the defaults", e);
    } finally {
      approvalTimers.loaded = true;
    }
  })();
  return loading;
}

export async function saveApprovalTimers(next: { plan?: number; terminal?: number }): Promise<void> {
  const saved = await api.setApprovalTimers({
    plan_auto_accept_secs: next.plan ?? approvalTimers.plan,
    terminal_approval_secs: next.terminal ?? approvalTimers.terminal,
  });
  approvalTimers.plan = clampTimer(saved.plan_auto_accept_secs, DEFAULT_PLAN_SECS);
  approvalTimers.terminal = clampTimer(saved.terminal_approval_secs, DEFAULT_TERMINAL_SECS);
}
