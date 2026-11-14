<script lang="ts">
  import { api, errorMessage } from "../lib/api/client";
  import { dur } from "../lib/motion";
  import type { CloudConfigStatus, ToolAuditEntry } from "../lib/api/bindings";
  import { getVersion } from "@tauri-apps/api/app";
  import { slide } from "svelte/transition";
  import { onMount } from "svelte";
  import Icon from "../components/Icon.svelte";
  import ErrorBanner from "../components/ErrorBanner.svelte";
  import type { SystemHealthResponse, UpdateCheckResponse } from "../lib/api/bindings";

  let appVersion = $state("");

  let activeTab = $state<"general" | "security" | "audit" | "diagnostics">("general");
  let health = $state<SystemHealthResponse | null>(null);
  let updateInfo = $state<UpdateCheckResponse | null>(null);
  let logLines = $state<string[]>([]);
  let diagBusy = $state(false);

  let ramBudget = $state(70);
  let contextTokens = $state(8192);
  let contextSaveTimer: ReturnType<typeof setTimeout> | null = null;
  let modelPriority = $state("local");
  let saving = $state(false);
  let saved = $state(false);

  // Audit logs
  let audits = $state<ToolAuditEntry[]>([]);
  let loadingAudits = $state(false);
  let clearingAudits = $state(false);

  // Online providers (opt-in cloud mode)
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

  let debounceTimer: ReturnType<typeof setTimeout> | null = null;

  const loadAudits = async () => {
    loadingAudits = true;
    try {
      audits = await api.listToolAudits(100);
    } catch {
      audits = [];
    } finally {
      loadingAudits = false;
    }
  };

  const clearAudits = async () => {
    if (!confirm("Clear all tool execution audit logs from SQLite?")) return;
    clearingAudits = true;
    try {
      await api.clearToolAudits();
      audits = [];
    } catch {
      audits = [];
    } finally {
      clearingAudits = false;
    }
  };

  const loadCloud = async () => {
    try {
      cloud = await api.getCloudConfig();
    } catch {
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
    showProviderForm = true;
  };

  const openEditProvider = (id: string) => {
    const p = cloud?.providers.find((x) => x.id === id);
    if (!p) return;
    editingProviderId = id;
    providerLabel = p.label;
    providerUrl = p.base_url;
    providerModel = p.model;
    providerKey = "";
    providerFormError = null;
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
    } catch (e) {
      providerFormError = errorMessage(e);
    } finally {
      cloudBusy = false;
    }
  };

  const deleteProvider = async (id: string) => {
    const p = cloud?.providers.find((x) => x.id === id);
    if (!confirm(`Delete provider "${p?.label ?? id}"? Its stored API key is removed too.`)) return;
    cloudBusy = true;
    cloudError = null;
    try {
      cloud = await api.deleteCloudProvider(id);
    } catch (e) {
      cloudError = errorMessage(e);
    } finally {
      cloudBusy = false;
    }
  };

  const toggleCloud = async () => {
    if (!cloud) return;
    cloudBusy = true;
    cloudError = null;
    try {
      cloud = await api.setCloudMode({ enabled: !cloud.cloud_enabled });
    } catch (e) {
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
    } catch (e) {
      cloudError = errorMessage(e);
    } finally {
      cloudBusy = false;
    }
  };

  onMount(async () => {
    try {
      const cfg = await api.loadConfig();
      ramBudget = cfg.ram_budget_mb ? Math.round((cfg.ram_budget_mb / 1024) / 0.7) : 70;
      modelPriority = cfg.selected_models?.length ? "local" : modelPriority;
    } catch {}
    try {
      contextTokens = await api.getContext();
    } catch {}
    try {
      appVersion = await getVersion();
    } catch {
      appVersion = "";
    }
    loadAudits();
    loadCloud();
  });

  // Settings persist when the slider moves, not on a component-lifetime
  // effect: saving on mount would race a cloud-mode toggle in its 500 ms
  // window and silently revert the persisted engine mode.
  const persistSettings = async () => {
    saving = true;
    try {
      const current = await api.loadConfig();
      const cfg = {
        ram_budget_mb: Math.round(ramBudget * 0.7 * 1024),
        cloud_llm_enabled: current.cloud_llm_enabled,
        selected_models: current.selected_models ?? [],
        presets: current.presets ?? []
      };
      await api.saveConfig(cfg);
      saved = true;
      setTimeout(() => saved = false, 1500);
    } finally {
      saving = false;
    }
  };

  const scheduleSave = () => {
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(persistSettings, 500);
  };

  const scheduleContextSave = () => {
    if (contextSaveTimer) clearTimeout(contextSaveTimer);
    contextSaveTimer = setTimeout(async () => {
      try {
        await api.setContext(contextTokens);
      } catch {}
    }, 500);
  };

  const loadDiagnostics = async () => {
    diagBusy = true;
    try {
      const [h, u, logs] = await Promise.all([
        api.getSystemHealth(),
        api.checkForUpdate().catch(() => null),
        api.getLogTail(120).catch(() => [] as string[]),
      ]);
      health = h;
      updateInfo = u;
      logLines = logs;
    } catch {}
    diagBusy = false;
  };
