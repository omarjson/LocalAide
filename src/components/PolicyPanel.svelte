<script lang="ts">
  // Permission rules and pre/post tool hooks, read and written through the
  // same typed policy_config_get/set commands the agent uses. Partial fields
  // are normalised before saving so the backend never sees half-empty rules.
  import { onMount } from "svelte";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import type {
    CapabilityAutoApprove,
    PermissionDecision,
    PermissionRule,
    PolicySettings,
  } from "../lib/api/bindings";

  const decisions: PermissionDecision[] = ["allow", "ask", "deny"];

  /** Read-only default: only reads run without asking. */
  const DEFAULT_CAPS: CapabilityAutoApprove = {
    read_files: true,
    edit_files: false,
    safe_commands: false,
    all_commands: false,
    browser: false,
    external_tools: false,
  };

  let rules = $state<PermissionRule[]>([]);
  let caps = $state<CapabilityAutoApprove>({ ...DEFAULT_CAPS });
  let preTool = $state("");
  let postTool = $state("");
  let timeout = $state(30);
  let loaded = $state(false);
  let saving = $state(false);
  let notice = $state<{ ok: boolean; text: string } | null>(null);

  const apply = (cfg: PolicySettings) => {
    rules = cfg.permissions.map((r) => ({
      action: r.action,
      resource: r.resource,
      decision: r.decision,
    }));
    caps = cfg.auto_approve_capabilities ?? { ...DEFAULT_CAPS };
    preTool = cfg.hooks.pre_tool ?? "";
    postTool = cfg.hooks.post_tool ?? "";
    timeout = cfg.hooks.timeout_secs;
  };

  const load = async () => {
    notice = null;
    try {
      apply(await api.policyConfigGet());
      loaded = true;
    } catch (e: unknown) {
      notice = { ok: false, text: `Policy could not load: ${errorMessage(e)}` };
    }
  };
  onMount(() => { void load(); });

  const addRule = () => {
    rules = [...rules, { action: "", resource: "", decision: "ask" }];
  };
  const removeRule = (index: number) => {
    rules = rules.filter((_, i) => i !== index);
  };

  const save = async () => {
    saving = true;
    notice = null;
    try {
      const clean = rules
        .filter((r) => r.action.trim() && r.resource.trim())
        .map((r) => ({
          action: r.action.trim(),
          resource: r.resource.trim(),
          decision: r.decision,
        }));
      const seconds = Math.round(Number(timeout));
      const saved = await api.policyConfigSet({
        permissions: clean,
        auto_approve_capabilities: { ...caps },
        hooks: {
          pre_tool: preTool.trim() || null,
          post_tool: postTool.trim() || null,
          timeout_secs: Number.isFinite(seconds) && seconds > 0 ? seconds : 30,
        },
      });
      apply(saved);
      notice = { ok: true, text: "Permission rules and hooks saved." };
    } catch (e: unknown) {
      notice = { ok: false, text: `Policy could not be saved: ${errorMessage(e)}` };
    } finally {
      saving = false;
    }
  };
</script>

