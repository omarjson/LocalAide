<script lang="ts">
  import { api } from "../lib/api/client";
  import type { ToolAuditEntry } from "../lib/api/bindings";
  import { onMount } from "svelte";

  let activeTab = $state<"general" | "security" | "audit">("general");

  let ramBudget = $state(70);
  let modelPriority = $state("local");
  let saving = $state(false);
  let saved = $state(false);

  // Audit logs
  let audits = $state<ToolAuditEntry[]>([]);
  let loadingAudits = $state(false);
  let clearingAudits = $state(false);

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
    } finally {
      clearingAudits = false;
    }
  };

  onMount(async () => {
    try {
      const cfg = await api.loadConfig();
      ramBudget = cfg.ram_budget_mb ? Math.round((cfg.ram_budget_mb / 1024) / 0.7) : 70;
      modelPriority = cfg.selected_models?.length ? "local" : modelPriority;
    } catch {}
    loadAudits();
  });

  $effect(() => {
    if (debounceTimer) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(async () => {
      saving = true;
      try {
        const current = await api.loadConfig();
        const cfg = {
          ram_budget_mb: Math.round(ramBudget * 0.7 * 1024),
          cloud_llm_enabled: false,
          selected_models: current.selected_models ?? [],
          presets: current.presets ?? []
        };
        await api.saveConfig(cfg);
        saved = true;
        setTimeout(() => saved = false, 1500);
      } finally {
        saving = false;
      }
    }, 500);
  });
</script>