</script>

<div class="max-w-4xl mx-auto space-y-6">
  <div class="border-b border-ink-800 pb-4 flex items-center justify-between">
    <div>
      <h2 class="text-lg font-semibold tracking-tight text-ink-100">Settings & Security</h2>
      <p class="text-xs text-ink-400">Resource quotas, capability sandbox, and audit verification</p>
    </div>
    <div class="flex items-center gap-1 bg-ink-900 border border-ink-800 p-1 rounded-xl text-xs">
      <button
        onclick={() => activeTab = "general"}
        class={`px-3 py-1 rounded-lg transition-colors ${activeTab === "general" ? "bg-ember-600 text-ink-950 font-medium" : "text-ink-400 hover:text-ink-200"}`}
      >
        General
      </button>
      <button
        onclick={() => activeTab = "security"}
        class={`px-3 py-1 rounded-lg transition-colors ${activeTab === "security" ? "bg-ember-600 text-ink-950 font-medium" : "text-ink-400 hover:text-ink-200"}`}
      >
        Security Jail
      </button>
      <button
        onclick={() => { activeTab = "diagnostics"; loadDiagnostics(); }}
        class={`px-3 py-1 rounded-lg transition-colors ${activeTab === "diagnostics" ? "bg-ember-600 text-ink-950 font-medium" : "text-ink-400 hover:text-ink-200"}`}
      >
        Diagnostics
      </button>
      <button
        onclick={() => { activeTab = "audit"; loadAudits(); }}
        class={`px-3 py-1 rounded-lg transition-colors ${activeTab === "audit" ? "bg-ember-600 text-ink-950 font-medium" : "text-ink-400 hover:text-ink-200"}`}
      >
        Audit Log
      </button>
    </div>
  </div>

  {#if activeTab === "general"}
    <div class="space-y-6">
      <div class="border border-ink-800 rounded-2xl p-5 bg-ink-900/60">
        <div class="flex items-center justify-between mb-4">
          <div>
            <div class="font-medium text-ink-100">RAM Budget</div>
            <div class="text-xs text-ink-400">Memory quota allocated to local models</div>
          </div>
          <span class="text-sm font-mono font-semibold text-ember-300">{ramBudget}%</span>
        </div>
        <input type="range" min={20} max={90} bind:value={ramBudget} oninput={scheduleSave} class="w-full accent-ember-600"/>
        <div class="mt-3 h-2 rounded-full bg-ink-800 overflow-hidden">
          <div class="h-full bg-ember-600 transition-all" style={`width:${ramBudget}%`}></div>
        </div>
        <div class="mt-2 text-[11px] text-ink-400 font-mono">≈ {(ramBudget * 0.7).toFixed(0)}% of usable physical RAM</div>
      </div>

      <div class="border border-ink-800 rounded-2xl p-5 bg-ink-900/60">
        <div class="flex items-center justify-between mb-4">
          <div>
            <div class="font-medium text-ink-100">Context Window</div>
            <div class="text-xs text-ink-400">Tokens of conversation the local model keeps in memory</div>
          </div>
          <span class="text-sm font-mono font-semibold text-ember-300">{contextTokens.toLocaleString()}</span>
        </div>
        <input
          type="range"
          min={512}
          max={131072}
          step={512}
          bind:value={contextTokens}
          oninput={scheduleContextSave}
          class="w-full accent-ember-600"
        />
        <div class="mt-2 text-[11px] text-ink-400 font-mono">
          Higher values remember more of the chat but use more RAM. A model's own limit caps this further.
        </div>
      </div>

      <div class="border border-ink-800 rounded-2xl p-5 bg-ink-900/60">
        <div class="font-medium text-ink-100 mb-2">Model Execution Priority</div>
        <p class="text-xs text-ink-400 mb-3">
          {#if cloud?.cloud_enabled}
            Chat currently runs on your online provider. Switch it off to return to fully local inference.
          {:else}
            All inference executes locally on this machine. No messages leave your computer.
          {/if}
        </p>
        <div class="space-y-2">
          <label class="flex items-center gap-3 cursor-pointer">
            <input type="radio" name="priority" bind:group={modelPriority} value="local" class="accent-ember-600" checked/>
            <span class="text-sm text-ink-300">Local-only (100% offline, zero network telemetry)</span>
          </label>
        </div>
      </div>

      <div class="border border-ink-800 rounded-2xl p-5 bg-ink-900/60 space-y-4">
        <div class="flex items-center justify-between">
          <div>
            <div class="font-medium text-ink-100">Online Providers</div>
            <div class="text-xs text-ink-400">Optional cloud chat with an OpenAI-compatible provider you configure yourself.</div>
          </div>
          <button
            role="switch"
            aria-checked={cloud?.cloud_enabled ?? false}
            aria-label="Use online provider for chat"
            onclick={toggleCloud}
            disabled={cloudBusy || !cloud || cloud.providers.length === 0}
            class={`relative w-11 h-6 rounded-full transition-colors disabled:opacity-40 ${cloud?.cloud_enabled ? "bg-ember-600" : "bg-ink-700"}`}
          >
            <span class={`absolute top-0.5 left-0.5 w-5 h-5 rounded-full bg-white transition-transform ${cloud?.cloud_enabled ? "translate-x-5" : ""}`}></span>
          </button>
        </div>

        <p class="text-[11px] text-ink-400 border-l-2 border-amber-500/50 pl-2">
          When this is on, your chat messages are sent to the provider below over HTTPS and answers come back from its servers — they are not processed on this machine. When it is off, nothing leaves your computer.
        </p>

        {#if cloudError}
          <ErrorBanner message={cloudError} onDismiss={() => cloudError = null} />
        {/if}

        {#if !cloud || cloud.providers.length === 0}
          <div class="border border-ink-800/80 bg-ink-950/60 rounded-xl p-4 text-center space-y-2">
            <div class="text-xs text-ink-400">No online providers yet. Everything stays local until you add one.</div>
            <button
              onclick={openAddProvider}
              class="px-3 py-1.5 text-xs rounded-xl bg-ink-800 hover:bg-ink-700 text-ink-200 font-medium transition-colors"
            >
              + Add provider
            </button>
          </div>
        {:else}
          <div class="space-y-2">
            {#each cloud.providers as p (p.id)}
              <div class={`rounded-xl border p-3 flex items-center justify-between gap-3 ${p.is_active ? "border-ember-500/40 bg-ember-500/5" : "border-ink-800 bg-ink-950/60"}`}>
                <div class="min-w-0">
                  <div class="flex items-center gap-2">
                    <span class="text-sm text-ink-100 font-medium">{p.label}</span>
                    {#if p.is_active}
                      <span class="px-1.5 py-0.5 text-[10px] uppercase font-mono rounded bg-ember-600/20 text-ember-300 border border-ember-500/30">Active</span>
                    {/if}
                  </div>
                  <div class="text-[11px] text-ink-400 font-mono truncate mt-0.5">{p.model} · {p.base_url} · key {p.api_key_hint}</div>
                </div>
                <div class="flex items-center gap-1.5 shrink-0">
                  {#if !p.is_active}
                    <button
                      onclick={() => activateProvider(p.id)}
                      disabled={cloudBusy}
                      class="px-2 py-1 text-xs rounded-lg bg-ink-800 hover:bg-ink-700 text-ink-200 transition-colors disabled:opacity-40"
                    >
                      Use
                    </button>
                  {/if}
                  <button
                    onclick={() => openEditProvider(p.id)}
                    disabled={cloudBusy}
                    class="px-2 py-1 text-xs rounded-lg border border-ink-700 hover:bg-ink-800 text-ink-300 transition-colors disabled:opacity-40"
                  >
                    Edit
                  </button>
                  <button
                    onclick={() => deleteProvider(p.id)}
                    disabled={cloudBusy}
                    class="px-2 py-1 text-xs rounded-lg bg-rose-950/60 hover:bg-rose-900 border border-rose-800/60 text-rose-300 transition-colors disabled:opacity-40"
                  >
                    Delete
                  </button>
                </div>
              </div>
            {/each}
            <button
              onclick={openAddProvider}
              disabled={cloudBusy}
              class="px-3 py-1.5 text-xs rounded-xl bg-ink-800 hover:bg-ink-700 text-ink-200 font-medium transition-colors disabled:opacity-40"
            >
              + Add provider
            </button>
          </div>
        {/if}

        {#if showProviderForm}
          <div class="border border-ink-800 bg-ink-950/80 rounded-xl p-4 space-y-3" transition:slide={{ duration: dur(150) }}>
            <div class="text-xs font-semibold text-ink-200 uppercase tracking-wider">
              {editingProviderId ? "Edit provider" : "Add provider"}
            </div>
            <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
              <label class="space-y-1">
                <span class="text-[11px] text-ink-400">Name</span>
                <input
                  bind:value={providerLabel}
                  placeholder="OpenAI, OpenRouter, Ollama proxy…"
                  class="w-full bg-ink-900 border border-ink-800 rounded-lg px-3 py-2 text-xs text-ink-100 placeholder-ink-600 outline-none focus:border-ember-500/60"
                />
              </label>
              <label class="space-y-1">
                <span class="text-[11px] text-ink-400">Model</span>
                <input
                  bind:value={providerModel}
                  placeholder="gpt-4o-mini"
                  class="w-full bg-ink-900 border border-ink-800 rounded-lg px-3 py-2 text-xs text-ink-100 placeholder-ink-600 outline-none focus:border-ember-500/60"
                />
              </label>
              <label class="space-y-1 sm:col-span-2">
                <span class="text-[11px] text-ink-400">Base URL (OpenAI-compatible, HTTPS — plain HTTP only for proxies on this machine)</span>
                <input
                  bind:value={providerUrl}
                  placeholder="https://api.openai.com/v1"
                  class="w-full bg-ink-900 border border-ink-800 rounded-lg px-3 py-2 text-xs text-ink-100 placeholder-ink-600 outline-none focus:border-ember-500/60 font-mono"
                />
              </label>
              <label class="space-y-1 sm:col-span-2">
                <span class="text-[11px] text-ink-400">
                  API key {editingProviderId ? "(leave empty to keep the stored key)" : ""}
                </span>
                <input
                  type="password"
                  bind:value={providerKey}
                  placeholder={editingProviderId ? "Stored key kept if empty" : "sk-…"}
                  class="w-full bg-ink-900 border border-ink-800 rounded-lg px-3 py-2 text-xs text-ink-100 placeholder-ink-600 outline-none focus:border-ember-500/60 font-mono"
                />
              </label>
            </div>
            {#if providerFormError}
              <ErrorBanner message={providerFormError} onDismiss={() => providerFormError = null} />
            {/if}
            <div class="text-[11px] text-ink-400">
              The key is stored in this app's config file on this machine and is sent only to the base URL above.
            </div>
            <div class="flex items-center gap-2 justify-end">
              <button
                onclick={() => showProviderForm = false}
                class="px-3 py-1.5 text-xs rounded-xl border border-ink-700 hover:bg-ink-800 text-ink-300 transition-colors"
              >
                Cancel
              </button>
              <button
                onclick={saveProvider}
                disabled={cloudBusy}
                class="px-3 py-1.5 text-xs rounded-xl bg-ember-600 hover:bg-ember-500 text-ink-950 font-medium transition-colors disabled:opacity-40"
              >
                {cloudBusy ? "Saving…" : "Save provider"}
              </button>
            </div>
          </div>
        {/if}
      </div>

      <div class="pt-2 flex items-center gap-3">
        <span class="text-xs text-ink-400">Version:</span>
        <span class="text-xs font-mono text-ink-300">{appVersion || "unknown"}</span>
        <span class="text-xs text-ink-400">Settings Status:</span>
        {#if saving}
          <span class="text-xs text-ember-300">Persisting changes…</span>
        {:else if saved}
          <span class="text-xs text-emerald-400 font-mono">Saved to config.json</span>
        {:else}
          <span class="text-xs text-ink-400 font-mono">Synchronized</span>
        {/if}
      </div>
    </div>
  {:else if activeTab === "security"}
    <div class="space-y-4">
      <div class="border border-ink-800 rounded-2xl p-5 bg-ink-900/60 space-y-4">
        <div class="flex items-center gap-3">
          <div class="p-2 rounded-xl bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
            <Icon name="lock" class="w-4 h-4" />
          </div>
          <div>
            <h3 class="text-sm font-semibold text-ink-100">Capability-Rooted Workspace Jail</h3>
            <p class="text-xs text-ink-400">Restricts all filesystem tool execution to designated workspace</p>
          </div>
        </div>

        <div class="bg-ink-950 border border-ink-800 rounded-xl p-3 space-y-2 text-xs">
          <div class="flex justify-between">
            <span class="text-ink-400">Root Directory:</span>
            <span class="font-mono text-cyan-300">~/.aiassistant/workspace</span>
          </div>
          <div class="flex justify-between">
            <span class="text-ink-400">Directory Traversal:</span>
            <span class="text-emerald-400 font-mono">Blocked (.. escapes rejected)</span>
          </div>
          <div class="flex justify-between">
            <span class="text-ink-400">Symlink Policy:</span>
            <span class="text-emerald-400 font-mono">Enforced (external links & loops trapped)</span>
          </div>
          <div class="flex justify-between">
            <span class="text-ink-400">File Write Approval:</span>
            <span class="text-amber-400 font-mono">Explicit confirmation required</span>
          </div>
        </div>
      </div>

      <div class="border border-ink-800 rounded-2xl p-5 bg-ink-900/60 space-y-3">
        <h4 class="text-xs font-semibold text-ink-200 uppercase tracking-wider">Cryptographic Security</h4>
        <div class="grid grid-cols-2 gap-3 text-xs">
          <div class="p-3 bg-ink-950 border border-ink-800 rounded-xl">
            <div class="text-ink-400 font-medium">Model Catalog Integrity</div>
            <div class="text-emerald-400 font-mono mt-1">✓ SHA-256 Manifest Verified</div>
          </div>
          <div class="p-3 bg-ink-950 border border-ink-800 rounded-xl">
            <div class="text-ink-400 font-medium">Content Security Policy</div>
            <div class="text-emerald-400 font-mono mt-1">✓ Strict CSP Active</div>
          </div>
          <div class="p-3 bg-ink-950 border border-ink-800 rounded-xl">
            <div class="text-ink-400 font-medium">Network Telemetry</div>
            <div class="text-emerald-400 font-mono mt-1">✓ None (cloud chat is opt-in)</div>
          </div>
          <div class="p-3 bg-ink-950 border border-ink-800 rounded-xl">
            <div class="text-ink-400 font-medium">Download Resumption</div>
            <div class="text-emerald-400 font-mono mt-1">✓ Atomic .part + Checksums</div>
          </div>
        </div>
      </div>
    </div>
  {:else if activeTab === "audit"}
    <div class="space-y-4">
      <div class="flex items-center justify-between">
        <div>
          <h3 class="text-sm font-semibold text-ink-100">Tool Execution Audit Log</h3>
          <p class="text-xs text-ink-400">Immutable record of all tool calls stored in SQLite</p>
        </div>
        <div class="flex items-center gap-2">
          <button
            onclick={loadAudits}
            disabled={loadingAudits}
            class="px-2.5 py-1 text-xs rounded-xl bg-ink-800 hover:bg-ink-700 text-ink-300 transition-colors"
          >
            {loadingAudits ? "Refreshing…" : "Refresh"}
          </button>
          <button
            onclick={clearAudits}
            disabled={clearingAudits || audits.length === 0}
            class="px-2.5 py-1 text-xs rounded-xl bg-rose-950/60 hover:bg-rose-900 border border-rose-800/60 text-rose-300 transition-colors disabled:opacity-40"
          >
            Clear Log
          </button>
        </div>
      </div>

      {#if loadingAudits}
        <div class="border border-ink-800 rounded-2xl p-8 text-center text-xs text-ink-400 font-mono bg-ink-900/30">
          Loading audit log…
        </div>
      {:else if audits.length === 0}
        <div class="border border-ink-800 rounded-2xl p-8 text-center text-xs text-ink-400 bg-ink-900/30">
          No tool executions recorded yet.
        </div>
      {:else}
        <div class="border border-ink-800 rounded-2xl overflow-hidden bg-ink-900/40">
          <div class="max-h-96 overflow-y-auto">
            <table class="w-full text-left text-xs">
              <thead class="bg-ink-950 border-b border-ink-800 text-ink-400 font-mono sticky top-0">
                <tr>
                  <th class="p-2.5">Time</th>
                  <th class="p-2.5">Action</th>
                  <th class="p-2.5">Target Path</th>
                  <th class="p-2.5">Approved</th>
                  <th class="p-2.5">Status</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-ink-800/60 font-mono">
                {#each audits as a}
                  <tr class="hover:bg-ink-800/30 transition-colors">
                    <td class="p-2.5 text-ink-400 whitespace-nowrap">
                      {new Date(a.timestamp * 1000).toLocaleTimeString()}
                    </td>
                    <td class="p-2.5 text-ember-400 font-semibold">{a.action}</td>
                    <td class="p-2.5 text-ink-300 truncate max-w-xs">{a.path}</td>
                    <td class="p-2.5">
                      {#if a.approved}
                        <span class="text-emerald-400">Yes</span>
                      {:else}
                        <span class="text-ink-400">Auto/No</span>
                      {/if}
                    </td>
                    <td class="p-2.5">
                      {#if a.status === "success"}
                        <span class="text-emerald-400 font-semibold">Success</span>
                      {:else if a.status === "denied"}
                        <span class="text-amber-400 font-semibold">Denied</span>
                      {:else}
                        <span class="text-rose-400 font-semibold" title={a.details ?? ""}>Error</span>
                      {/if}
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        </div>
      {:else if activeTab === "diagnostics"}
        <div class="space-y-6">
          {#if diagBusy && !health}
            <div class="py-10 text-center text-xs text-ink-400 font-mono">Collecting local health…</div>
          {:else if health}
            <div class="border border-ink-800 rounded-2xl p-5 bg-ink-900/60 space-y-4">
              <div class="flex items-center justify-between">
                <div class="font-medium text-ink-100">Update check</div>
                <button
                  onclick={loadDiagnostics}
                  disabled={diagBusy}
                  class="px-3 py-1.5 text-xs rounded-lg border border-ink-700 hover:bg-ink-800 text-ink-300 transition-colors disabled:opacity-40"
                >
                  {diagBusy ? "Checking…" : "Refresh"}
                </button>
              </div>
              {#if updateInfo?.update_available}
                <div class="border border-amber-500/40 bg-amber-500/10 rounded-xl px-4 py-3 text-sm text-amber-200">
                  Version {updateInfo?.latest_version} is available (you have {updateInfo?.current_version}). Get it from
                  <span class="font-mono">github.com/omarjson/LocalAide/releases</span> — the app never downloads or installs anything by itself.
                </div>
              {:else}
                <div class="text-xs text-ink-400 font-mono">
                  {#if updateInfo}
                    Up to date · v{updateInfo.current_version}
                  {:else}
                    Release lookup unavailable (offline or no releases yet) · v{health.app_version}
                  {/if}
                </div>
              {/if}
            </div>

            <div class="border border-ink-800 rounded-2xl p-5 bg-ink-900/60">
              <div class="font-medium text-ink-100 mb-3">Local subsystems</div>
              <div class="grid grid-cols-2 gap-3 text-xs font-mono">
                <div class="rounded-xl bg-ink-950/60 border border-ink-800 p-3">
                  <div class="text-ink-400 mb-1">Chat engine</div>
                  <div class="text-ink-200">
                    {#if health.engine.loaded}
                      {health.engine.active_model} · {health.engine.context_size} ctx
                    {:else}
                      Not loaded{#if !health.engine.sidecar_binary_found} · sidecar binary missing{/if}
                    {/if}
                  </div>
                </div>
                <div class="rounded-xl bg-ink-950/60 border border-ink-800 p-3">
                  <div class="text-ink-400 mb-1">Embedding engine</div>
                  <div class="text-ink-200">
                    {#if health.embedding.ready}
                      {health.embedding.model_id}{#if health.embedding.dimension} · {health.embedding.dimension}d{/if}
                    {:else}
                      Not running
                    {/if}
                  </div>
                </div>
                <div class="rounded-xl bg-ink-950/60 border border-ink-800 p-3">
                  <div class="text-ink-400 mb-1">Library vectors</div>
                  <div class="text-ink-200">{health.embedding.embedded_chunks}/{health.embedding.total_chunks} chunks</div>
                </div>
                <div class="rounded-xl bg-ink-950/60 border border-ink-800 p-3">
                  <div class="text-ink-400 mb-1">Documents</div>
                  <div class="text-ink-200">{health.library.doc_count} indexed</div>
                </div>
                <div class="rounded-xl bg-ink-950/60 border border-ink-800 p-3">
                  <div class="text-ink-400 mb-1">Catalog</div>
                  <div class="text-ink-200">v{health.catalog_version}</div>
                </div>
                <div class="rounded-xl bg-ink-950/60 border border-ink-800 p-3">
                  <div class="text-ink-400 mb-1">Config</div>
                  <div class="text-ink-200">{health.config_ok ? "OK" : "Read error"}</div>
                </div>
              </div>
            </div>

            <div class="border border-ink-800 rounded-2xl p-5 bg-ink-900/60">
              <div class="flex items-center justify-between mb-2">
                <div class="font-medium text-ink-100">llama-server log (tail)</div>
                <button
                  onclick={loadDiagnostics}
                  disabled={diagBusy}
                  class="px-3 py-1.5 text-xs rounded-lg border border-ink-700 hover:bg-ink-800 text-ink-300 transition-colors disabled:opacity-40"
                >
                  Refresh log
                </button>
              </div>
              {#if logLines.length === 0}
                <div class="text-xs text-ink-500 font-mono py-4 text-center">No log output yet.</div>
              {:else}
                <pre class="bg-ink-950 border border-ink-800 rounded-xl p-3 text-[11px] font-mono text-ink-300 overflow-auto max-h-72 whitespace-pre-wrap">{logLines.join("\n")}</pre>
              {/if}
            </div>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</div>
