<script lang="ts">
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { slide } from "svelte/transition";
  import Button from "../components/Button.svelte";
  import ConfirmDialog from "../components/ConfirmDialog.svelte";
  import ErrorBanner from "../components/ErrorBanner.svelte";
  import Icon from "../components/Icon.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import SettingsGroup from "../components/SettingsGroup.svelte";
  import SettingsRow from "../components/SettingsRow.svelte";
  import StatusBadge from "../components/StatusBadge.svelte";
  import Tabs from "../components/Tabs.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import { openInBrowser } from "../lib/links";
  import { dur } from "../lib/motion";
  import { applyTheme, currentTheme } from "../lib/theme";
  import type {
    BackupConfigDto,
    BackupEntry,
    CloudConfigStatus,
    CloudProviderPreset,
    SystemHealthResponse,
    ToolAuditEntry,
    UpdateCheckResponse,
    WebProviderStatus,
  } from "../lib/api/bindings";

  type SettingsSection = "general" | "online" | "web" | "privacy" | "backups" | "activity" | "about";
  type Confirmation = {
    title: string;
    message: string;
    confirmLabel: string;
    action: () => void | Promise<void>;
  };

  const sections: { id: SettingsSection; label: string; icon: "sun" | "cloud" | "globe" | "lock" | "database" | "file" | "info" }[] = [
    { id: "general", label: "General", icon: "sun" },
    { id: "online", label: "Online providers", icon: "cloud" },
    { id: "web", label: "Web search", icon: "globe" },
    { id: "privacy", label: "Privacy & files", icon: "lock" },
    { id: "backups", label: "Backups", icon: "database" },
    { id: "activity", label: "Activity log", icon: "file" },
    { id: "about", label: "About", icon: "info" },
  ];

  const themeTabs = [
    { value: "dark", label: "Dark" },
    { value: "light", label: "Light" },
  ];

  let activeTab = $state<SettingsSection>("general");
  let appVersion = $state("");
  let usableRamGb = $state(0);
  let health = $state<SystemHealthResponse | null>(null);
  let updateInfo = $state<UpdateCheckResponse | null>(null);
  let logLines = $state<string[]>([]);
  let diagBusy = $state(false);
  let updateBusy = $state(false);

  let backupConfig = $state<BackupConfigDto | null>(null);
  let backups = $state<BackupEntry[]>([]);
  let backupTargetInput = $state("");
  let includeKeys = $state(false);
  let backupPass = $state("");
  let restorePass = $state("");
  let restoreConfigFlag = $state(false);
  let selectedBackup = $state<string | null>(null);
  let backupBusy = $state(false);
  let backupNotice = $state<string | null>(null);

  // Percent of memory, after the system reserve, that models may use.
  let ramBudget = $state(70);
  let contextTokens = $state(8192);
  let contextSaveTimer: ReturnType<typeof setTimeout> | null = null;
  let settingsNotice = $state<string | null>(null);

  let audits = $state<ToolAuditEntry[]>([]);
  let loadingAudits = $state(false);
  let clearingAudits = $state(false);

  let cloud = $state<CloudConfigStatus | null>(null);
  let cloudError = $state<string | null>(null);
  let cloudBusy = $state(false);
  let showProviderForm = $state(false);
  let editingProviderId = $state<string | null>(null);
  let providerLabel = $state("");
  let providerUrl = $state("");
  let providerModel = $state("");
  let providerKey = $state("");
  let providerFormError = $state<string | null>(null);

  let webProviders = $state<WebProviderStatus[]>([]);
  let webPresets = $state<CloudProviderPreset[]>([]);
  let appliedPresetId = $state<string | null>(null);
  let webKeyDraft = $state<Record<string, string>>({});
  let webBusy = $state(false);
  let webNotice = $state<string | null>(null);
  const appliedPreset = $derived(webPresets.find((preset) => preset.id === appliedPresetId) ?? null);
  let confirmation = $state<Confirmation | null>(null);

  let theme = $state<"dark" | "light">(currentTheme());

  const setTheme = async (value: "dark" | "light") => {
    theme = value;
    applyTheme(value);
    try {
      await api.setTheme(value);
    } catch (e: unknown) {
      settingsNotice = errorMessage(e);
    }
  };

  const selectSection = (section: SettingsSection) => {
    activeTab = section;
    if (section === "activity") void loadAudits();
    if (section === "backups") void loadBackup();
    if (section === "about") void loadDiagnostics();
  };

  const loadWebProviders = async () => {
    try {
      webProviders = await api.webProviders();
    } catch (e: unknown) {
      console.error(e);
      webProviders = [];
    }
  };

  const labelOf = (id: string) => webProviders.find((provider) => provider.id === id)?.label ?? id;

  const saveWebKey = async (id: string) => {
    webBusy = true;
    webNotice = null;
    try {
      const value = webKeyDraft[id]?.trim() ?? "";
      await api.webSetKey(id, value ? value : null);
      webKeyDraft = { ...webKeyDraft, [id]: "" };
      await loadWebProviders();
      webNotice = `${labelOf(id)} key ${value ? "saved" : "cleared"}.`;
    } catch (e: unknown) {
      webNotice = errorMessage(e);
    } finally {
      webBusy = false;
    }
  };

  const setWebDefault = async (id: string) => {
    webBusy = true;
    webNotice = null;
    try {
      await api.webSetDefault(id);
      await loadWebProviders();
      webNotice = `${labelOf(id)} is now the default search provider.`;
    } catch (e: unknown) {
      webNotice = errorMessage(e);
    } finally {
      webBusy = false;
    }
  };

  const loadPresets = async () => {
    if (webPresets.length) return;
    try {
      webPresets = await api.cloudProviderPresets();
    } catch (e: unknown) {
      console.error(e);
    }
  };

  const applyPreset = () => {
    const preset = webPresets.find((item) => item.id === appliedPresetId);
    if (!preset) return;
    providerLabel = preset.label;
    providerUrl = preset.base_url;
    providerModel = preset.model;
    providerKey = "";
    providerFormError = null;
  };

  const loadAudits = async () => {
    loadingAudits = true;
    try {
      audits = await api.listToolAudits(100);
    } catch (e: unknown) {
      console.error(e);
      audits = [];
    } finally {
      loadingAudits = false;
    }
  };

  const clearAuditsNow = async () => {
    clearingAudits = true;
    try {
      await api.clearToolAudits();
      audits = [];
    } catch (e: unknown) {
      settingsNotice = errorMessage(e);
    } finally {
      clearingAudits = false;
    }
  };

  const clearAudits = () => {
    if (audits.length === 0) return;
    confirmation = {
      title: "Clear the activity log?",
      message: "This removes the saved list of actions. It cannot be undone.",
      confirmLabel: "Clear log",
      action: clearAuditsNow,
    };
  };

  const loadCloud = async () => {
    try {
      cloud = await api.getCloudConfig();
    } catch (e: unknown) {
      console.error(e);
      cloud = null;
    }
  };

  const openAddProvider = () => {
    editingProviderId = null;
    providerLabel = "";
    providerUrl = "";
    providerModel = "";
    providerKey = "";
    providerFormError = null;
    appliedPresetId = null;
    void loadPresets();
    showProviderForm = true;
  };

  const openEditProvider = (id: string) => {
    const provider = cloud?.providers.find((item) => item.id === id);
    if (!provider) return;
    editingProviderId = id;
    providerLabel = provider.label;
    providerUrl = provider.base_url;
    providerModel = provider.model;
    providerKey = "";
    providerFormError = null;
    appliedPresetId = null;
    void loadPresets();
    showProviderForm = true;
  };

  const saveProvider = async () => {
    providerFormError = null;
    cloudBusy = true;
    try {
      cloud = await api.saveCloudProvider({
        id: editingProviderId ?? undefined,
        label: providerLabel,
        base_url: providerUrl,
        model: providerModel,
        api_key: providerKey ? providerKey : undefined,
      });
      showProviderForm = false;
    } catch (e: unknown) {
      providerFormError = errorMessage(e);
    } finally {
      cloudBusy = false;
    }
  };

  const deleteProviderNow = async (id: string) => {
    cloudBusy = true;
    cloudError = null;
    try {
      cloud = await api.deleteCloudProvider(id);
    } catch (e: unknown) {
      cloudError = errorMessage(e);
    } finally {
      cloudBusy = false;
    }
  };

  const deleteProvider = (id: string) => {
    const provider = cloud?.providers.find((item) => item.id === id);
    confirmation = {
      title: "Remove this provider?",
      message: `LocalAide will remove ${provider?.label ?? id} and its saved key from this computer.`,
      confirmLabel: "Remove provider",
      action: () => deleteProviderNow(id),
    };
  };

  const toggleCloud = async () => {
    if (!cloud) return;
    cloudBusy = true;
    cloudError = null;
    try {
      cloud = await api.setCloudMode({ enabled: !cloud.cloud_enabled });
    } catch (e: unknown) {
      cloudError = errorMessage(e);
    } finally {
      cloudBusy = false;
    }
  };

  const activateProvider = async (id: string) => {
    if (!cloud) return;
    cloudBusy = true;
    cloudError = null;
    try {
      cloud = await api.setCloudMode({ enabled: cloud.cloud_enabled, provider_id: id });
    } catch (e: unknown) {
      cloudError = errorMessage(e);
    } finally {
      cloudBusy = false;
    }
  };

  // Memory for models: the Models screen and setup flag models above this share.
  const persistSettings = async () => {
    settingsNotice = null;
    try {
      ramBudget = await api.setModelMemory(ramBudget);
    } catch (e: unknown) {
      settingsNotice = errorMessage(e);
    }
  };

  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  const scheduleSave = () => {
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => void persistSettings(), 500);
  };

  const scheduleContextSave = () => {
    if (contextSaveTimer) clearTimeout(contextSaveTimer);
    contextSaveTimer = setTimeout(async () => {
      try {
        await api.setContext(contextTokens);
      } catch (e: unknown) {
        settingsNotice = errorMessage(e);
      }
    }, 500);
  };

  // Reads only this computer. GitHub is contacted by checkForUpdates alone.
  const loadDiagnostics = async () => {
    diagBusy = true;
    try {
      const [healthResult, logs] = await Promise.all([
        api.getSystemHealth(),
        api.getLogTail(120).catch(() => [] as string[]),
      ]);
      health = healthResult;
      logLines = logs;
    } catch (e: unknown) {
      settingsNotice = errorMessage(e);
    } finally {
      diagBusy = false;
    }
  };

  const checkForUpdates = async () => {
    updateBusy = true;
    try {
      updateInfo = await api.checkForUpdate();
    } catch (e: unknown) {
      settingsNotice = errorMessage(e);
    } finally {
      updateBusy = false;
    }
  };

  const loadBackup = async () => {
    backupBusy = true;
    try {
      backupConfig = await api.getBackupConfig();
      backupTargetInput = backupConfig.target_dir ?? "";
      includeKeys = backupConfig.include_api_keys;
      backups = await api.listBackups(backupTargetInput || undefined);
    } catch (e: unknown) {
      backupNotice = errorMessage(e);
    } finally {
      backupBusy = false;
    }
  };

  const saveBackupConfig = async () => {
    backupBusy = true;
    backupNotice = null;
    try {
      await api.setBackupConfig(backupTargetInput.trim() || null, includeKeys);
      backups = await api.listBackups(backupTargetInput.trim() || undefined);
      backupNotice = "Backup settings saved.";
    } catch (e: unknown) {
      backupNotice = errorMessage(e);
    } finally {
      backupBusy = false;
    }
  };

  const runBackup = async () => {
    if (backupPass.length < 8) {
      backupNotice = "The passphrase needs at least 8 characters.";
      return;
    }
    backupBusy = true;
    backupNotice = null;
    try {
      const result = await api.runBackup(backupPass, includeKeys, backupTargetInput.trim() || undefined);
      backupNotice = `Backup written: ${result.path} (${result.memories} notes, ${result.conversations} chats).`;
      backupPass = "";
      backups = await api.listBackups(backupTargetInput.trim() || undefined);
    } catch (e: unknown) {
      backupNotice = errorMessage(e);
    } finally {
      backupBusy = false;
    }
  };

  const runRestore = async () => {
    if (!selectedBackup) return;
    if (!restorePass) {
      backupNotice = "Enter the backup passphrase to restore.";
      return;
    }
    backupBusy = true;
    backupNotice = null;
    try {
      const report = await api.restoreBackup(selectedBackup, restorePass, restoreConfigFlag, backupTargetInput.trim() || undefined);
      backupNotice = `Restored ${report.memories_restored} notes and ${report.conversations_restored} chats.` +
        (report.config_restored ? " Config restored." : "") +
        (report.skipped_newer_config ? " A newer config was kept." : "");
      restorePass = "";
    } catch (e: unknown) {
      backupNotice = errorMessage(e);
    } finally {
      backupBusy = false;
    }
  };

  const browseBackupFolder = async () => {
    try {
      const picked = await api.pickFolder();
      if (picked) backupTargetInput = picked;
    } catch (e: unknown) {
      backupNotice = errorMessage(e);
    }
  };

  const runConfirmation = async () => {
    const request = confirmation;
    confirmation = null;
    if (request) await request.action();
  };

  const contextWords = () => Math.round(contextTokens / 1.365).toLocaleString();
  const modelMemory = () => usableRamGb > 0 ? `${(usableRamGb * ramBudget / 100).toFixed(1)} GB` : `${ramBudget}%`;

  onMount(async () => {
    // A setting that fails to load shows a message, so it never looks empty.
    const failed: string[] = [];
    try {
      ramBudget = await api.getModelMemory();
    } catch (e: unknown) {
      failed.push(`memory for models (${errorMessage(e)})`);
    }
    try {
      contextTokens = await api.getContext();
    } catch (e: unknown) {
      failed.push(`conversation memory (${errorMessage(e)})`);
    }
    try {
      appVersion = await getVersion();
    } catch (e: unknown) {
      console.error(e);
    }
    try {
      const probe = await api.systemProbe();
      // Models get a share of memory after 2 GB is kept for the system.
      usableRamGb = Math.max(probe.total_ram_gb - 2, 0);
    } catch (e: unknown) {
      console.error(e);
    }
    try {
      const stored = await api.getTheme();
      if (stored === "light" || stored === "dark") {
        theme = stored;
        applyTheme(stored);
      }
    } catch (e: unknown) {
      failed.push(`theme (${errorMessage(e)})`);
    }
    if (failed.length > 0) {
      settingsNotice = `Some settings could not load: ${failed.join(", ")}.`;
    }
    await loadCloud();
    await loadWebProviders();
  });