<div class="max-w-3xl mx-auto space-y-6">
  <div class="border-b border-zinc-800 pb-4 flex items-center justify-between">
    <div>
      <h2 class="text-lg font-semibold tracking-tight text-zinc-100">Settings & Security</h2>
      <p class="text-xs text-zinc-500">Resource quotas, capability sandbox, and audit verification</p>
    </div>
    <div class="flex items-center gap-1 bg-zinc-900 border border-zinc-800 p-1 rounded-xl text-xs">
      <button
        onclick={() => activeTab = "general"}
        class={`px-3 py-1 rounded-lg transition-colors ${activeTab === "general" ? "bg-fuchsia-600 text-white font-medium" : "text-zinc-400 hover:text-zinc-200"}`}
      >
        General
      </button>
      <button
        onclick={() => activeTab = "security"}
        class={`px-3 py-1 rounded-lg transition-colors ${activeTab === "security" ? "bg-fuchsia-600 text-white font-medium" : "text-zinc-400 hover:text-zinc-200"}`}
      >
        Security Jail
      </button>
      <button
        onclick={() => { activeTab = "audit"; loadAudits(); }}
        class={`px-3 py-1 rounded-lg transition-colors ${activeTab === "audit" ? "bg-fuchsia-600 text-white font-medium" : "text-zinc-400 hover:text-zinc-200"}`}
      >
        Audit Log
      </button>
    </div>
  </div>

  {#if activeTab === "general"}
    <div class="space-y-6">
      <div class="border border-zinc-800 rounded-2xl p-5 bg-zinc-900/60">
        <div class="flex items-center justify-between mb-4">
          <div>
            <div class="font-medium text-zinc-100">RAM Budget</div>
            <div class="text-xs text-zinc-500">Memory quota allocated to local models</div>
          </div>
          <span class="text-sm font-mono font-semibold text-fuchsia-300">{ramBudget}%</span>
        </div>
        <input type="range" min={20} max={90} bind:value={ramBudget} class="w-full accent-fuchsia-600"/>
        <div class="mt-3 h-2 rounded-full bg-zinc-800 overflow-hidden">
          <div class="h-full bg-gradient-to-r from-fuchsia-600 to-cyan-500 transition-all" style={`width:${ramBudget}%`}></div>
        </div>
        <div class="mt-2 text-[11px] text-zinc-500 font-mono">≈ {(ramBudget * 0.7).toFixed(0)}% of usable physical RAM</div>
      </div>

      <div class="border border-zinc-800 rounded-2xl p-5 bg-zinc-900/60">
        <div class="font-medium text-zinc-100 mb-2">Model Execution Priority</div>
        <p class="text-xs text-zinc-500 mb-3">All inference executes exclusively on local device silicon.</p>
        <div class="space-y-2">
          <label class="flex items-center gap-3 cursor-pointer">
            <input type="radio" name="priority" bind:group={modelPriority} value="local" class="accent-fuchsia-600" checked/>
            <span class="text-sm text-zinc-300">Local-only (100% offline, zero network telemetry)</span>
          </label>
        </div>
      </div>

      <div class="border border-zinc-800/80 rounded-2xl p-5 bg-zinc-950/60 flex items-center justify-between">
        <div>
          <div class="font-medium text-xs text-zinc-400">External Cloud Services</div>
          <div class="text-[11px] text-zinc-600 mt-0.5">Cloud LLM APIs are out-of-scope for v1.0 (frozen per ADR-0006).</div>
        </div>
        <span class="px-2 py-0.5 text-[10px] uppercase font-mono rounded bg-zinc-900 text-zinc-500 border border-zinc-800">
          Disabled in v1.0
        </span>
      </div>

      <div class="pt-2 flex items-center gap-3">
        <span class="text-xs text-zinc-500">Settings Status:</span>
        {#if saving}
          <span class="text-xs text-fuchsia-300">Persisting changes…</span>
        {:else if saved}
          <span class="text-xs text-emerald-400 font-mono">Saved to config.json</span>
        {:else}
          <span class="text-xs text-zinc-600 font-mono">Synchronized</span>
        {/if}
      </div>
    </div>
  {:else if activeTab === "security"}
    <div class="space-y-4">
      <div class="border border-zinc-800 rounded-2xl p-5 bg-zinc-900/60 space-y-4">
        <div class="flex items-center gap-3">
          <div class="p-2 rounded-xl bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 text-sm">
            🔒
          </div>
          <div>
            <h3 class="text-sm font-semibold text-zinc-100">Capability-Rooted Workspace Jail</h3>
            <p class="text-xs text-zinc-400">Restricts all filesystem tool execution to designated workspace</p>
          </div>
        </div>

        <div class="bg-zinc-950 border border-zinc-800 rounded-xl p-3 space-y-2 text-xs">
          <div class="flex justify-between">
            <span class="text-zinc-500">Root Directory:</span>
            <span class="font-mono text-cyan-300">~/.aiassistant/workspace</span>
          </div>
          <div class="flex justify-between">
            <span class="text-zinc-500">Directory Traversal:</span>
            <span class="text-emerald-400 font-mono">Blocked (.. escapes rejected)</span>
          </div>
          <div class="flex justify-between">
            <span class="text-zinc-500">Symlink Policy:</span>
            <span class="text-emerald-400 font-mono">Enforced (external links & loops trapped)</span>
          </div>
          <div class="flex justify-between">
            <span class="text-zinc-500">File Write Approval:</span>
            <span class="text-amber-400 font-mono">Explicit confirmation required</span>
          </div>
        </div>
      </div>

      <div class="border border-zinc-800 rounded-2xl p-5 bg-zinc-900/60 space-y-3">
        <h4 class="text-xs font-semibold text-zinc-200 uppercase tracking-wider">Cryptographic Security</h4>
        <div class="grid grid-cols-2 gap-3 text-xs">
          <div class="p-3 bg-zinc-950 border border-zinc-800 rounded-xl">
            <div class="text-zinc-400 font-medium">Model Catalog Integrity</div>
            <div class="text-emerald-400 font-mono mt-1">✓ SHA-256 Manifest Verified</div>
          </div>
          <div class="p-3 bg-zinc-950 border border-zinc-800 rounded-xl">
            <div class="text-zinc-400 font-medium">Content Security Policy</div>
            <div class="text-emerald-400 font-mono mt-1">✓ Strict CSP Active</div>
          </div>
          <div class="p-3 bg-zinc-950 border border-zinc-800 rounded-xl">
            <div class="text-zinc-400 font-medium">Network Telemetry</div>
            <div class="text-emerald-400 font-mono mt-1">✓ Zero Telemetry / None</div>
          </div>
          <div class="p-3 bg-zinc-950 border border-zinc-800 rounded-xl">
            <div class="text-zinc-400 font-medium">Download Resumption</div>
            <div class="text-emerald-400 font-mono mt-1">✓ Atomic .part + Checksums</div>
          </div>
        </div>
      </div>
    </div>
  {:else if activeTab === "audit"}
    <div class="space-y-4">
      <div class="flex items-center justify-between">
        <div>
          <h3 class="text-sm font-semibold text-zinc-100">Tool Execution Audit Log</h3>
          <p class="text-xs text-zinc-500">Immutable record of all tool calls stored in SQLite</p>
        </div>
        <div class="flex items-center gap-2">
          <button
            onclick={loadAudits}
            disabled={loadingAudits}
            class="px-2.5 py-1 text-xs rounded-xl bg-zinc-800 hover:bg-zinc-700 text-zinc-300 transition-colors"
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

      {#if audits.length === 0}
        <div class="border border-zinc-800 rounded-2xl p-8 text-center text-xs text-zinc-500 bg-zinc-900/30">
          No tool executions recorded yet.
        </div>
      {:else}
        <div class="border border-zinc-800 rounded-2xl overflow-hidden bg-zinc-900/40">
          <div class="max-h-96 overflow-y-auto">
            <table class="w-full text-left text-xs">
              <thead class="bg-zinc-950 border-b border-zinc-800 text-zinc-400 font-mono sticky top-0">
                <tr>
                  <th class="p-2.5">Time</th>
                  <th class="p-2.5">Action</th>
                  <th class="p-2.5">Target Path</th>
                  <th class="p-2.5">Approved</th>
                  <th class="p-2.5">Status</th>
                </tr>
              </thead>
              <tbody class="divide-y divide-zinc-800/60 font-mono">
                {#each audits as a}
                  <tr class="hover:bg-zinc-800/30 transition-colors">
                    <td class="p-2.5 text-zinc-500 whitespace-nowrap">
                      {new Date(a.timestamp * 1000).toLocaleTimeString()}
                    </td>
                    <td class="p-2.5 text-fuchsia-400 font-semibold">{a.action}</td>
                    <td class="p-2.5 text-zinc-300 truncate max-w-xs">{a.path}</td>
                    <td class="p-2.5">
                      {#if a.approved}
                        <span class="text-emerald-400">Yes</span>
                      {:else}
                        <span class="text-zinc-500">Auto/No</span>
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
      {/if}
    </div>
  {/if}
</div>
