<script lang="ts">
  // Opt-in notifications and a short chime. Everything is local: the Web
  // Notification API, a WebAudio tone, and preferences in localStorage. The
  // switches default to off and nothing is requested until the user asks.
  import { onMount } from "svelte";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import {
    DEFAULT_NOTIFICATION_PREFS,
    fireNotification,
    loadNotificationPrefs,
    notificationPermission,
    requestNotificationPermission,
    saveNotificationPrefs,
    type NotificationKind,
    type NotificationPrefs,
  } from "../lib/notifications.svelte";

  let prefs = $state<NotificationPrefs>({ ...DEFAULT_NOTIFICATION_PREFS });
  let permission = $state<NotificationPermission | "unsupported">("unsupported");
  let notice = $state<{ ok: boolean; text: string } | null>(null);

  const kinds: { key: NotificationKind; label: string; help: string }[] = [
    { key: "goal", label: "Goal completes", help: "A tracked goal is marked done." },
    { key: "turn", label: "Answer finishes while away", help: "A chat turn ends while the window is unfocused." },
    { key: "approval", label: "Tool approval needed", help: "The model waits for your decision." },
    { key: "question", label: "Model asks a question", help: "A question card needs your answer." },
  ];

  onMount(() => {
    prefs = loadNotificationPrefs();
    permission = notificationPermission();
  });

  const persist = () => {
    prefs = saveNotificationPrefs(prefs);
    notice = { ok: true, text: "Notification preferences saved on this device." };
  };

  const enableDesktop = async () => {
    const result = await requestNotificationPermission();
    permission = result;
    if (result === "granted") {
      prefs = saveNotificationPrefs({ ...prefs, enabled: true, desktop: true });
      notice = { ok: true, text: "Desktop notifications are allowed." };
    } else if (result === "unsupported") {
      notice = { ok: false, text: "This window has no notification API; the chime still works." };
    } else {
      notice = { ok: false, text: "Notifications were blocked. The chime can still be used." };
    }
  };

  const test = () => {
    const fired = fireNotification("goal", "LocalAide test", "Notifications are working.");
    notice = fired
      ? { ok: true, text: "Test sent." }
      : { ok: false, text: "Nothing sent — turn notifications on and allow at least one output." };
  };

  const toggle = (key: NotificationKind) => {
    prefs = { ...prefs, [key]: !prefs[key] };
  };
</script>

<section class="mt-10 rounded-xl border border-ink-700 bg-ink-900 p-5" aria-labelledby="notifications-title">
  <div class="flex flex-wrap items-start justify-between gap-3">
    <div>
      <h2 id="notifications-title" class="flex items-center gap-2 text-lg font-semibold text-ink-50">
        <Icon name="info" class="h-5 w-5 text-ink-400" />Notifications and sound
      </h2>
      <p class="mt-1 text-sm text-ink-400">Quiet, local alerts for things that need you. No messages are sent anywhere.</p>
    </div>
    <Button variant="secondary" onclick={test}><Icon name="sparkle" class="h-4 w-4" />Send a test</Button>
  </div>

  {#if notice}
    <p class={`mt-3 rounded-md p-2 text-sm ${notice.ok ? "bg-emerald-500/10 text-emerald-300" : "bg-rose-500/10 text-rose-300"}`} role={notice.ok ? "status" : "alert"}>{notice.text}</p>
  {/if}

  <div class="mt-5 space-y-3">
    <label class="flex cursor-pointer items-start gap-3">
      <input type="checkbox" class="mt-0.5 h-4 w-4 accent-ember-500" bind:checked={prefs.enabled} onchange={persist} />
      <span><span class="block text-sm font-medium text-ink-100">Enable notifications</span><span class="block text-xs text-ink-500">Master switch. Nothing fires while this is off.</span></span>
    </label>
    <label class="flex cursor-pointer items-start gap-3">
      <input type="checkbox" class="mt-0.5 h-4 w-4 accent-ember-500" bind:checked={prefs.desktop} onchange={persist} />
      <span><span class="block text-sm font-medium text-ink-100">Desktop notifications</span><span class="block text-xs text-ink-500">Permission: {permission === "unsupported" ? "not available here" : permission}.</span></span>
    </label>
    <label class="flex cursor-pointer items-start gap-3">
      <input type="checkbox" class="mt-0.5 h-4 w-4 accent-ember-500" bind:checked={prefs.sound} onchange={persist} />
      <span><span class="block text-sm font-medium text-ink-100">Play a short chime</span><span class="block text-xs text-ink-500">A quiet two-note tone from this device's audio.</span></span>
    </label>
  </div>

  {#if permission !== "granted" && permission !== "unsupported"}
    <div class="mt-3"><Button variant="secondary" onclick={() => void enableDesktop()}>Allow desktop notifications</Button></div>
  {/if}

  <h3 class="mt-6 text-sm font-semibold text-ink-200">Notify me when…</h3>
  <div class="mt-2 space-y-2">
    {#each kinds as kind (kind.key)}
      <label class="flex cursor-pointer items-center gap-3 rounded-lg border border-ink-800 bg-ink-950 px-3 py-2">
        <input
          type="checkbox"
          class="h-4 w-4 accent-ember-500"
          checked={prefs[kind.key]}
          onchange={() => { toggle(kind.key); persist(); }}
        />
        <span class="min-w-0"><span class="block text-sm text-ink-100">{kind.label}</span><span class="block text-xs text-ink-500">{kind.help}</span></span>
      </label>
    {/each}
  </div>

  <div class="mt-5 flex justify-end"><Button variant="primary" onclick={persist}>Save preferences</Button></div>
</section>
