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
  import Slider from "../components/Slider.svelte";
  import Switch from "../components/Switch.svelte";
  import ProviderCard from "../components/ProviderCard.svelte";
  import ProviderForm from "../components/ProviderForm.svelte";
  import CloudAuditLog from "../components/CloudAuditLog.svelte";
  import SelfTest from "../components/SelfTest.svelte";
  import VoiceSettings from "../components/VoiceSettings.svelte";
  import ShareSettings from "../components/ShareSettings.svelte";
  import Tabs from "../components/Tabs.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import { formatBytes } from "../lib/format";
  import { openInBrowser } from "../lib/links";
  import { dur } from "../lib/motion";
  import { openLegal } from "../lib/legal.svelte";
  import { settingsRequest, settingsView, type SettingsSectionId } from "../lib/nav.svelte";
  import { dev, flag, moduleOn, resetDev, updateDev } from "../lib/dev.svelte";
  import { clearRequestLog, requestLog } from "../lib/requestLog.svelte";
  import { buildDiagnosticReport, requestStats } from "../lib/diagnostics";
  import { platform } from "../lib/platform";
  import type { EngineInfo, OpenCodeSettings } from "../lib/api/bindings";
  import { scale, setScale } from "../lib/scale.svelte";
  import MarkdownDoc from "../components/MarkdownDoc.svelte";
  import McpSettings from "../components/McpSettings.svelte";
  import McpOAuthPanel from "../components/McpOAuthPanel.svelte";
  import AgentsPanel from "../components/AgentsPanel.svelte";
  import DoctorPanel from "../components/DoctorPanel.svelte";
  import { applyAccent, applyDensity, applyFont, applyTheme, currentAccent, currentDensity, currentFont, currentTheme, type Accent, type AppFont, type Density } from "../lib/theme";
  import { SETTINGS_GROUPS, SETTINGS_GROUP_OF, type SettingsGroupId } from "../lib/screens";
  import GoalPanel from "../components/GoalPanel.svelte";
  import UsagePanel from "../components/UsagePanel.svelte";
  import PolicyPanel from "../components/PolicyPanel.svelte";
  import NotificationsPanel from "../components/NotificationsPanel.svelte";
  import PluginPanels from "../components/PluginPanels.svelte";
  import type {
    BackupConfigDto,
    BackupEntry,
    CloudConfigStatus,
    CloudProviderPreset,
    ContextInfo,
    SystemHealthResponse,
    ToolAuditEntry,
    UpdateCheckResponse,
    UpdateProgressEvent,
    WebProviderStatus,
  } from "../lib/api/bindings";

  type SettingsSection = SettingsSectionId;
  type Confirmation = {
    title: string;
    message: string;
    confirmLabel: string;
    action: () => void | Promise<void>;
  };

  const allSections: { id: SettingsSection; label: string; icon: "sun" | "cloud" | "globe" | "lock" | "database" | "file" | "info" | "plug" | "flask" | "wrench" | "mic" | "link" }[] = [
    { id: "general", label: "General", icon: "sun" },
    { id: "online", label: "Online providers", icon: "cloud" },
    { id: "tools", label: "Tools and MCP", icon: "wrench" },
    { id: "web", label: "Web search", icon: "globe" },
    { id: "voice", label: "Voice", icon: "mic" },
    { id: "share", label: "Share my assistant", icon: "link" },
    { id: "integrations", label: "Integrations", icon: "plug" },
    { id: "privacy", label: "Privacy & files", icon: "lock" },
    { id: "backups", label: "Backups", icon: "database" },
    { id: "activity", label: "Activity log", icon: "file" },
    { id: "developer", label: "Developer", icon: "flask" },
    { id: "about", label: "About", icon: "info" },
  ];
  const sections = $derived(allSections.filter((sec) => (sec.id !== "online" || moduleOn("online")) && (sec.id !== "integrations" || moduleOn("opencode"))));

  // Seven top-level groups in the fixed order from screens.ts. Every existing
  // section stays reachable: picking a section brings its group forward, and
  // deep links still open the exact section they ask for.
  const groupOf = (section: SettingsSection): SettingsGroupId =>
    SETTINGS_GROUP_OF[section] ?? "general";
  let activeGroup = $state<SettingsGroupId>(groupOf(settingsView.section as SettingsSection));
  const groupSections = $derived(sections.filter((sec) => groupOf(sec.id) === activeGroup));

  const selectGroup = (id: SettingsGroupId) => {
    activeGroup = id;
    const first = sections.find((sec) => groupOf(sec.id) === id);
    if (first) selectSection(first.id);
  };

  // Search across sections: type a word ("theme", "voice", "logs") and the
  // section list narrows to what matches. Words a section answers to, beyond
  // its title, live in SECTION_KEYWORDS.
  const SECTION_KEYWORDS: Record<SettingsSection, string> = {
    general: "theme dark light appearance scale font size engine memory ram model context startup language",
    online: "provider cloud api key openai kilo remote internet service",
    tools: "mcp tools servers commands allow deny permissions integrations opencode",
    web: "web search internet duckduckgo brave results",
    voice: "voice speech microphone dictation speak talk listen tts stt whisper",
    share: "share link token invite friend publish assistant",
    integrations: "opencode bridge connect ide agent",
    privacy: "privacy files folders index storage data delete forget local disk",
    backups: "backup restore export import snapshot history",
    activity: "activity log logs diagnostics requests engine output debug",
    developer: "developer flags experimental sampling system prompt raw features",
    about: "about version update check release license credits changelog",
  };

  let settingsQuery = $state("");

  const searchHit = (section: { id: SettingsSection; label: string }) => {
    const q = settingsQuery.trim().toLowerCase();
    if (!q) return false;
    return (
      section.label.toLowerCase().includes(q) ||
      (SECTION_KEYWORDS[section.id] ?? "").includes(q)
    );
  };

  const visibleSections = $derived(
    settingsQuery.trim() ? sections.filter(searchHit) : sections,
  );

  const searchKeydown = (e: KeyboardEvent) => {
    if (e.key === "Escape") {
      settingsQuery = "";
      return;
    }
    if (e.key === "Enter" && settingsQuery.trim()) {
      const first = visibleSections[0];
      if (first) selectSection(first.id);
    }
  };

  // Integrations: OpenCode
  let opencode = $state<OpenCodeSettings | null>(null);
  let opencodeUrl = $state("");
  let opencodePassword = $state("");
  let opencodeBusy = $state(false);
  let opencodeNotice = $state<{ ok: boolean; text: string } | null>(null);

  const loadOpenCode = async () => {
    try {
      opencode = await api.opencodeGet();
      opencodeUrl = opencode.base_url ?? opencode.default_url;
    } catch (e: unknown) {
      opencodeNotice = { ok: false, text: errorMessage(e) };
    }
  };

  const linkOpenCode = async () => {
    opencodeBusy = true;
    opencodeNotice = null;
    try {
      opencode = await api.opencodeSet(opencodeUrl.trim(), opencodePassword ? opencodePassword : undefined);
      opencodePassword = "";
      const health = await api.opencodeCheck();
      opencodeNotice = { ok: true, text: `Connected to OpenCode${health.version ? ` ${health.version}` : ""}.` };
    } catch (e: unknown) {
      opencodeNotice = { ok: false, text: errorMessage(e) };
    } finally {
      opencodeBusy = false;
    }
  };

  const unlinkOpenCode = async () => {
    opencodeBusy = true;
    try {
      opencode = await api.opencodeSet(null, "");
      opencodeNotice = { ok: true, text: "OpenCode is no longer linked." };
    } catch (e: unknown) {
      opencodeNotice = { ok: false, text: errorMessage(e) };
    } finally {
      opencodeBusy = false;
    }
  };

  // Developer
  let devBusy = $state(false);
  let devNotice = $state<string | null>(null);
  const setDev = async (update: Parameters<typeof updateDev>[0]) => {
    devBusy = true;
    devNotice = null;
    try {
      await updateDev(update);
    } catch (e: unknown) {
      devNotice = errorMessage(e);
    } finally {
      devBusy = false;
    }
  };

  // Web search: a Firecrawl server on this computer
  let firecrawlUrl = $state("");
  const saveFirecrawlUrl = async () => {
    webBusy = true;
    webNotice = null;
    try {
      const value = firecrawlUrl.trim();
      await api.webSetFirecrawlUrl(value || null);
      await loadWebProviders();
      webNotice = value ? `Firecrawl now uses your server at ${value}.` : "Firecrawl uses its own service again.";
    } catch (e: unknown) {
      webNotice = errorMessage(e);
    } finally {
      webBusy = false;
    }
  };

  const themeTabs = [
    { value: "dark", label: "Dark" },
    { value: "light", label: "Light" },
  ];

  // Starts where the user left Settings last time.
  let activeTab = $state<SettingsSection>(settingsView.section as SettingsSection);
  let engine = $state<EngineInfo | null>(null);
  let scalePct = $state(Math.round(scale.value * 100));
  let accent = $state<Accent>(currentAccent());
  let font = $state<AppFont>(currentFont());
  let density = $state<Density>(currentDensity());

  const accentSwatches: { id: Accent; label: string; fill: string }[] = [
    { id: "ember", label: "Deep Ember", fill: "#ff7a1a" },
    { id: "ocean", label: "Ocean", fill: "oklch(73% 0.17 245)" },
    { id: "violet", label: "Violet", fill: "oklch(73% 0.17 305)" },
    { id: "forest", label: "Forest", fill: "oklch(73% 0.17 155)" },
    { id: "rose", label: "Rose", fill: "oklch(73% 0.17 15)" },
    { id: "gold", label: "Gold", fill: "oklch(73% 0.17 95)" },
  ];
  const fontTabs = [
    { value: "system", label: "System" },
    { value: "serif", label: "Serif" },
    { value: "mono", label: "Mono" },
  ];
  const densityTabs = [
    { value: "comfortable", label: "Comfortable" },
    { value: "compact", label: "Compact" },
  ];

  $effect(() => {
    scalePct = Math.round(scale.value * 100);
  });
  let engineBusy = $state(false);
  const fetchEngine = async () => {
    engineBusy = true;
    try {
      engine = await api.engineFetch();
    } catch (e: unknown) {
      settingsNotice = errorMessage(e);
    } finally {
      engineBusy = false;
    }
  };
  let appVersion = $state("");
  let usableRamGb = $state(0);
  let health = $state<SystemHealthResponse | null>(null);
  let updateInfo = $state<UpdateCheckResponse | null>(null);
  let logLines = $state<string[]>([]);
  let logSource = $state<"app" | "engine">("app");
  const logTabs = [
    { value: "app", label: "LocalAide" },
    { value: "engine", label: "Model engine" },
  ];
  let diagBusy = $state(false);
  let updateBusy = $state(false);
  let installProgress = $state<UpdateProgressEvent | null>(null);
  let installError = $state<string | null>(null);

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
  let contextInfo = $state<ContextInfo | null>(null);
  let reloadingModel = $state(false);
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
  let webProviders = $state<WebProviderStatus[]>([]);
  let webKeyDraft = $state<Record<string, string>>({});
  let webBusy = $state(false);
  let webNotice = $state<string | null>(null);
  // Presets tell which saved providers need a key, for the card status.
  let cloudPresets = $state<CloudProviderPreset[]>([]);
  let testingProvider = $state<string | null>(null);
  const presetFor = (url: string) => cloudPresets.find((p) => p.base_url.replace(/\/+$/, "") === url.replace(/\/+$/, ""));
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
    activeGroup = groupOf(section);
    settingsView.section = section as SettingsSectionId;
    if (section === "activity") void loadAudits();
    if (section === "backups") void loadBackup();
    if (section === "about") void loadDiagnostics();
    if (section === "integrations") void loadOpenCode();
  };

  const loadWebProviders = async () => {
    try {
      webProviders = await api.webProviders();
      firecrawlUrl = webProviders.find((p) => p.id === "firecrawl")?.base_url ?? "";
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
    if (cloudPresets.length) return;
    try {
      cloudPresets = await api.cloudProviderPresets();
    } catch (e: unknown) {
      console.error(e);
    }
  };

  const testProvider = async (id: string) => {
    testingProvider = id;
    cloudError = null;
    try {
      cloud = await api.testCloudProvider(id);
    } catch (e: unknown) {
      cloudError = errorMessage(e);
    } finally {
      testingProvider = null;
    }
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
    void loadPresets();
    try {
      cloud = await api.getCloudConfig();
    } catch (e: unknown) {
      console.error(e);
      cloud = null;
    }
  };

  const openAddProvider = () => {
    editingProviderId = null;
    showProviderForm = true;
  };

  const openEditProvider = (id: string) => {
    if (!cloud?.providers.some((item) => item.id === id)) return;
    editingProviderId = id;
    showProviderForm = true;
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

  const toggleProviderEnabled = async (id: string, enabled: boolean) => {
    cloudBusy = true;
    cloudError = null;
    try {
      cloud = await api.setProviderEnabled(id, enabled);
    } catch (e: unknown) {
      cloudError = errorMessage(e);
    } finally {
      cloudBusy = false;
    }
  };

  // The master switch. Off sends chat back to this computer and hides every
  // provider from the model menu; the providers and keys stay saved.
  const setOnline = async (enabled: boolean) => {
    if (!cloud) return;
    cloudBusy = true;
    cloudError = null;
    try {
      cloud = await api.setOnlineAllowed(enabled);
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
        api.getLogTail(120, logSource).catch(() => [] as string[]),
      ]);
      health = healthResult;
      logLines = logs;
    } catch (e: unknown) {
      settingsNotice = errorMessage(e);
    } finally {
      diagBusy = false;
    }
  };

  // Copies a local report: health counters, backend timings and both log
  // tails. The user reviews it before sharing; nothing is uploaded.
  const copyDiagnostics = async () => {
    diagBusy = true;
    try {
      const [appLog, engineLog] = await Promise.all([
        api.getLogTail(120, "app").catch(() => [] as string[]),
        api.getLogTail(120, "engine").catch(() => [] as string[]),
      ]);
      const report = buildDiagnosticReport({
        health,
        platform: platform(),
        entries: requestLog.entries,
        appLog,
        engineLog,
      });
      await navigator.clipboard.writeText(report);
      settingsNotice = "Diagnostics copied. Review before sharing.";
    } catch (e: unknown) {
      settingsNotice = `Could not copy diagnostics: ${errorMessage(e)}`;
    } finally {
      diagBusy = false;
    }
  };

  const checkForUpdates = async () => {
    updateBusy = true;
    installError = null;
    try {
      updateInfo = await api.checkForUpdate();
    } catch (e: unknown) {
      settingsNotice = errorMessage(e);
    } finally {
      updateBusy = false;
    }
  };

  // Downloads, checks the signature and installs. The app restarts itself
  // when it is done; your chats and models are kept.
  const installUpdate = async () => {
    installError = null;
    installProgress = { stage: "downloading", downloaded: 0, total: null };
    try {
      await api.installUpdate((event) => { installProgress = event; });
    } catch (e: unknown) {
      installError = errorMessage(e);
      installProgress = null;
    }
  };

  const installPercent = $derived(
    installProgress?.total ? Math.min(100, Math.round((installProgress.downloaded / installProgress.total) * 100)) : null,
  );

  // The command palette can open a section (and start an update check).
  $effect(() => {
    // Each request opens its section once; coming back later keeps the
    // section the user chose since.
    if (settingsRequest.tick === 0 || !settingsRequest.section || settingsRequest.tick === settingsView.handledTick) return;
    settingsView.handledTick = settingsRequest.tick;
    selectSection(settingsRequest.section);
    if (settingsRequest.checkUpdates) {
      settingsRequest.checkUpdates = false;
      void checkForUpdates();
    }
  });

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

  let compacting = $state(false);
  let compactNotice = $state<string | null>(null);

  const compactLibrary = async () => {
    compacting = true;
    settingsNotice = null;
    try {
      const result = await api.compactLibrary();
      const saved = Math.max(result.bytes_before - result.bytes_after, 0);
      compactNotice = saved > 0
        ? `Freed ${formatBytes(saved)}. The library now uses ${formatBytes(result.bytes_after)}.`
        : "There was no unused space to free.";
    } catch (e: unknown) {
      settingsNotice = errorMessage(e);
    } finally {
      compacting = false;
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
  const tokensLabel = (n: number) => n >= 1024 ? `${Math.round(n / 1024).toLocaleString()}K` : `${n}`;
  // The loaded model keeps the size it started with until it loads again.
  const contextNeedsReload = $derived(
    !!contextInfo?.active_id && contextInfo.loaded !== null && contextInfo.loaded !== contextTokens,
  );
  const contextDescription = $derived.by(() => {
    if (!contextInfo) return "How much of a chat the AI keeps in mind. Longer uses more memory.";
    const limit = contextInfo.model_name
      ? `${contextInfo.model_name} can hold up to ${tokensLabel(contextInfo.max)} tokens.`
      : `Up to ${tokensLabel(contextInfo.max)} tokens.`;
    return `How much of a chat the AI keeps in mind. ${limit} Longer uses more memory.`;
  });

  const reloadForContext = async () => {
    const id = contextInfo?.active_id;
    if (!id) return;
    reloadingModel = true;
    settingsNotice = null;
    try {
      if (contextSaveTimer) clearTimeout(contextSaveTimer);
      await api.setContext(contextTokens);
      await api.loadInferenceModel(id);
      contextInfo = await api.getContextInfo();
    } catch (e: unknown) {
      settingsNotice = errorMessage(e);
    } finally {
      reloadingModel = false;
    }
  };
  const modelMemory = () => usableRamGb > 0 ? `${(usableRamGb * ramBudget / 100).toFixed(1)} GB` : `${ramBudget}%`;

  onMount(async () => {
    // Load what the remembered section shows.
    selectSection(activeTab);
    void api.engineInfo().then((info) => (engine = info)).catch(() => undefined);
    // A setting that fails to load shows a message, so it never looks empty.
    const failed: string[] = [];
    try {
      ramBudget = await api.getModelMemory();
    } catch (e: unknown) {
      failed.push(`memory for models (${errorMessage(e)})`);
    }
    try {
      contextInfo = await api.getContextInfo();
      contextTokens = Math.min(contextInfo.value, contextInfo.max);
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
      <div class="relative mb-2">
        <Icon name="search" class="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-ink-500" />
        <input
          type="search"
          bind:value={settingsQuery}
          onkeydown={searchKeydown}
          aria-label="Search settings"
          placeholder="Search settings"
          class="h-9 w-full rounded-md border border-ink-700 bg-ink-900 pl-9 pr-3 text-sm text-ink-100 outline-none transition-colors placeholder:text-ink-500 focus:border-edge"
        />
      </div>
      <div class="mb-2 flex flex-wrap gap-1 border-b border-ink-800 pb-2" aria-label="Settings groups">
        {#each SETTINGS_GROUPS as group (group.id)}
          <button
            type="button"
            aria-pressed={activeGroup === group.id}
            title={group.label}
            onclick={() => selectGroup(group.id)}
            class={`flex items-center gap-1.5 rounded-md px-2 py-1.5 text-xs transition-colors ${activeGroup === group.id
              ? "bg-ink-800 font-semibold text-ink-50"
              : "text-ink-400 hover:bg-veil/5 hover:text-ink-100"}`}
          >
            <Icon name={group.icon} class="h-3.5 w-3.5 shrink-0" />
            <span class="truncate">{group.label}</span>
          </button>
        {/each}
      </div>
      <span class="sr-only">Settings sections</span>
      {#if settingsQuery.trim()}
        <p class="px-3 pb-1 text-xs text-ink-500" role="status">
          {visibleSections.length === 0 ? "No section matches" : `${visibleSections.length} ${visibleSections.length === 1 ? "section matches" : "sections match"}`}
        </p>
      {/if}
      {#each (settingsQuery.trim() ? visibleSections : groupSections) as section (section.id)}
        <button
          type="button"
          aria-current={activeTab === section.id ? "page" : undefined}
          onclick={() => selectSection(section.id)}
          class={`relative flex h-9 items-center gap-3 rounded-md px-3 text-left text-sm transition-colors duration-150 ${activeTab === section.id
            ? "bg-ink-800 font-semibold text-ink-50"
            : "text-ink-300 hover:bg-veil/5 hover:text-ink-50"}`}
        >
          <span class={`absolute left-0 top-1/2 w-[3px] -translate-y-1/2 rounded-full bg-ember-500 transition-[height,opacity] duration-200 ease-out ${activeTab === section.id ? "h-4 opacity-100" : "h-0 opacity-0"}`} aria-hidden="true"></span>
          <Icon name={section.icon} class="h-[18px] w-[18px]" />
          <span class="truncate">{section.label}</span>
        </button>
      {/each}
    </nav>

    <div class="min-w-0">
      {#if settingsNotice}
        <div class="mb-4"><ErrorBanner message={settingsNotice} onRetry={() => selectSection(activeTab)} onDismiss={() => settingsNotice = null} /></div>
      {/if}

      <!-- No entrance animation here: switching sections used to replay a
           fly-in every time, which read as the panel flipping to a new
           screen. The branches below already swap the content. -->
      <div>
      {#if activeTab === "general"}
        <div>
          <h2 class="text-xl font-semibold text-ink-50">General</h2>
          <p class="mt-1 text-sm text-ink-400">Change how LocalAide looks and uses this computer.</p>

          <div class="mt-6"><SelfTest /></div>

          <h3 class="mb-2 mt-7 text-[15px] font-semibold text-ink-50">Appearance</h3>
          <SettingsGroup>
            <SettingsRow icon="sun" title="Theme" description="Dark is the default. Your choice is kept when you restart.">
              {#snippet control()}
                <Tabs items={themeTabs} value={theme} onChange={(value) => { if (value === "dark" || value === "light") void setTheme(value); }} ariaLabel="Theme" />
              {/snippet}
            </SettingsRow>
            <SettingsRow icon="sparkle" title="Accent" description="The color behind every highlight, fill and glow. Contrast is kept for each one." class="border-t border-ink-800">
              {#snippet control()}
                <div class="flex items-center gap-1.5" role="radiogroup" aria-label="Accent">
                  {#each accentSwatches as swatch (swatch.id)}
                    <button
                      type="button"
                      role="radio"
                      aria-checked={accent === swatch.id}
                      aria-label={swatch.label}
                      title={swatch.label}
                      class={`grid h-7 w-7 place-items-center rounded-full border-2 transition-transform hover:scale-110 ${accent === swatch.id ? "border-ink-100" : "border-transparent"}`}
                      style={`background:${swatch.fill}`}
                      onclick={() => { accent = swatch.id; applyAccent(swatch.id); }}
                    >
                      {#if accent === swatch.id}<Icon name="check" class="h-3.5 w-3.5 text-white drop-shadow" />{/if}
                    </button>
                  {/each}
                </div>
              {/snippet}
            </SettingsRow>
            <SettingsRow icon="file" title="Font" description="The face the whole app speaks in." class="border-t border-ink-800">
              {#snippet control()}
                <Tabs items={fontTabs} value={font} onChange={(value) => { if (value === "system" || value === "serif" || value === "mono") { font = value; applyFont(value); } }} ariaLabel="Font" />
              {/snippet}
            </SettingsRow>
            <SettingsRow icon="scale" title="Density" description="Compact tightens the spacing between everything." class="border-t border-ink-800">
              {#snippet control()}
                <Tabs items={densityTabs} value={density} onChange={(value) => { if (value === "comfortable" || value === "compact") { density = value; applyDensity(value); } }} ariaLabel="Density" />
              {/snippet}
            </SettingsRow>
            <SettingsRow icon="scale" title="Interface scale" description="Makes everything bigger or smaller, from 70% to 250%. Ctrl + and Ctrl − change it anywhere; Ctrl 0 resets it." class="border-t border-ink-800">
              {#snippet control()}
                <Slider min={70} max={250} step={10} bind:value={scalePct} onchange={() => void setScale(scalePct / 100)} class="w-44" label="Interface scale" />
                <span class="min-w-12 text-right text-sm tabular-nums text-ink-200">{scalePct}%</span>
                {#if scale.value !== 1}<button type="button" class="rounded-md px-2 py-1 text-xs text-ink-400 hover:bg-veil/5 hover:text-ink-100" onclick={() => void setScale(1)}>Reset</button>{/if}
              {/snippet}
            </SettingsRow>
          </SettingsGroup>

          <NotificationsPanel />

          <h3 class="mb-2 mt-7 text-[15px] font-semibold text-ink-50">Performance</h3>
          <SettingsGroup>
            <SettingsRow icon="computer" title="Model engine" description={engine ? (engine.custom ? `A llama-server you placed yourself is in use: ${engine.path ?? ""}` : engine.installed ? `llama.cpp ${engine.build}, the build for ${engine.cpu} processors. It picks the fastest instructions this processor has.` : `Downloads once, the first time a model loads: llama.cpp ${engine.build} for ${engine.cpu} processors${engine.download_bytes ? `, ${formatBytes(engine.download_bytes)}` : ""}, from github.com.`) : "Checking…"}>
              {#snippet control()}
                {#if engine && !engine.installed}
                  <Button variant="secondary" onclick={() => void fetchEngine()} disabled={engineBusy}>{engineBusy ? "Downloading…" : "Download now"}</Button>
                {:else if engine}
                  <span class="text-sm text-emerald-300">Ready</span>
                {/if}
              {/snippet}
            </SettingsRow>
            <SettingsRow icon="models" title="Memory for models" description="How much of this computer's memory the AI may use. More lets you run bigger models.">
              {#snippet control()}
                <Slider min={20} max={90} bind:value={ramBudget} oninput={scheduleSave} class="w-44" label="Memory for models" />
                <span class="min-w-20 text-right text-sm tabular-nums text-ink-200">{modelMemory()}</span>
              {/snippet}
            </SettingsRow>
            <SettingsRow icon="chat" title="Conversation memory" description={contextDescription} class="border-t border-ink-800">
              {#snippet control()}
                <Slider min={contextInfo?.min ?? 512} max={contextInfo?.max ?? 131072} step={512} bind:value={contextTokens} oninput={scheduleContextSave} class="w-44" label="Conversation memory" />
                <span class="min-w-32 text-right text-sm tabular-nums text-ink-200"><span class="block">{tokensLabel(contextTokens)} tokens</span><span class="block text-xs text-ink-400">about {contextWords()} words</span></span>
              {/snippet}
            </SettingsRow>
            {#if contextNeedsReload && contextInfo?.loaded}
              <div class="flex flex-wrap items-center justify-between gap-3 border-t border-ink-800 px-4 py-3 text-sm text-ink-300" transition:slide={{ duration: dur(150) }}>
                <span class="flex items-center gap-2"><Icon name="info" class="h-4 w-4 shrink-0 text-ember-300" />The loaded model still uses {tokensLabel(contextInfo.loaded)} tokens. The new size applies the next time it loads.</span>
                <Button variant="secondary" disabled={reloadingModel} onclick={() => void reloadForContext()}>{reloadingModel ? "Reloading…" : "Reload model"}</Button>
              </div>
            {/if}
          </SettingsGroup>

          <h3 class="mb-2 mt-7 text-[15px] font-semibold text-ink-50">Where chats run</h3>
          <SettingsGroup>
            <SettingsRow icon="computer" title="On this computer only" description={cloud?.online_allowed ? "Online models are offered in the model menu. A message leaves this computer only when you pick one." : "Nothing you type is sent anywhere."}>
              {#snippet control()}
                <StatusBadge label={cloud?.online_allowed ? "Off" : "On"} tone={cloud?.online_allowed ? "idle" : "ok"} dot={!cloud?.online_allowed} />
              {/snippet}
            </SettingsRow>
            <SettingsRow icon="cloud" title="Online providers" description="Offer models from services you choose, with your own key. Off keeps chat on this computer." class="border-t border-ink-800">
              {#snippet control()}
                {#if !cloud || cloud.providers.length === 0}
                  <Button variant="secondary" onclick={() => { activeTab = "online"; openAddProvider(); }}>Add provider</Button>
                {:else}
                  <Switch checked={cloud.online_allowed} label="Online providers" disabled={cloudBusy} onchange={(on) => void setOnline(on)} />
                {/if}
              {/snippet}
            </SettingsRow>
          </SettingsGroup>
          <p class="mt-3 text-sm text-ink-400">Online providers are off until you add one. You can also add one straight from the chat's model menu. When they are on, a message goes online only when you pick an online model in the chat.</p>
        </div>
      {:else if activeTab === "online"}
        <div>
          <h2 class="text-xl font-semibold text-ink-50">Online providers</h2>
          <p class="mt-1 text-sm text-ink-400">Optional cloud chat with providers you choose. Some are free, and some need no key at all. Turn on as many as you like: their models appear in the chat's model menu, and a message goes online only when you pick one of them.</p>

          {#if cloudError}<div class="mt-4"><ErrorBanner message={cloudError} onRetry={() => void loadCloud()} onDismiss={() => cloudError = null} /></div>{/if}
          <div class="mt-6 rounded-lg border border-ember-500/30 bg-ember-500/10 p-4 text-sm leading-6 text-ember-200">
            While chat uses an online model, your messages go to that model's provider. With a model on this computer, nothing leaves it.
          </div>

          <h3 class="mb-2 mt-7 text-[15px] font-semibold text-ink-50">Your providers</h3>
          <SettingsGroup>
            <SettingsRow icon="cloud" title="Chat can use online models" description={cloud?.online_allowed ? "On. The providers switched on below appear in the chat's model menu. Pick one of their models there to use it." : "Off. Chat uses the model on this computer, and the providers below are not available."}>
              {#snippet control()}
                <Switch checked={cloud?.online_allowed ?? false} label="Chat can use online models" disabled={cloudBusy || !cloud} onchange={(on) => { if (on && cloud && cloud.providers.length === 0) openAddProvider(); void setOnline(on); }} />
              {/snippet}
            </SettingsRow>
          </SettingsGroup>

          {#if showProviderForm}
            <div class="mt-3">
              {#key editingProviderId}
              <ProviderForm
                editing={editingProviderId ? cloud?.providers.find((p) => p.id === editingProviderId) ?? null : null}
                onSaved={(status) => { cloud = status; showProviderForm = false; }}
                onCancel={() => (showProviderForm = false)}
              />
              {/key}
            </div>
          {/if}

          {#if !cloud || cloud.providers.length === 0}
            {#if !showProviderForm}
              <div class="mt-3 rounded-xl border border-dashed border-ink-700 p-6 text-center">
                <p class="text-sm text-ink-300">No provider yet. Chat stays on this computer until you add one.</p>
                <Button variant="primary" class="mt-4" onclick={openAddProvider}><Icon name="plus" class="h-4 w-4" />Add a provider</Button>
              </div>
            {/if}
          {:else}
            {#if !cloud.online_allowed}
              <p class="mt-3 flex items-center gap-2 text-sm text-ink-400"><Icon name="info" class="h-4 w-4 shrink-0" />Not available while online models are switched off. You can still edit or remove them.</p>
            {/if}
            <div class="mt-3 space-y-3">
              {#each cloud.providers as provider (provider.id)}
                <ProviderCard
                  {provider}
                  available={cloud.online_allowed}
                  needsKey={!provider.api_key_hint && !!presetFor(provider.base_url)?.needs_key}
                  busy={cloudBusy}
                  testing={testingProvider === provider.id}
                  onToggle={(on) => void toggleProviderEnabled(provider.id, on)}
                  onTest={() => void testProvider(provider.id)}
                  onEdit={() => openEditProvider(provider.id)}
                  onRemove={() => deleteProvider(provider.id)}
                />
              {/each}
            </div>
            {#if !showProviderForm}
              <Button variant="secondary" class="mt-3" disabled={cloudBusy} onclick={openAddProvider}><Icon name="plus" class="h-4 w-4" />Add a provider</Button>
            {/if}
          {/if}

          <div class="mt-9">
            <CloudAuditLog />
          </div>
        </div>
      {:else if activeTab === "tools"}
        <McpSettings />
        <McpOAuthPanel />
        <div class="mt-8"><AgentsPanel /></div>
        <PolicyPanel />
      {:else if activeTab === "web"}
        <div>
          <h2 class="text-xl font-semibold text-ink-50">Web search</h2>
          <p class="mt-1 text-sm text-ink-400">Every provider works without a key. LocalAide searches only when you press Search, or when you turn on Web in the chat box.</p>
          {#if webNotice}<div class="mt-4 rounded-md border border-ink-700 bg-ink-900 p-3 text-sm text-ink-300" in:slide={{ duration: dur(150) }}>{webNotice}</div>{/if}
          <div class="mt-6 space-y-2">
            {#each webProviders as provider (provider.id)}
              <div class={`rounded-lg border p-4 transition-colors ${provider.is_default ? "border-ember-500/40 bg-ember-500/5" : "border-ink-700 bg-ink-900"}`}>
                <div class="flex flex-col gap-3 lg:flex-row lg:items-start lg:justify-between">
                  <div class="min-w-0">
                    <div class="flex flex-wrap items-center gap-2">
                      <span class="font-semibold text-ink-50">{provider.label}</span>
                      {#if provider.is_default}<StatusBadge label="Default" tone="ok" dot />{/if}
                      <span class="rounded-full bg-emerald-500/10 px-2 py-0.5 text-xs font-semibold text-emerald-300">{provider.key_set ? "Your key" : "No key needed"}</span>
                      <span class="text-xs text-ink-500">{provider.supports === "crawl" ? "Reads pages" : provider.supports === "search" ? "Search" : "Search and reads pages"}</span>
                    </div>
                    <p class="mt-1 text-sm text-ink-400">{provider.note}</p>
                  </div>
                  <div class="flex flex-wrap items-center gap-2">
                    {#if provider.takes_key}
                      <input type="password" bind:value={webKeyDraft[provider.id]} placeholder={provider.key_set ? "Replace key" : "Optional key"} autocomplete="off" class="h-9 w-44 rounded-md border border-ink-700 bg-ink-950 px-2 text-sm text-ink-100 placeholder:text-ink-400" />
                      <Button variant="secondary" disabled={webBusy} onclick={() => void saveWebKey(provider.id)}>{webKeyDraft[provider.id]?.trim() || !provider.key_set ? "Save key" : "Remove key"}</Button>
                    {/if}
                    {#if provider.supports.includes("search") && !provider.is_default}<Button variant="subtle" disabled={webBusy} onclick={() => void setWebDefault(provider.id)}>Make default</Button>{/if}
                  </div>
                </div>
                {#if provider.id === "firecrawl"}
                  <div class="mt-3 border-t border-ink-800 pt-3">
                    <label class="block text-sm text-ink-400" for="firecrawl-url">Your own Firecrawl server (optional)</label>
                    <div class="mt-1 flex flex-col gap-2 sm:flex-row">
                      <input id="firecrawl-url" bind:value={firecrawlUrl} placeholder="http://localhost:3002" class="h-9 min-w-0 flex-1 rounded-md border border-ink-700 bg-ink-950 px-3 font-mono text-sm text-ink-100 placeholder:text-ink-500" />
                      <Button variant="secondary" disabled={webBusy} onclick={() => void saveFirecrawlUrl()}>{firecrawlUrl.trim() ? "Use this server" : "Use Firecrawl's service"}</Button>
                    </div>
                    <p class="mt-1.5 text-xs text-ink-500">Run Firecrawl yourself (for example with its Docker setup) and nothing goes to Firecrawl's servers. Leave empty to use their keyless free tier.</p>
                  </div>
                {/if}
              </div>
            {:else}
              <div class="rounded-lg border border-ink-700 bg-ink-900 p-5 text-sm text-ink-400">The provider list is not ready. Restart the app and try again.</div>
            {/each}
          </div>
          <p class="mt-4 text-sm text-ink-400">Keys stay in this app's local settings and are sent only to their own provider. Without a key, free limits apply per computer. Backups leave keys out unless you choose to include them.</p>
        </div>
      {:else if activeTab === "voice"}
        <VoiceSettings />
      {:else if activeTab === "share"}
        <ShareSettings />
      {:else if activeTab === "integrations"}
        <div>
          <h2 class="text-xl font-semibold text-ink-50">Integrations</h2>
          <p class="mt-1 text-sm text-ink-400">Connect LocalAide to tools you already run on this computer.</p>
          <div class="mt-6 rounded-lg border border-ink-700 bg-ink-900 p-5">
            <div class="flex items-start gap-3">
              <span class="grid h-10 w-10 shrink-0 place-items-center rounded-lg bg-ink-800 text-ember-300"><Icon name="code" class="h-5 w-5" /></span>
              <div class="min-w-0 flex-1">
                <div class="flex flex-wrap items-center gap-2"><h3 class="font-semibold text-ink-50">OpenCode</h3>{#if opencode?.base_url}<StatusBadge label="Linked" tone="ok" dot />{/if}</div>
                <p class="mt-1 text-sm leading-6 text-ink-400">Hand coding tasks from chat to OpenCode, the open-source coding agent. Start its server with <code class="rounded bg-ink-800 px-1 font-mono text-[0.85em]">opencode serve</code>, link it here, then use the <Icon name="code" class="inline h-3.5 w-3.5" /> button under any message. Front ends for OpenCode, such as OpenChamber, see the same sessions.</p>
              </div>
            </div>
            <div class="mt-4 grid gap-3 sm:grid-cols-[1fr_14rem]">
              <label class="text-sm text-ink-400">Server address<input bind:value={opencodeUrl} placeholder={opencode?.default_url ?? "http://127.0.0.1:4096"} class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 font-mono text-sm text-ink-100 placeholder:text-ink-500" /></label>
              <label class="text-sm text-ink-400">Password {opencode?.has_password ? "(saved)" : "(if set)"}<input type="password" bind:value={opencodePassword} autocomplete="off" placeholder={opencode?.has_password ? "Keep the saved one" : "OPENCODE_SERVER_PASSWORD"} class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 text-sm text-ink-100 placeholder:text-ink-500" /></label>
            </div>
            {#if opencodeNotice}
              <p class={`mt-3 flex items-center gap-2 text-sm ${opencodeNotice.ok ? "text-emerald-300" : "text-rose-300"}`} in:slide={{ duration: dur(150) }}><Icon name={opencodeNotice.ok ? "check" : "alert"} class="h-4 w-4" />{opencodeNotice.text}</p>
            {/if}
            <div class="mt-4 flex flex-wrap justify-end gap-2">
              {#if opencode?.base_url}<Button variant="subtle" disabled={opencodeBusy} onclick={() => void unlinkOpenCode()}>Unlink</Button>{/if}
              <Button variant="primary" disabled={opencodeBusy || !opencodeUrl.trim()} onclick={() => void linkOpenCode()}>{opencodeBusy ? "Connecting…" : opencode?.base_url ? "Save and test" : "Link and test"}</Button>
            </div>
          </div>
          <div class="mt-3 rounded-lg border border-ink-700 bg-ink-900 p-5">
            <div class="flex items-start gap-3">
              <span class="grid h-10 w-10 shrink-0 place-items-center rounded-lg bg-ink-800 text-ember-300"><Icon name="cloud" class="h-5 w-5" /></span>
              <div class="min-w-0 flex-1">
                <h3 class="font-semibold text-ink-50">OpenCode Zen and Kilo Code models</h3>
                <p class="mt-1 text-sm leading-6 text-ink-400">To chat with OpenCode's or Kilo Code's free models directly, add them as online providers. Neither needs a sign-up.</p>
              </div>
              {#if moduleOn("online")}<Button variant="secondary" onclick={() => { selectSection("online"); openAddProvider(); }}>Add a provider</Button>{/if}
            </div>
          </div>
          <PluginPanels />
        </div>
      {:else if activeTab === "developer"}
        {@const ds = dev.settings}
        <div>
          <h2 class="text-xl font-semibold text-ink-50">Developer</h2>
          <p class="mt-1 text-sm text-ink-400">Experimental features, the parts LocalAide is made of, and what the chat modes tell the model.</p>
          {#if devNotice}<div class="mt-4"><ErrorBanner message={devNotice} onDismiss={() => devNotice = null} /></div>{/if}
          {#if !ds}
            <div class="skeleton mt-6 h-24"></div>
          {:else}
            <div class="mt-6"></div>
            <SettingsGroup>
              <SettingsRow icon="flask" title="Developer mode" description="Turns on the experimental features below. They may change or disappear in later versions.">
                {#snippet control()}
                  <Switch checked={ds.enabled} label="Developer mode" disabled={devBusy} onchange={(on) => void setDev({ enabled: on })} />
                {/snippet}
              </SettingsRow>
            </SettingsGroup>

            <h3 class="mb-2 mt-7 flex items-center gap-2 text-[15px] font-semibold text-ink-50"><Icon name="flask" class="h-4 w-4 text-ember-300" />Experimental features</h3>
            <div class={`space-y-2 transition-opacity ${ds.enabled ? "" : "pointer-events-none opacity-50"}`} aria-disabled={!ds.enabled}>
              {#each ds.flag_list as f (f.id)}
                <div class="flex items-center justify-between gap-4 rounded-lg border border-ink-700 bg-ink-900 p-3.5 transition-colors hover:border-edge">
                  <span class="min-w-0"><span class="block text-sm font-semibold text-ink-100">{f.label}</span><span class="block text-sm text-ink-400">{f.description}</span></span>
                  <Switch checked={!!ds.flags[f.id]} label={f.label} disabled={!ds.enabled || devBusy} onchange={(on) => void setDev({ flags: { [f.id]: on } })} />
                </div>
              {/each}
            </div>

            <h3 class="mb-2 mt-7 flex items-center gap-2 text-[15px] font-semibold text-ink-50"><Icon name="layers" class="h-4 w-4 text-ember-300" />Modules</h3>
            <p class="mb-2 text-sm text-ink-400">Switch off parts of LocalAide you do not use. They stay off until you switch them on here, even outside developer mode.</p>
            <div class="grid gap-2 sm:grid-cols-2">
              {#each ds.module_list as m (m.id)}
                <div class="flex items-center justify-between gap-4 rounded-lg border border-ink-700 bg-ink-900 p-3.5 transition-colors hover:border-edge">
                  <span class="min-w-0"><span class="block text-sm font-semibold text-ink-100">{m.label}</span><span class="block text-sm text-ink-400">{m.description}</span></span>
                  <Switch checked={!!ds.modules[m.id]} label={m.label} disabled={devBusy} onchange={(on) => void setDev({ modules: { [m.id]: on } })} />
                </div>
              {/each}
            </div>

            <h3 class="mb-2 mt-7 flex items-center gap-2 text-[15px] font-semibold text-ink-50"><Icon name="chat" class="h-4 w-4 text-ember-300" />Mode instructions</h3>
            <div class="space-y-2">
              {#each ds.modes as m (m.id)}
                <details class="group rounded-lg border border-ink-700 bg-ink-900">
                  <summary class="flex cursor-pointer list-none items-center gap-2 px-4 py-3 text-sm font-semibold text-ink-100"><Icon name="chevron-right" class="h-4 w-4 transition-transform duration-200 group-open:rotate-90" />{m.label}<span class="font-normal text-ink-500">· {m.description}</span></summary>
                  <p class="border-t border-ink-800 px-4 py-3 font-mono text-[12px] leading-5 text-ink-300">{m.instruction || "No extra instruction. The model gets your system prompt and any notes or web context."}</p>
                </details>
              {/each}
            </div>

            {#if flag("request_log")}
              <div class="mb-2 mt-7 flex items-center justify-between">
                <h3 class="flex items-center gap-2 text-[15px] font-semibold text-ink-50"><Icon name="file" class="h-4 w-4 text-ember-300" />Request log</h3>
                <Button variant="subtle" onclick={clearRequestLog}>Clear</Button>
              </div>
              <div class="max-h-80 overflow-y-auto rounded-lg border border-ink-700 bg-ink-950 font-mono text-[12px]">
                {#each requestLog.entries as r (r.id)}
                  <div class="flex items-center gap-3 border-b border-ink-800 px-3 py-1.5 last:border-b-0">
                    <span class={`h-2 w-2 shrink-0 rounded-full ${r.ok ? "bg-emerald-400" : "bg-rose-400"}`}></span>
                    <span class="w-20 shrink-0 text-ink-500">{new Date(r.at).toLocaleTimeString()}</span>
                    <span class="min-w-0 flex-1 truncate text-ink-100">{r.command}</span>
                    {#if r.error}<span class="max-w-[40%] truncate text-rose-300" title={r.error}>{r.error}</span>{/if}
                    <span class={`w-16 shrink-0 text-right ${r.ms > 1000 ? "text-amber-300" : "text-ink-400"}`}>{r.ms.toFixed(0)} ms</span>
                  </div>
                {:else}
                  <p class="px-3 py-6 text-center text-ink-500">No calls yet. Use the app and they appear here.</p>
                {/each}
              </div>
            {/if}

            <div class="mt-7 flex justify-end"><Button variant="subtle" disabled={devBusy} onclick={() => { void resetDev().catch((e: unknown) => devNotice = errorMessage(e)); }}>Reset everything here</Button></div>
          {/if}

          <GoalPanel />
          <UsagePanel />
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

          <h3 class="mb-2 mt-7 text-[15px] font-semibold text-ink-50">Storage</h3>
          <SettingsGroup>
            <SettingsRow icon="database" title="Document library" description={compactNotice ?? "After you remove many documents, this frees the space they used."}>
              {#snippet control()}
                <Button variant="secondary" disabled={compacting} onclick={() => void compactLibrary()}>{compacting ? "Working…" : "Free up space"}</Button>
              {/snippet}
            </SettingsRow>
          </SettingsGroup>
        </div>
      {:else if activeTab === "backups"}
        <div>
          <h2 class="text-xl font-semibold text-ink-50">Backups</h2>
          <p class="mt-1 text-sm text-ink-400">Save your notes, chats and settings in an encrypted local file.</p>
          <div class="mt-6 rounded-lg border border-ink-700 bg-ink-900 p-4">
            <label class="block text-sm text-ink-400" for="backup-folder">Backup folder</label><div class="mt-1 flex gap-2"><input id="backup-folder" bind:value={backupTargetInput} placeholder="Leave empty for the default folder" class="h-10 min-w-0 flex-1 rounded-md border border-ink-700 bg-ink-950 px-3 text-sm text-ink-100 outline-none placeholder:text-ink-400 focus:border-edge" /><Button variant="secondary" class="h-10" disabled={backupBusy} onclick={() => void browseBackupFolder()}>Browse…</Button></div>
            <div class="mt-4 flex items-center gap-3 text-sm text-ink-300"><Switch checked={includeKeys} label="Include provider keys in the backup" onchange={(on) => (includeKeys = on)} />Include provider keys in the backup</div>
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
          <div class="mt-6"><DoctorPanel /></div>
          <h3 class="mb-2 mt-7 text-[15px] font-semibold text-ink-50">Updates</h3>
          <div class="rounded-lg border border-ink-700 bg-ink-900 p-4">
            <div class="flex flex-wrap items-center justify-between gap-3">
              <div>
                <div class="font-semibold text-ink-50">Check for a new version</div>
                <p class="mt-0.5 text-sm text-ink-400">LocalAide asks GitHub only when you press the button. Updates keep your chats, notes and models.</p>
              </div>
              <Button variant="secondary" disabled={updateBusy || installProgress !== null} onclick={() => void checkForUpdates()}>
                <Icon name="refresh" class={`h-4 w-4 ${updateBusy ? "animate-spin" : ""}`} />
                {updateBusy ? "Checking…" : "Check for updates"}
              </Button>
            </div>
            {#if updateInfo?.update_available}
              <div class="mt-4 rounded-md border border-ember-500/30 bg-ember-500/5 p-4">
                <div class="flex flex-wrap items-center justify-between gap-3">
                  <div>
                    <div class="font-semibold text-ink-50">Version {updateInfo.latest_version} is ready</div>
                    <p class="text-sm text-ink-400">You have {updateInfo.current_version}.</p>
                  </div>
                  {#if updateInfo.can_install}
                    <Button variant="primary" disabled={installProgress !== null} onclick={() => void installUpdate()}>
                      <Icon name="download" class="h-4 w-4" />
                      {installProgress ? "Installing…" : "Install and restart"}
                    </Button>
                  {:else}
                    <Button variant="primary" onclick={(e) => openInBrowser(e, updateInfo?.release_url ?? "")}>
                      <Icon name="external" class="h-4 w-4" />
                      Open the download page
                    </Button>
                  {/if}
                </div>
                {#if installProgress}
                  <div class="mt-4" in:slide={{ duration: dur(150) }}>
                    <div class="h-1.5 overflow-hidden rounded-full bg-ink-800">
                      {#if installPercent !== null && installProgress.stage === "downloading"}
                        <div class="h-full rounded-full bg-ember-500 transition-[width] duration-300 ease-out" style={`width:${installPercent}%`}></div>
                      {:else}
                        <div class="progress-indeterminate h-full w-1/3 rounded-full bg-ember-500"></div>
                      {/if}
                    </div>
                    <p class="mt-2 text-sm text-ink-400" aria-live="polite">
                      {#if installProgress.stage === "downloading"}
                        Downloading{installPercent !== null ? ` · ${installPercent}%` : ""}{installProgress.total ? ` of ${formatBytes(installProgress.total)}` : ""}
                      {:else if installProgress.stage === "installing"}
                        Checking the signature and installing…
                      {:else}
                        Restarting LocalAide…
                      {/if}
                    </p>
                  </div>
                {/if}
                {#if installError}
                  <div class="mt-3"><ErrorBanner message={installError} onRetry={() => void installUpdate()} onDismiss={() => installError = null} /></div>
                {/if}
                {#if !updateInfo.can_install}
                  <p class="mt-3 text-sm text-ink-400">This copy cannot install updates by itself. Download the new version and open it; it replaces this one and keeps your data.</p>
                {/if}
                {#if updateInfo.notes}
                  <details class="group mt-3">
                    <summary class="flex cursor-pointer list-none items-center gap-1 text-sm font-semibold text-ember-300 hover:underline">
                      <Icon name="chevron-right" class="h-4 w-4 transition-transform duration-200 group-open:rotate-90" />
                      What's new
                    </summary>
                    <div class="mt-2 max-h-64 overflow-y-auto rounded-md border border-ink-800 bg-ink-950 p-3">
                      <MarkdownDoc source={updateInfo.notes} />
                    </div>
                  </details>
                {/if}
              </div>
            {:else}
              <p class="mt-3 text-sm text-ink-400">{updateInfo ? `You have the newest version, ${updateInfo.current_version}.` : "Not checked yet."}</p>
            {/if}
          </div>

          <h3 class="mb-2 mt-7 text-[15px] font-semibold text-ink-50">Legal</h3>
          <SettingsGroup>
            <SettingsRow icon="shield" title="Privacy Policy" description="What LocalAide stores, and when it uses the internet.">
              {#snippet control()}<Button variant="subtle" onclick={() => openLegal("privacy")}>Read</Button>{/snippet}
            </SettingsRow>
            <SettingsRow icon="scale" title="Terms of Use" description="The rules for using LocalAide and the models it downloads." class="border-t border-ink-800">
              {#snippet control()}<Button variant="subtle" onclick={() => openLegal("terms")}>Read</Button>{/snippet}
            </SettingsRow>
            <SettingsRow icon="file" title="Open-source notices" description="LocalAide is MIT licensed and built on open-source software." class="border-t border-ink-800">
              {#snippet control()}<Button variant="subtle" onclick={() => openLegal("notices")}>Read</Button>{/snippet}
            </SettingsRow>
          </SettingsGroup>
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
          {#if health && health.performance}
            {@const perf = requestStats(requestLog.entries)}
            <div class="mt-4 rounded-lg border border-ink-700 bg-ink-900 p-4">
              <div class="flex items-center justify-between gap-3">
                <h3 class="font-semibold text-ink-50">Performance</h3>
                <Button variant="subtle" disabled={diagBusy} onclick={() => void copyDiagnostics()}>Copy diagnostics</Button>
              </div>
              <div class="mt-3 grid grid-cols-2 gap-3 text-sm sm:grid-cols-4">
                <div><span class="block text-ink-500">Uptime</span><span class="font-medium text-ink-100">{health.performance.uptime_secs}s</span></div>
                <div><span class="block text-ink-500">App memory</span><span class="font-medium text-ink-100">{formatBytes(health.performance.rss_bytes)}</span></div>
                <div><span class="block text-ink-500">Calls · p95</span><span class="font-medium text-ink-100">{perf.count} · {perf.p95Ms.toFixed(0)} ms</span></div>
                <div><span class="block text-ink-500">Logs on disk</span><span class="font-medium text-ink-100">{formatBytes(health.performance.logs_bytes)}</span></div>
              </div>
              {#if perf.slowest}
                <p class="mt-3 text-xs text-ink-400">Slowest call: <code class="text-ink-200">{perf.slowest.command}</code> · {perf.slowest.ms.toFixed(0)} ms</p>
              {/if}
              <p class="mt-2 text-xs text-ink-500">Measured on this machine. Copy produces a report you can review before sharing; nothing is uploaded.</p>
            </div>
          {/if}
          <div class="mt-4 rounded-lg border border-ink-700 bg-ink-900 p-4"><div class="flex items-center justify-between gap-3"><h3 class="font-semibold text-ink-50">Local log</h3><div class="flex items-center gap-2"><Tabs items={logTabs} value={logSource} onChange={(value) => { if (value === "app" || value === "engine") { logSource = value; void loadDiagnostics(); } }} ariaLabel="Which log" /><Button variant="subtle" disabled={diagBusy} onclick={() => void loadDiagnostics()}>Refresh</Button></div></div>{#if logLines.length === 0}<p class="mt-3 text-sm text-ink-400">No log output yet.</p>{:else}<pre class="mt-3 max-h-72 overflow-auto rounded-md border border-ink-700 bg-ink-950 p-3 text-sm leading-6 text-ink-300 whitespace-pre-wrap">{logLines.join("\n")}</pre>{/if}</div>
        </div>
      {/if}
      </div>
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
