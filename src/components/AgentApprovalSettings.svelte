<script lang="ts">
  // Settings → Tools and MCP: how the agent works and how long approvals
  // wait. The mode is the same saved state as the chat box's permission
  // icon (chatModes.ts); the timers are saved in config.json.
  import { onMount } from "svelte";
  import SettingsGroup from "./SettingsGroup.svelte";
  import SettingsRow from "./SettingsRow.svelte";
  import Tabs from "./Tabs.svelte";
  import { errorMessage } from "../lib/api/client";
  import { onSharedApprovalMode, permissionLabel, readSavedApprovalMode, shareApprovalMode, type PermissionModeInfo } from "../lib/chatModes";
  import { approvalTimers, loadApprovalTimers, saveApprovalTimers } from "../lib/approvalTimers.svelte";
  import { describeTimer, PLAN_TIMER_CHOICES, TERMINAL_TIMER_CHOICES } from "../lib/countdown";

  let mode = $state<PermissionModeInfo["id"]>(readSavedApprovalMode() ?? "ask");
  let busy = $state(false);
  let notice = $state<string | null>(null);

  const modeTabs = [
    { value: "plan", label: "Plan mode" },
    { value: "smart", label: "Autonomous mode" },
  ];
  const otherMode = $derived(mode !== "plan" && mode !== "smart");

  onMount(() => {
    void loadApprovalTimers();
    return onSharedApprovalMode((next) => (mode = next));
  });

  const pickMode = (value: string) => {
    if (value !== "plan" && value !== "smart") return;
    mode = value;
    shareApprovalMode(value);
  };

  const saveTimer = async (which: "plan" | "terminal", value: string) => {
    const seconds = Number(value);
    if (!Number.isFinite(seconds)) return;
    busy = true;
    notice = null;
    try {
      await saveApprovalTimers(which === "plan" ? { plan: seconds } : { terminal: seconds });
    } catch (e: unknown) {
      notice = `The timer could not be saved: ${errorMessage(e)}`;
    } finally {
      busy = false;
    }
  };
</script>

<section class="mt-10" aria-labelledby="agent-approvals-title">
  <h2 id="agent-approvals-title" class="text-xl font-semibold text-ink-50">Agent and approvals</h2>
  <p class="mt-1 text-sm text-ink-400">How the agent works in chat, and how long it waits for you when it asks.</p>
  {#if notice}<p class="mt-3 rounded-md bg-rose-500/10 p-2 text-sm text-rose-300" role="alert">{notice}</p>{/if}
  <div class="mt-4">
    <SettingsGroup>
      <SettingsRow
        icon="lightbulb"
        title="Agent mode"
        description={otherMode
          ? `Now: ${permissionLabel(mode)}, set from the chat box. Plan mode plans before editing; Autonomous mode works and asks only when there is risk.`
          : "Plan mode plans before editing. Autonomous mode works and asks only when there is risk. Applies to every chat, and the chat box's permission icon can still change it."}
      >
        {#snippet control()}
          <Tabs items={modeTabs} value={mode} onChange={pickMode} ariaLabel="Agent mode" />
        {/snippet}
      </SettingsRow>
      <SettingsRow
        icon="clock"
        title="Plan auto-accept"
        description={approvalTimers.plan > 0
          ? `A finished plan is built after ${describeTimer(approvalTimers.plan, "")} unless you choose Keep planning. The countdown holds while you type.`
          : "Off. A finished plan waits until you choose Build or Keep planning."}
        class="border-t border-ink-800"
      >
        {#snippet control()}
          <Tabs items={PLAN_TIMER_CHOICES} value={String(approvalTimers.plan)} onChange={(v) => { if (!busy) void saveTimer("plan", v); }} ariaLabel="Plan auto-accept" />
        {/snippet}
      </SettingsRow>
      <SettingsRow
        icon="terminal"
        title="Terminal commands run after"
        description={approvalTimers.terminal > 0
          ? `When the agent asks to run a command, it runs after ${describeTimer(approvalTimers.terminal, "")} unless you choose Don't run. The countdown shows on the request and holds while the chat is not on screen.`
          : "Wait for me. A command the agent asks to run waits until you choose."}
        class="border-t border-ink-800"
      >
        {#snippet control()}
          <Tabs items={TERMINAL_TIMER_CHOICES} value={String(approvalTimers.terminal)} onChange={(v) => { if (!busy) void saveTimer("terminal", v); }} ariaLabel="Terminal commands run after" />
        {/snippet}
      </SettingsRow>
    </SettingsGroup>
  </div>
</section>
