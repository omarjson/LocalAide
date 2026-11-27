<script lang="ts">
  // Automations: prompts the assistant answers on a schedule. A run is a
  // normal answer, just one nobody is watching — the feed keeps the last
  // 50 so you can read them when you are.
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { slide } from "svelte/transition";
  import Button from "../components/Button.svelte";
  import ConfirmDialog from "../components/ConfirmDialog.svelte";
  import Icon from "../components/Icon.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import Switch from "../components/Switch.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import { dur } from "../lib/motion";
  import type { Automation, AutomationsState } from "../lib/api/bindings";

  let automations = $state<Automation[]>([]);
  let runs = $state<AutomationsState["runs"]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let editing = $state<Automation | null>(null);
  let pendingDelete = $state<Automation | null>(null);
  let saving = $state(false);
  let runningId = $state<string | null>(null);

  // The form
  let formId = $state<string | null>(null);
  let formName = $state("");
  let formPrompt = $state("");
  let formKind = $state<"daily" | "hourly" | "every">("daily");
  let formHour = $state(7);
  let formMinute = $state(0);
  let formInterval = $state(60);
  let formEnabled = $state(true);

  const load = async () => {
    try {
      const state = await api.automationsList();
      automations = state.automations;
      runs = state.runs;
      error = null;
    } catch (e: unknown) {
      error = errorMessage(e);
    } finally {
      loading = false;
    }
  };

  onMount(() => {
    void load();
    // The scheduler and Run-now buttons push updates on this event.
    let stop: (() => void) | undefined;
    listen<AutomationsState["runs"]>("automations-changed", (event) => {
      runs = event.payload ?? [];
      void load();
    }).then((unlisten) => {
      stop = unlisten;
    }).catch(() => undefined);
    return () => stop?.();
  });

  const scheduleLabel = (a: Automation): string => {
    if (a.kind === "daily") {
      const hh = String(a.hour ?? 7).padStart(2, "0");
      const mm = String(a.minute ?? 0).padStart(2, "0");
      return `Every day at ${hh}:${mm}`;
    }
    if (a.kind === "hourly") return "Every hour";
    return `Every ${a.interval_minutes ?? 60} minutes`;
  };

  const sinceLabel = (unix: number): string => {
    const diff = Math.max(0, Date.now() / 1000 - unix);
    if (diff < 60) return "just now";
    if (diff < 3600) return `${Math.floor(diff / 60)} min ago`;
    if (diff < 86_400) return `${Math.floor(diff / 3600)} h ago`;
    return `${Math.floor(diff / 86_400)} d ago`;
  };

  const openNew = () => {
    formId = null;
    formName = "";
    formPrompt = "";
    formKind = "daily";
    formHour = 7;
    formMinute = 0;
    formInterval = 60;
    formEnabled = true;
    editing = { id: "", name: "", prompt: "", kind: "daily", hour: 7, minute: 0, interval_minutes: 60, enabled: true, created_at: 0, last_run_at: null };
  };

  const openEdit = (a: Automation) => {
    formId = a.id;
    formName = a.name;
    formPrompt = a.prompt;
    formKind = (a.kind as "daily" | "hourly" | "every") ?? "daily";
    formHour = a.hour ?? 7;
    formMinute = a.minute ?? 0;
    formInterval = a.interval_minutes ?? 60;
    formEnabled = a.enabled;
    editing = a;
  };

  const save = async () => {
    saving = true;
    error = null;
    try {
      const state = await api.automationsSave({
        id: formId ?? "",
        name: formName,
        prompt: formPrompt,
        kind: formKind,
        hour: formKind === "daily" ? formHour : null,
        minute: formMinute,
        interval_minutes: formKind === "every" ? formInterval : null,
        enabled: formEnabled,
        created_at: 0,
        last_run_at: null,
      });
      automations = state.automations;
      runs = state.runs;
      editing = null;
      notice = formId ? "Automation updated." : "Automation scheduled.";
    } catch (e: unknown) {
      error = errorMessage(e);
    } finally {
      saving = false;
    }
  };

  const remove = async () => {
    const target = pendingDelete;
    pendingDelete = null;
    if (!target) return;
    try {
      const state = await api.automationsDelete(target.id);
      automations = state.automations;
      runs = state.runs;
    } catch (e: unknown) {
      error = errorMessage(e);
    }
  };

  const toggle = async (a: Automation) => {
    try {
      const state = await api.automationsSave({ ...a, enabled: !a.enabled });
      automations = state.automations;
    } catch (e: unknown) {
      error = errorMessage(e);
    }
  };

  const runNow = async (a: Automation) => {
    runningId = a.id;
    notice = null;
    try {
      await api.automationsRunNow(a.id);
      notice = `${a.name} is running now. Its answer lands in the feed when it finishes.`;
    } catch (e: unknown) {
      error = errorMessage(e);
    } finally {
      runningId = null;
    }
  };