</script>

<div class="mx-auto max-w-5xl pb-10">
  <PageHeader title="Settings" description="Change how LocalAide looks, stores files and uses the internet." />
  <div class="grid gap-8 lg:grid-cols-[220px_minmax(0,1fr)]">
    <nav class="flex flex-col gap-1" aria-label="Settings sections">
      <span class="sr-only">Settings sections</span>
      {#each sections as section (section.id)}
        <button
          type="button"
          aria-current={activeTab === section.id ? "page" : undefined}
          onclick={() => selectSection(section.id)}
          class={`relative flex h-9 items-center gap-3 rounded-md px-3 text-left text-sm transition-colors ${activeTab === section.id
            ? "bg-ink-800 font-semibold text-ink-50"
            : "text-ink-300 hover:bg-veil/5 hover:text-ink-50"}`}
        >
          {#if activeTab === section.id}<span class="absolute left-0 top-2.5 h-4 w-[3px] rounded-full bg-ember-500" aria-hidden="true"></span>{/if}
          <Icon name={section.icon} class="h-[18px] w-[18px]" />
          {section.label}
        </button>
      {/each}
    </nav>

    <div class="min-w-0">
      {#if settingsNotice}
        <div class="mb-4"><ErrorBanner message={settingsNotice} onRetry={() => selectSection(activeTab)} onDismiss={() => settingsNotice = null} /></div>
      {/if}

      {#if activeTab === "general"}
        <div>
          <h2 class="text-xl font-semibold text-ink-50">General</h2>
          <p class="mt-1 text-sm text-ink-400">Change how LocalAide looks and uses this computer.</p>

          <h3 class="mb-2 mt-7 text-[15px] font-semibold text-ink-50">Appearance</h3>
          <SettingsGroup>
            <SettingsRow icon="sun" title="Theme" description="Dark is the default. Your choice is kept when you restart.">
              {#snippet control()}
                <Tabs items={themeTabs} value={theme} onChange={(value) => { if (value === "dark" || value === "light") void setTheme(value); }} ariaLabel="Theme" />
              {/snippet}
            </SettingsRow>
          </SettingsGroup>

          <h3 class="mb-2 mt-7 text-[15px] font-semibold text-ink-50">Performance</h3>
          <SettingsGroup>
            <SettingsRow icon="models" title="Memory for models" description="How much of this computer's memory the AI may use. More lets you run bigger models.">
              {#snippet control()}
                <input type="range" min="20" max="90" bind:value={ramBudget} oninput={scheduleSave} class="w-44 accent-ember-600" aria-label="Memory for models" />
                <span class="min-w-20 text-right text-sm tabular-nums text-ink-200">{modelMemory()}</span>
              {/snippet}
            </SettingsRow>
            <SettingsRow icon="chat" title="Conversation memory" description="How much of a chat the AI keeps in mind. Longer uses more memory." class="border-t border-ink-800">
              {#snippet control()}
                <input type="range" min="512" max="131072" step="512" bind:value={contextTokens} oninput={scheduleContextSave} class="w-44 accent-ember-600" aria-label="Conversation memory" />
                <span class="min-w-28 text-right text-sm tabular-nums text-ink-200">About {contextWords()} words</span>
              {/snippet}
            </SettingsRow>
          </SettingsGroup>

          <h3 class="mb-2 mt-7 text-[15px] font-semibold text-ink-50">Where chats run</h3>
          <SettingsGroup>
            <SettingsRow icon="computer" title="On this computer only" description={cloud?.cloud_enabled ? "Turn off online chat to keep messages here." : "Nothing you type is sent anywhere."}>
              {#snippet control()}
                <StatusBadge label={cloud?.cloud_enabled ? "Off" : "On"} tone={cloud?.cloud_enabled ? "idle" : "ok"} dot={!cloud?.cloud_enabled} />
              {/snippet}
            </SettingsRow>
            <SettingsRow icon="cloud" title="Use an online provider" description="Send chats to a service you choose, with your own key. It stays off until you turn it on." class="border-t border-ink-800">
              {#snippet control()}
                {#if !cloud || cloud.providers.length === 0}
                  <Button variant="secondary" onclick={() => { activeTab = "online"; openAddProvider(); }}>Add provider</Button>
                {:else}
                  <button
                    type="button"
                    role="switch"
                    aria-checked={cloud.cloud_enabled}
                    aria-label="Use online provider for chat"
                    onclick={() => void toggleCloud()}
                    disabled={cloudBusy}
                    class={`relative h-5 w-10 rounded-full border transition-colors disabled:opacity-50 ${cloud.cloud_enabled ? "border-ember-600 bg-ember-600" : "border-edge bg-ink-800"}`}
                  >
                    <span class={`absolute top-0.5 h-3 w-3 rounded-full transition-transform ${cloud.cloud_enabled ? "translate-x-5 bg-on-ember" : "left-1 bg-ink-300"}`}></span>
                  </button>
                {/if}
              {/snippet}
            </SettingsRow>
          </SettingsGroup>
          <p class="mt-3 text-sm text-ink-400">Online chat is off by default. When it is on, messages go only to the provider you choose.</p>
        </div>
      {:else if activeTab === "online"}
        <div>
          <h2 class="text-xl font-semibold text-ink-50">Online providers</h2>
          <p class="mt-1 text-sm text-ink-400">Optional cloud chat with a provider you choose.</p>

          {#if cloudError}<div class="mt-4"><ErrorBanner message={cloudError} onRetry={() => void loadCloud()} onDismiss={() => cloudError = null} /></div>{/if}
          <div class="mt-6 rounded-lg border border-amber-500/30 bg-amber-500/10 p-4 text-sm leading-6 text-amber-200">
            When this is on, your chat messages go to the provider below. When it is off, nothing leaves this computer.
          </div>

          <h3 class="mb-2 mt-7 text-[15px] font-semibold text-ink-50">Your providers</h3>
          <SettingsGroup>
            <SettingsRow icon="cloud" title="Online chat" description={cloud?.cloud_enabled ? "Online chat is on." : "Online chat is off."}>
              {#snippet control()}
                <button
                  type="button"
                  role="switch"
                  aria-checked={cloud?.cloud_enabled ?? false}
                  aria-label="Use online provider for chat"
                  onclick={() => void toggleCloud()}
                  disabled={cloudBusy || !cloud || cloud.providers.length === 0}
                  class={`relative h-5 w-10 rounded-full border transition-colors disabled:opacity-50 ${cloud?.cloud_enabled ? "border-ember-600 bg-ember-600" : "border-edge bg-ink-800"}`}
                >
                  <span class={`absolute top-0.5 h-3 w-3 rounded-full transition-transform ${cloud?.cloud_enabled ? "translate-x-5 bg-on-ember" : "left-1 bg-ink-300"}`}></span>
                </button>
              {/snippet}
            </SettingsRow>
          </SettingsGroup>

          {#if !cloud || cloud.providers.length === 0}
            <div class="mt-3 rounded-lg border border-ink-700 bg-ink-900 p-5 text-center">
              <p class="text-sm text-ink-400">No provider yet. Everything stays local until you add one.</p>
              <Button variant="primary" class="mt-4" onclick={openAddProvider}><Icon name="plus" class="h-4 w-4" />Add provider</Button>
            </div>
          {:else}
            <div class="mt-3 space-y-2">
              {#each cloud.providers as provider (provider.id)}
                <div class={`rounded-lg border p-4 ${provider.is_active ? "border-ember-500/40 bg-ember-500/5" : "border-ink-700 bg-ink-900"}`}>
                  <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                    <div class="min-w-0">
                      <div class="flex flex-wrap items-center gap-2">
                        <span class="font-semibold text-ink-50">{provider.label}</span>
                        {#if provider.is_active}<StatusBadge label="Selected" tone="ok" dot />{/if}
                      </div>
                      <div class="mt-1 truncate text-sm text-ink-400">{provider.model} · {provider.base_url} · key {provider.api_key_hint}</div>
                    </div>
                    <div class="flex shrink-0 items-center gap-2">
                      {#if !provider.is_active}<Button variant="secondary" disabled={cloudBusy} onclick={() => void activateProvider(provider.id)}>Use this</Button>{/if}
                      <Button variant="subtle" disabled={cloudBusy} onclick={() => openEditProvider(provider.id)}>Edit</Button>
                      <Button variant="danger" disabled={cloudBusy} onclick={() => deleteProvider(provider.id)}>Remove</Button>
                    </div>
                  </div>
                </div>
              {/each}
              <Button variant="secondary" class="mt-2" disabled={cloudBusy} onclick={openAddProvider}><Icon name="plus" class="h-4 w-4" />Add provider</Button>
            </div>
          {/if}

          {#if showProviderForm}
            <div class="mt-4 rounded-lg border border-ink-700 bg-ink-900 p-4" transition:slide={{ duration: dur(150) }}>
              <div class="flex items-center justify-between">
                <h3 class="font-semibold text-ink-50">{editingProviderId ? "Edit provider" : "Add provider"}</h3>
                <Button variant="subtle" ariaLabel="Close provider form" onclick={() => showProviderForm = false}><Icon name="close" class="h-4 w-4" /></Button>
              </div>
              <label class="mt-4 block text-sm text-ink-400">
                Start with a provider preset
                <select bind:value={appliedPresetId} onchange={applyPreset} class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 text-sm text-ink-100 outline-none focus:border-edge">
                  <option value={null}>Choose a provider…</option>
                  {#each webPresets as preset (preset.id)}<option value={preset.id}>{preset.label}</option>{/each}
                </select>
              </label>
              {#if appliedPreset}
                <p class="mt-2 text-sm text-ink-400">{appliedPreset.note} {#if appliedPreset.key_url}<a class="text-ember-300 hover:underline" href={appliedPreset.key_url} rel="noreferrer" onclick={(e) => openInBrowser(e, appliedPreset?.key_url ?? "")}>Get a key</a>{/if}</p>
              {/if}
              <div class="mt-4 grid gap-3 sm:grid-cols-2">
                <label class="text-sm text-ink-400">Name<input bind:value={providerLabel} placeholder="A name for this provider" class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 text-sm text-ink-100 outline-none placeholder:text-ink-400 focus:border-edge" /></label>
                <label class="text-sm text-ink-400">Model<input bind:value={providerModel} placeholder="Model name" class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 text-sm text-ink-100 outline-none placeholder:text-ink-400 focus:border-edge" /></label>
                <label class="text-sm text-ink-400 sm:col-span-2">Provider address<input bind:value={providerUrl} placeholder="https://api.example.com/v1" class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 text-sm text-ink-100 outline-none placeholder:text-ink-400 focus:border-edge" /></label>
                <label class="text-sm text-ink-400 sm:col-span-2">API key {editingProviderId ? "(leave empty to keep the saved key)" : ""}<input type="password" bind:value={providerKey} placeholder="Paste your key" class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 text-sm text-ink-100 outline-none placeholder:text-ink-400 focus:border-edge" /></label>
              </div>
              {#if providerFormError}<div class="mt-3"><ErrorBanner message={providerFormError} onRetry={() => void saveProvider()} onDismiss={() => providerFormError = null} /></div>{/if}
              <p class="mt-3 text-sm text-ink-400">The key stays in this app's local settings and is sent only to the address above.</p>
              <div class="mt-4 flex justify-end gap-2"><Button variant="subtle" onclick={() => showProviderForm = false}>Cancel</Button><Button variant="primary" disabled={cloudBusy} onclick={() => void saveProvider()}>{cloudBusy ? "Saving…" : "Save provider"}</Button></div>
            </div>
          {/if}
        </div>
      {:else if activeTab === "web"}
        <div>
          <h2 class="text-xl font-semibold text-ink-50">Web search</h2>
          <p class="mt-1 text-sm text-ink-400">Search and read web pages only when you click. Some providers need a key.</p>
          {#if webNotice}<div class="mt-4 rounded-md border border-ink-700 bg-ink-900 p-3 text-sm text-ink-300">{webNotice}</div>{/if}
          <div class="mt-6 space-y-2">
            {#each webProviders as provider (provider.id)}
              <div class={`rounded-lg border p-4 ${provider.is_default ? "border-ember-500/40 bg-ember-500/5" : "border-ink-700 bg-ink-900"}`}>
                <div class="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
                  <div class="min-w-0">
                    <div class="flex flex-wrap items-center gap-2"><span class="font-semibold text-ink-50">{provider.label}</span>{#if provider.is_default}<StatusBadge label="Default" tone="ok" dot />{/if}</div>
                    <div class="mt-1 text-sm text-ink-400">{provider.needs_key ? (provider.key_set ? "Key saved" : "Needs an API key") : "No key needed"} · {provider.supports}</div>
                  </div>
                  <div class="flex flex-wrap items-center gap-2">
                    {#if provider.needs_key}
                      <input type="password" bind:value={webKeyDraft[provider.id]} placeholder={provider.key_set ? "Replace key" : "Paste API key"} class="h-9 w-44 rounded-md border border-ink-700 bg-ink-950 px-2 text-sm text-ink-100 outline-none placeholder:text-ink-400 focus:border-edge" />
                      <Button variant="secondary" disabled={webBusy} onclick={() => void saveWebKey(provider.id)}>Save key</Button>
                    {/if}
                    {#if provider.supports.includes("search") && !provider.is_default}<Button variant="subtle" disabled={webBusy} onclick={() => void setWebDefault(provider.id)}>Make default</Button>{/if}
                  </div>
                </div>
              </div>
            {:else}
              <div class="rounded-lg border border-ink-700 bg-ink-900 p-5 text-sm text-ink-400">The provider list is not ready. Restart the app and try again.</div>
            {/each}
          </div>
          <p class="mt-4 text-sm text-ink-400">Keys stay in this app's local settings and are sent only to their own provider. Backups leave them out unless you choose to include them.</p>
        </div>
      {:else if activeTab === "privacy"}
        <div>
          <h2 class="text-xl font-semibold text-ink-50">Privacy & files</h2>
          <p class="mt-1 text-sm text-ink-400">See where LocalAide keeps files and what it can access.</p>
          <h3 class="mb-2 mt-7 text-[15px] font-semibold text-ink-50">Workspace access</h3>
          <SettingsGroup>
            <SettingsRow icon="lock" title="Safe workspace" description="File tools stay inside the LocalAide workspace folder.">
              {#snippet control()}<StatusBadge label="Protected" tone="ok" dot />{/snippet}
            </SettingsRow>
            <SettingsRow icon="folder" title="Workspace folder" description="~/.aiassistant/workspace" class="border-t border-ink-800" />
            <SettingsRow icon="check" title="File changes" description="Writing a file always needs your approval." class="border-t border-ink-800">
              {#snippet control()}<StatusBadge label="Ask first" tone="warn" />{/snippet}
            </SettingsRow>
          </SettingsGroup>

          <h3 class="mb-2 mt-7 text-[15px] font-semibold text-ink-50">Local privacy</h3>
          <SettingsGroup>
            <SettingsRow icon="shield" title="No tracking" description="LocalAide does not send analytics or crash reports.">
              {#snippet control()}<StatusBadge label="On" tone="ok" dot />{/snippet}
            </SettingsRow>
            <SettingsRow icon="download" title="Downloads" description="Downloaded files are checked before they become active." class="border-t border-ink-800">
              {#snippet control()}<StatusBadge label="Checked" tone="ok" dot />{/snippet}
            </SettingsRow>
          </SettingsGroup>
        </div>
      {:else if activeTab === "backups"}
        <div>
          <h2 class="text-xl font-semibold text-ink-50">Backups</h2>
          <p class="mt-1 text-sm text-ink-400">Save your notes, chats and settings in an encrypted local file.</p>
          <div class="mt-6 rounded-lg border border-ink-700 bg-ink-900 p-4">
            <label class="block text-sm text-ink-400" for="backup-folder">Backup folder</label><div class="mt-1 flex gap-2"><input id="backup-folder" bind:value={backupTargetInput} placeholder="Leave empty for the default folder" class="h-10 min-w-0 flex-1 rounded-md border border-ink-700 bg-ink-950 px-3 text-sm text-ink-100 outline-none placeholder:text-ink-400 focus:border-edge" /><Button variant="secondary" class="h-10" disabled={backupBusy} onclick={() => void browseBackupFolder()}>Browse…</Button></div>
            <label class="mt-4 flex items-center gap-3 text-sm text-ink-300"><input type="checkbox" bind:checked={includeKeys} class="accent-ember-600" />Include provider keys in the backup</label>
            <p class="mt-2 text-sm text-ink-400">Keys stay out unless you choose this option.</p>
            <Button variant="secondary" class="mt-4" disabled={backupBusy} onclick={() => void saveBackupConfig()}>Save backup settings</Button>
          </div>
          <div class="mt-4 rounded-lg border border-ink-700 bg-ink-900 p-4">
            <h3 class="font-semibold text-ink-50">Create a backup</h3>
            <div class="mt-3 flex flex-col gap-2 sm:flex-row"><input type="password" bind:value={backupPass} placeholder="Passphrase, at least 8 characters" class="h-10 min-w-0 flex-1 rounded-md border border-ink-700 bg-ink-950 px-3 text-sm text-ink-100 outline-none placeholder:text-ink-400 focus:border-edge" /><Button variant="primary" disabled={backupBusy || backupPass.length < 8} onclick={() => void runBackup()}>{backupBusy ? "Working…" : "Back up now"}</Button></div>
            {#if backupConfig?.last_backup_at}<p class="mt-2 text-sm text-ink-400">Last backup: {new Date(backupConfig.last_backup_at * 1000).toLocaleString()}</p>{/if}
          </div>
          <div class="mt-4 rounded-lg border border-ink-700 bg-ink-900 p-4">
            <h3 class="font-semibold text-ink-50">Restore</h3>
            {#if backups.length === 0}
              <p class="mt-2 text-sm text-ink-400">No backup files are in that folder yet.</p>
            {:else}
              <div class="mt-3 space-y-2">{#each backups as backup (backup.file_name)}<button type="button" onclick={() => selectedBackup = backup.file_name} class={`w-full rounded-md border p-3 text-left text-sm ${selectedBackup === backup.file_name ? "border-ember-500/50 bg-ember-500/10 text-ember-200" : "border-ink-700 text-ink-300 hover:bg-veil/5"}`}>{backup.file_name} · {(backup.size_bytes / 1024).toFixed(0)} KB</button>{/each}</div>
              <div class="mt-3 flex flex-col gap-2 lg:flex-row lg:items-center"><input type="password" bind:value={restorePass} placeholder="Passphrase for the selected backup" class="h-10 min-w-0 flex-1 rounded-md border border-ink-700 bg-ink-950 px-3 text-sm text-ink-100 outline-none placeholder:text-ink-400 focus:border-edge" /><label class="flex items-center gap-2 text-sm text-ink-400"><input type="checkbox" bind:checked={restoreConfigFlag} class="accent-ember-600" />Also restore settings</label><Button variant="secondary" disabled={backupBusy || !selectedBackup || !restorePass} onclick={() => void runRestore()}>Restore</Button></div>
            {/if}
          </div>
          {#if backupNotice}<div class="mt-3 border-l-2 border-ember-500/50 pl-3 text-sm text-ink-300">{backupNotice}</div>{/if}
        </div>
      {:else if activeTab === "activity"}
        <div>
          <h2 class="text-xl font-semibold text-ink-50">Activity log</h2>
          <p class="mt-1 text-sm text-ink-400">See file actions made by LocalAide in your workspace.</p>
          <div class="mt-6 flex flex-wrap items-center justify-between gap-3"><div class="text-sm text-ink-400">{audits.length} saved actions</div><div class="flex gap-2"><Button variant="secondary" disabled={loadingAudits} onclick={() => void loadAudits()}>{loadingAudits ? "Refreshing…" : "Refresh"}</Button><Button variant="danger" disabled={clearingAudits || audits.length === 0} onclick={clearAudits}>Clear log</Button></div></div>
          {#if loadingAudits}
            <div class="mt-3 rounded-lg border border-ink-700 bg-ink-900 p-10 text-center text-sm text-ink-400">Loading the activity log…</div>
          {:else if audits.length === 0}
            <div class="mt-3 rounded-lg border border-ink-700 bg-ink-900 p-10 text-center text-sm text-ink-400">No file actions recorded yet.</div>
          {:else}
            <div class="mt-3 overflow-hidden rounded-lg border border-ink-700 bg-ink-900"><div class="max-h-96 overflow-auto"><table class="w-full text-left text-sm"><thead class="sticky top-0 border-b border-ink-700 bg-ink-950 text-ink-400"><tr><th class="p-3">Time</th><th class="p-3">Action</th><th class="p-3">File</th><th class="p-3">Status</th></tr></thead><tbody>{#each audits as audit}<tr class="border-b border-ink-800 last:border-0 hover:bg-veil/5"><td class="whitespace-nowrap p-3 text-ink-400">{new Date(audit.timestamp * 1000).toLocaleTimeString()}</td><td class="p-3 text-ink-200">{audit.action}</td><td class="max-w-xs truncate p-3 text-ink-400">{audit.path}</td><td class="p-3">{#if audit.status === "success"}<StatusBadge label="Done" tone="ok" />{:else if audit.status === "denied"}<StatusBadge label="Denied" tone="warn" />{:else}<StatusBadge label="Error" tone="idle" />{/if}</td></tr>{/each}</tbody></table></div></div>
          {/if}
        </div>
      {:else if activeTab === "about"}
        <div>
          <h2 class="text-xl font-semibold text-ink-50">About</h2>
          <p class="mt-1 text-sm text-ink-400">LocalAide keeps chat, models and files on this computer by default.</p>
          <div class="mt-6 rounded-lg border border-ink-700 bg-ink-900 p-4"><div class="flex items-center justify-between gap-3"><div><h3 class="font-semibold text-ink-50">LocalAide</h3><p class="mt-1 text-sm text-ink-400">Version {appVersion || "unknown"}</p></div><StatusBadge label="Local by default" tone="ok" dot /></div></div>
          <h3 class="mb-2 mt-7 text-[15px] font-semibold text-ink-50">Updates</h3>
          <div class="rounded-lg border border-ink-700 bg-ink-900 p-4">
            <div class="flex flex-wrap items-center justify-between gap-3"><div class="font-semibold text-ink-50">Release check</div><Button variant="secondary" disabled={updateBusy} onclick={() => void checkForUpdates()}>{updateBusy ? "Checking…" : "Check for updates"}</Button></div>
            {#if updateInfo?.update_available}<div class="mt-3 rounded-md border border-amber-500/30 bg-amber-500/10 p-3 text-sm text-amber-200">Version {updateInfo.latest_version} is available. LocalAide never installs updates by itself. <a href={updateInfo.release_url} class="font-semibold underline" onclick={(e) => openInBrowser(e, updateInfo?.release_url ?? "")}>Open the download page</a></div>{:else}<p class="mt-3 text-sm text-ink-400">{updateInfo ? `You have the latest version, ${updateInfo.current_version}.` : "Not checked yet. LocalAide contacts GitHub only when you press Check for updates."}</p>{/if}
          </div>
          {#if health}
            <h3 class="mb-2 mt-7 text-[15px] font-semibold text-ink-50">Local systems</h3>
            <SettingsGroup>
              <SettingsRow icon="chat" title="Chat model" description={health.engine.loaded ? `${health.engine.active_model ?? "Model loaded"} · ${health.engine.context_size.toLocaleString()} tokens` : "No model loaded"}>
                {#snippet control()}<StatusBadge label={health!.engine.loaded ? "Ready" : "Off"} tone={health!.engine.loaded ? "ok" : "idle"} />{/snippet}
              </SettingsRow>
              <SettingsRow icon="search" title="Meaning search" description={health.embedding.ready ? `${health.embedding.embedded_chunks} of ${health.embedding.total_chunks} files prepared` : "Not running"} class="border-t border-ink-800" />
              <SettingsRow icon="file" title="Documents" description={`${health.library.doc_count} in your library`} class="border-t border-ink-800" />
              <SettingsRow icon="check" title="Settings" description={health.config_ok ? "Saved correctly" : "Could not read settings"} class="border-t border-ink-800" />
            </SettingsGroup>
          {/if}
          <div class="mt-4 rounded-lg border border-ink-700 bg-ink-900 p-4"><div class="flex items-center justify-between gap-3"><h3 class="font-semibold text-ink-50">Local log</h3><Button variant="subtle" disabled={diagBusy} onclick={() => void loadDiagnostics()}>Refresh</Button></div>{#if logLines.length === 0}<p class="mt-3 text-sm text-ink-400">No log output yet.</p>{:else}<pre class="mt-3 max-h-72 overflow-auto rounded-md border border-ink-700 bg-ink-950 p-3 text-sm leading-6 text-ink-300 whitespace-pre-wrap">{logLines.join("\n")}</pre>{/if}</div>
        </div>
      {/if}
    </div>
  </div>
</div>

{#if confirmation}
  <ConfirmDialog
    title={confirmation.title}
    message={confirmation.message}
    confirmLabel={confirmation.confirmLabel}
    onConfirm={() => void runConfirmation()}
    onCancel={() => confirmation = null}
  />
{/if}