<section class="mt-10 rounded-xl border border-ink-700 bg-ink-900 p-5" aria-labelledby="policy-title">
  <div class="flex flex-wrap items-start justify-between gap-3">
    <div>
      <h2 id="policy-title" class="flex items-center gap-2 text-lg font-semibold text-ink-50">
        <Icon name="shield_check" class="h-5 w-5 text-ink-400" />Permissions and tool hooks
      </h2>
      <p class="mt-1 text-sm text-ink-400">Decide which actions may run, and which local scripts run before or after a tool. All rules stay in your settings.</p>
    </div>
    <Button variant="secondary" disabled={saving} onclick={() => void load()}>
      <Icon name="refresh" class="h-4 w-4" />Reload
    </Button>
  </div>

  {#if notice}
    <p class={`mt-3 rounded-md p-2 text-sm ${notice.ok ? "bg-emerald-500/10 text-emerald-300" : "bg-rose-500/10 text-rose-300"}`} role={notice.ok ? "status" : "alert"}>{notice.text}</p>
  {/if}

  {#if loaded}
    <h3 class="mt-5 text-sm font-semibold text-ink-200">What runs without asking</h3>
    <p class="mt-1 text-xs text-ink-500">Read-only by default: only reading runs without asking. Switch on more only if you trust it — a missing or unknown setting always asks.</p>
    <div class="mt-2 space-y-2">
      <label class="flex items-start gap-2 text-sm text-ink-300">
        <input type="checkbox" bind:checked={caps.read_files} class="mt-0.5 h-4 w-4 rounded border-ink-700 bg-ink-950" />
        <span><span class="font-medium text-ink-100">Read files</span> — reading files and notes runs without asking.</span>
      </label>
      <label class="flex items-start gap-2 text-sm text-ink-300">
        <input type="checkbox" bind:checked={caps.edit_files} class="mt-0.5 h-4 w-4 rounded border-ink-700 bg-ink-950" />
        <span><span class="font-medium text-ink-100">Edit files</span> — changing files runs without asking each time.</span>
      </label>
      <label class="flex items-start gap-2 text-sm text-ink-300">
        <input type="checkbox" bind:checked={caps.safe_commands} class="mt-0.5 h-4 w-4 rounded border-ink-700 bg-ink-950" />
        <span><span class="font-medium text-ink-100">Safe commands</span> — tame terminal commands (making folders, checking status) run without asking.</span>
      </label>
      <label class="flex items-start gap-2 text-sm text-ink-300">
        <input type="checkbox" bind:checked={caps.all_commands} class="mt-0.5 h-4 w-4 rounded border-ink-700 bg-ink-950" />
        <span><span class="font-medium text-ink-100">All commands</span> — any terminal command runs without asking. Blocked commands still never run.</span>
      </label>
      <label class="flex items-start gap-2 text-sm text-ink-300">
        <input type="checkbox" bind:checked={caps.browser} class="mt-0.5 h-4 w-4 rounded border-ink-700 bg-ink-950" />
        <span><span class="font-medium text-ink-100">Browser</span> — searching the web and reading pages runs without asking.</span>
      </label>
      <label class="flex items-start gap-2 text-sm text-ink-300">
        <input type="checkbox" bind:checked={caps.external_tools} class="mt-0.5 h-4 w-4 rounded border-ink-700 bg-ink-950" />
        <span><span class="font-medium text-ink-100">External tools</span> — tools from MCP servers run without asking.</span>
      </label>
    </div>

    <h3 class="mt-7 text-sm font-semibold text-ink-200">Permission rules</h3>
    <div class="mt-2 grid grid-cols-[1fr_1fr_auto_auto] gap-2 px-1 text-xs font-semibold text-ink-500">
      <span>Action</span><span>Resource</span><span>Decision</span><span class="w-16"></span>
    </div>
    <div class="mt-1 space-y-2">
      {#each rules as rule, i (i)}
        <div class="grid grid-cols-[1fr_1fr_auto_auto] items-center gap-2">
          <label class="sr-only" for={`rule-action-${i}`}>Action for rule {i + 1}</label>
          <input id={`rule-action-${i}`} bind:value={rule.action} placeholder="e.g. shell" class="h-9 min-w-0 rounded-md border border-ink-700 bg-ink-950 px-2 text-sm text-ink-100 placeholder:text-ink-500" />
          <label class="sr-only" for={`rule-resource-${i}`}>Resource for rule {i + 1}</label>
          <input id={`rule-resource-${i}`} bind:value={rule.resource} placeholder="e.g. workspace/**" class="h-9 min-w-0 rounded-md border border-ink-700 bg-ink-950 px-2 text-sm text-ink-100 placeholder:text-ink-500" />
          <label class="sr-only" for={`rule-decision-${i}`}>Decision for rule {i + 1}</label>
          <select id={`rule-decision-${i}`} bind:value={rule.decision} class="h-9 rounded-md border border-ink-700 bg-ink-950 px-2 text-sm text-ink-100">
            {#each decisions as d}<option value={d}>{d}</option>{/each}
          </select>
          <Button variant="subtle" ariaLabel={`Remove rule ${i + 1}`} onclick={() => removeRule(i)}><Icon name="close" class="h-4 w-4" /></Button>
        </div>
      {/each}
      {#if rules.length === 0}<p class="text-sm text-ink-500">No rules yet. The default policy asks before each change.</p>{/if}
    </div>
    <div class="mt-2"><Button variant="subtle" onclick={addRule}><Icon name="plus" class="h-4 w-4" />Add rule</Button></div>

    <h3 class="mt-7 text-sm font-semibold text-ink-200">Tool hooks</h3>
    <p class="mt-1 text-xs text-ink-500">Optional local commands. Leave blank to run nothing.</p>
    <div class="mt-2 grid gap-3 md:grid-cols-3">
      <div>
        <label class="mb-1 block text-xs text-ink-400" for="hook-pre">Before a tool</label>
        <input id="hook-pre" bind:value={preTool} placeholder="command" class="h-9 w-full rounded-md border border-ink-700 bg-ink-950 px-2 font-mono text-sm text-ink-100 placeholder:text-ink-500" />
      </div>
      <div>
        <label class="mb-1 block text-xs text-ink-400" for="hook-post">After a tool</label>
        <input id="hook-post" bind:value={postTool} placeholder="command" class="h-9 w-full rounded-md border border-ink-700 bg-ink-950 px-2 font-mono text-sm text-ink-100 placeholder:text-ink-500" />
      </div>
      <div>
        <label class="mb-1 block text-xs text-ink-400" for="hook-timeout">Timeout (seconds)</label>
        <input id="hook-timeout" type="number" min="1" bind:value={timeout} class="h-9 w-full rounded-md border border-ink-700 bg-ink-950 px-2 text-sm text-ink-100" />
      </div>
    </div>

    <div class="mt-5 flex justify-end"><Button variant="primary" disabled={saving} onclick={() => void save()}>{saving ? "Saving…" : "Save policy"}</Button></div>
  {:else if !notice}
    <p class="mt-4 text-sm text-ink-400">Loading policy…</p>
  {/if}
</section>