</script>

<div class="mx-auto max-w-4xl pb-10">
  <PageHeader title="Automations" description="Prompts the assistant answers on a schedule — nobody watching, the feed keeps the answers.">
    {#snippet actions()}
      <Button variant="primary" onclick={openNew}><Icon name="plus" class="h-4 w-4" />New automation</Button>
    {/snippet}
  </PageHeader>

  {#if error}
    <div class="mb-4 rounded-md bg-rose-500/10 px-3 py-2 text-sm text-rose-300" role="alert">{error}</div>
  {/if}
  {#if notice}
    <div class="mb-4 rounded-md bg-emerald-500/10 px-3 py-2 text-sm text-emerald-300" transition:slide={{ duration: dur(150) }}>{notice}</div>
  {/if}

  {#if loading}
    <div class="rounded-lg border border-ink-700 bg-ink-900 px-4 py-12 text-center text-sm text-ink-400">Loading…</div>
  {:else if automations.length === 0 && runs.length === 0}
    <div class="rounded-lg border border-ink-700 bg-ink-900 p-8 text-center">
      <Icon name="clock" class="mx-auto h-8 w-8 text-ink-600" />
      <h2 class="mt-3 font-semibold text-ink-50">Nothing scheduled yet</h2>
      <p class="mx-auto mt-1 max-w-md text-sm leading-6 text-ink-400">
        An automation is a prompt the assistant answers on its own — a morning summary, an hourly check, a weekly digest. Runs are read-only: nothing is written while you are away.
      </p>
      <div class="mt-4"><Button variant="primary" onclick={openNew}><Icon name="plus" class="h-4 w-4" />New automation</Button></div>
    </div>
  {:else}
    {#if automations.length > 0}
      <div class="space-y-2">
        {#each automations as a (a.id)}
          <div class={`rounded-lg border border-ink-700 bg-ink-900 p-4 ${a.enabled ? "" : "opacity-70"}`}>
            <div class="flex flex-wrap items-center gap-3">
              <span class="grid h-9 w-9 shrink-0 place-items-center rounded-lg bg-ink-800 text-ink-400"><Icon name="clock" class="h-[18px] w-[18px]" /></span>
              <div class="min-w-0 flex-1">
                <p class="truncate font-semibold text-ink-50">{a.name}</p>
                <p class="truncate text-sm text-ink-400">{scheduleLabel(a)}{a.last_run_at ? ` · last ran ${sinceLabel(a.last_run_at)}` : " · not run yet"}</p>
              </div>
              <Switch checked={a.enabled} label={`Turn ${a.name} ${a.enabled ? "off" : "on"}`} onchange={() => void toggle(a)} />
              <Button variant="subtle" disabled={runningId !== null} onclick={() => void runNow(a)}>{runningId === a.id ? "Running…" : "Run now"}</Button>
              <Button variant="secondary" onclick={() => openEdit(a)}>Edit</Button>
              <button type="button" class="chat-icon-btn" aria-label={`Delete ${a.name}`} onclick={() => (pendingDelete = a)}><Icon name="trash" class="h-4 w-4" /></button>
            </div>
            <p class="mt-2 line-clamp-2 text-sm text-ink-400">{a.prompt}</p>
          </div>
        {/each}
      </div>
    {/if}

    {#if runs.length > 0}
      <h2 class="mb-2 mt-8 text-[15px] font-semibold text-ink-50">Run feed</h2>
      <div class="space-y-2">
        {#each runs as run, i (`${run.automation_id}-${run.at}-${i}`)}
          <details class="rounded-lg border border-ink-700 bg-ink-900 p-4" transition:slide={{ duration: dur(150) }}>
            <summary class="flex cursor-pointer flex-wrap items-center gap-2 list-none">
              <Icon name={run.ok ? "check" : "x-circle"} class={`h-4 w-4 shrink-0 ${run.ok ? "text-emerald-300" : "text-rose-300"}`} />
              <span class="font-medium text-ink-100">{run.name}</span>
              <span class="text-sm text-ink-500">{sinceLabel(run.at)}</span>
              <span class="flex-1"></span>
              <span class="text-xs text-ink-500">{(run.ms / 1000).toFixed(1)} s</span>
            </summary>
            <p class="mt-3 whitespace-pre-wrap text-sm leading-6 text-ink-200">{run.answer || "(no answer came back)"}</p>
          </details>
        {/each}
      </div>
    {/if}
  {/if}
</div>

{#if editing}
  <div class="fixed inset-0 z-50 grid place-items-center bg-black/50 p-4" role="dialog" aria-modal="true" aria-label="Edit automation">
    <div class="w-full max-w-lg rounded-2xl border border-ink-700 bg-ink-950 p-5 shadow-[var(--shadow-elev)]">
      <h2 class="text-lg font-semibold text-ink-50">{formId ? "Edit automation" : "New automation"}</h2>
      <div class="mt-4 space-y-3">
        <label class="block text-sm text-ink-400">Name
          <input bind:value={formName} placeholder="Morning summary" class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 text-sm text-ink-100 placeholder:text-ink-500" />
        </label>
        <label class="block text-sm text-ink-400">The prompt it answers
          <textarea bind:value={formPrompt} rows="3" placeholder="Summarize what changed in my notes this week." class="mt-1 w-full resize-y rounded-md border border-ink-700 bg-ink-950 px-3 py-2 text-sm text-ink-100 placeholder:text-ink-500"></textarea>
        </label>
        <div class="flex flex-wrap items-end gap-3">
          <label class="text-sm text-ink-400">Schedule
            <select bind:value={formKind} class="mt-1 h-10 w-36 rounded-md border border-ink-700 bg-ink-950 px-2 text-sm text-ink-100">
              <option value="daily">Every day at…</option>
              <option value="hourly">Every hour</option>
              <option value="every">Every N minutes</option>
            </select>
          </label>
          {#if formKind === "daily"}
            <label class="text-sm text-ink-400">Time
              <span class="mt-1 flex items-center gap-1">
                <input type="number" min="0" max="23" bind:value={formHour} class="h-10 w-16 rounded-md border border-ink-700 bg-ink-950 px-2 text-sm tabular-nums text-ink-100" aria-label="Hour" />
                <span class="text-ink-400">:</span>
                <input type="number" min="0" max="59" bind:value={formMinute} class="h-10 w-16 rounded-md border border-ink-700 bg-ink-950 px-2 text-sm tabular-nums text-ink-100" aria-label="Minute" />
              </span>
            </label>
          {:else if formKind === "every"}
            <label class="text-sm text-ink-400">Minutes between runs
              <input type="number" min="5" max="10080" bind:value={formInterval} class="mt-1 h-10 w-24 rounded-md border border-ink-700 bg-ink-950 px-2 text-sm tabular-nums text-ink-100" />
            </label>
          {/if}
          <label class="flex h-10 items-center gap-2 text-sm text-ink-400">
            <Switch checked={formEnabled} label="Automation enabled" onchange={(v) => (formEnabled = v)} />
            Enabled
          </label>
        </div>
      </div>
      <div class="mt-5 flex justify-end gap-2">
        <Button variant="subtle" onclick={() => (editing = null)}>Cancel</Button>
        <Button variant="primary" disabled={saving || !formName.trim() || !formPrompt.trim()} onclick={() => void save()}>{saving ? "Saving…" : formId ? "Save changes" : "Schedule"}</Button>
      </div>
    </div>
  </div>
{/if}

{#if pendingDelete}
  <ConfirmDialog
    title="Delete this automation?"
    message={`${pendingDelete.name} stops answering on its schedule. Past runs stay in the feed until the app clears them.`}
    confirmLabel="Delete automation"
    onConfirm={() => void remove()}
    onCancel={() => (pendingDelete = null)}
  />
{/if}
