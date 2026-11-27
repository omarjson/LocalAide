<script lang="ts">
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import Button from "./Button.svelte";
  import ConfirmDialog from "./ConfirmDialog.svelte";
  import ErrorBanner from "./ErrorBanner.svelte";
  import Icon, { type IconName } from "./Icon.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";
  import StatusBadge from "./StatusBadge.svelte";
  import Switch from "./Switch.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import { openInBrowser } from "../lib/links";
  import { dur } from "../lib/motion";
  import { moduleOn } from "../lib/dev.svelte";
  import type { McpPreset,
  ArtCraftPreset, McpServer, McpTool } from "../lib/api/bindings";

  let servers = $state<McpServer[]>([]);
  let presets = $state<McpPreset[]>([]);
  let artcraft = $state<ArtCraftPreset[]>([]);
  let artcraftBusy = $state<string | null>(null);
  let error = $state<string | null>(null);
  let notice = $state<string | null>(null);
  let busy = $state(false);
  let testing = $state<string | null>(null);
  let toolsById = $state<Record<string, McpTool[]>>({});
  let testErrors = $state<Record<string, string>>({});
  let pendingDelete = $state<McpServer | null>(null);

  // The add / edit form.
  type Pair = { name: string; value: string; hint: string };
  let showForm = $state(false);
  let editingId = $state<string | null>(null);
  let formKind = $state<"stdio" | "http">("http");
  let formName = $state("");
  let formCommand = $state("");
  let formArgs = $state("");
  let formCwd = $state("");
  let formUrl = $state("");
  let formPairs = $state<Pair[]>([]);
  let formAuto = $state(false);
  let formError = $state<string | null>(null);
  let formRequirement = $state("");

  let showImport = $state(false);
  let importText = $state("");

  const builtins: { icon: IconName; title: string; text: string }[] = [
    { icon: "globe", title: "Web search", text: "Searches and reads pages. Only while Web is on in the chat box." },
    { icon: "memory", title: "Notes and documents", text: "Searches your notes and library. Saving a note asks you first." },
    { icon: "folder", title: "Workspace files", text: "Reads files in the LocalAide workspace folder. Writing asks you first." },
    { icon: "skills", title: "Skills", text: "Reads your saved skills and follows them." },
  ];

  const load = async () => {
    try {
      [servers, presets, artcraft] = await Promise.all([api.mcpServers(), api.mcpPresets(), api.mcpArtcraftPresets().catch(() => [] as ArtCraftPreset[])]);
    } catch (e: unknown) {
      error = errorMessage(e);
    }
  };
  onMount(load);

  const addArtcraft = async (preset: ArtCraftPreset) => {
    artcraftBusy = preset.id;
    try {
      servers = await api.saveMcpServer({
        name: preset.id.replace("artcraft-", "Artcraft "),
        enabled: true,
        auto_approve: false,
        kind: "stdio",
        command: preset.command,
        args: preset.args,
        cwd: preset.cwd,
        url: "",
        env: Object.entries(preset.env).map(([name, value]) => ({ name, value: String(value) })),
        headers: [],
      });
      await load();
    } catch (e: unknown) {
      error = errorMessage(e);
    } finally {
      artcraftBusy = null;
    }
  };

  const openAdd = (preset?: McpPreset) => {
    editingId = null;
    formKind = preset?.kind ?? "http";
    formName = preset?.name ?? "";
    formCommand = preset?.command ?? "";
    formArgs = (preset?.args ?? []).join("\n");
    formCwd = "";
    formUrl = preset?.url ?? "";
    formPairs = [];
    formAuto = false;
    formError = null;
    formRequirement = preset?.requirement ?? "";
    showForm = true;
    showImport = false;
  };

  const openEdit = (server: McpServer) => {
    editingId = server.id;
    formKind = server.kind;
    formName = server.name;
    formCommand = server.command;
    formArgs = server.args.join("\n");
    formCwd = server.cwd ?? "";
    formUrl = server.url;
    formPairs = (server.kind === "stdio" ? server.env : server.headers).map((p) => ({ name: p.name, value: "", hint: p.hint }));
    formAuto = server.auto_approve;
    formError = null;
    formRequirement = "";
    showForm = true;
    showImport = false;
  };

  const save = async () => {
    busy = true;
    formError = null;
    const pairs = formPairs
      .filter((p) => p.name.trim())
      // An empty value on a stored pair keeps the stored value.
      .map((p) => (p.value === "" && p.hint ? { name: p.name.trim() } : { name: p.name.trim(), value: p.value }));
    try {
      servers = await api.saveMcpServer({
        id: editingId ?? undefined,
        name: formName,
        enabled: editingId ? (servers.find((s) => s.id === editingId)?.enabled ?? true) : true,
        auto_approve: formAuto,
        kind: formKind,
        command: formCommand,
        args: formArgs.split("\n").map((a) => a.trim()).filter(Boolean),
        cwd: formCwd.trim() || null,
        url: formUrl,
        env: formKind === "stdio" ? pairs : [],
        headers: formKind === "http" ? pairs : [],
      });
      showForm = false;
      notice = `${formName.trim()} is saved. Press Test to check that it starts.`;
    } catch (e: unknown) {
      formError = errorMessage(e);
    } finally {
      busy = false;
    }
  };

  const toggle = async (server: McpServer, change: { enabled?: boolean; auto_approve?: boolean }) => {
    try {
      servers = await api.toggleMcpServer(server.id, change);
    } catch (e: unknown) {
      error = errorMessage(e);
    }
  };

  const test = async (server: McpServer) => {
    testing = server.id;
    delete testErrors[server.id];
    try {
      toolsById[server.id] = await api.testMcpServer(server.id);
      servers = await api.mcpServers();
    } catch (e: unknown) {
      testErrors[server.id] = errorMessage(e);
    } finally {
      testing = null;
    }
  };

  const remove = async (server: McpServer) => {
    pendingDelete = null;
    try {
      servers = await api.deleteMcpServer(server.id);
    } catch (e: unknown) {
      error = errorMessage(e);
    }
  };

  const doImport = async () => {
    busy = true;
    error = null;
    try {
      const result = await api.importMcpServers(importText);
      servers = result.servers;
      notice =
        result.added > 0
          ? `Added ${result.added} server${result.added === 1 ? "" : "s"}${result.skipped ? `; ${result.skipped} were already here or not valid` : ""}.`
          : "Nothing new was added. Those servers are already in the list.";
      importText = "";
      showImport = false;
    } catch (e: unknown) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  };

  const added = (preset: McpPreset) =>
    servers.some((s) => (preset.kind === "http" ? s.url === preset.url : s.command === preset.command && s.args.join(" ").startsWith(preset.args.join(" "))));
</script>

<div>
  <h2 class="text-xl font-semibold text-ink-50">Tools and MCP</h2>
  <p class="mt-1 max-w-[65ch] text-sm leading-6 text-ink-400">
    In chat, the model decides for itself when a tool helps, so a simple question gets a direct answer. Switch tools on or off for a chat with the Tools button in the message box. Anything that changes a file or a note asks you first.
  </p>

  {#if error}<div class="mt-4"><ErrorBanner message={error} onDismiss={() => (error = null)} /></div>{/if}
  {#if notice}
    <div class="mt-4 flex items-start justify-between gap-3 rounded-lg border border-emerald-500/20 bg-emerald-500/10 p-3 text-sm text-emerald-300" role="status">
      <span>{notice}</span>
      <button type="button" aria-label="Dismiss" onclick={() => (notice = null)} class="grid h-6 w-6 shrink-0 place-items-center rounded-md hover:bg-emerald-500/10"><Icon name="close" class="h-3.5 w-3.5" /></button>
    </div>
  {/if}

  <h3 class="mb-2 mt-7 text-[15px] font-semibold text-ink-50">Built in</h3>
  <SettingsGroup>
    {#each builtins as b, i (b.title)}
      <div class={`flex items-center gap-4 px-[18px] py-3 ${i > 0 ? "border-t border-ink-800" : ""}`}>
        <span class="grid h-9 w-9 shrink-0 place-items-center rounded-md bg-ink-800 text-ink-200"><Icon name={b.icon} class="h-[18px] w-[18px]" /></span>
        <span class="min-w-0"><span class="block font-semibold text-ink-50">{b.title}</span><span class="block text-sm text-ink-400">{b.text}</span></span>
      </div>
    {/each}
  </SettingsGroup>

  <div class="mb-2 mt-8 flex flex-wrap items-center justify-between gap-2">
    <h3 class="text-[15px] font-semibold text-ink-50">MCP servers</h3>
    <div class="flex gap-2">
      <Button variant="subtle" onclick={() => { showImport = !showImport; showForm = false; }}><Icon name="file-plus" class="h-4 w-4" />Paste a config</Button>
      <Button variant="secondary" onclick={() => openAdd()}><Icon name="plus" class="h-4 w-4" />Add server</Button>
    </div>
  </div>
  <p class="mb-3 max-w-[65ch] text-sm leading-6 text-ink-400">
    MCP servers give the model more tools: library docs, GitHub, a browser and more. A local server is a program that runs on this computer with your permissions, so add only servers you trust. A remote server gets the tool requests the model makes.
  </p>
  {#if !moduleOn("mcp")}
    <p class="mb-3 rounded-lg border border-amber-500/30 bg-amber-500/10 p-3 text-sm text-amber-200">MCP servers are switched off in Settings → Developer → Modules, so chat does not use them.</p>
  {/if}

  {#if showImport}
    <div class="mb-4 rounded-lg border border-ink-700 bg-ink-900 p-4" transition:slide={{ duration: dur(150) }}>
      <label class="block text-sm text-ink-300" for="mcp-import">Paste the JSON from a server's instructions, or from Claude Desktop, Cursor, VS Code, OpenCode or Crush.</label>
      <textarea id="mcp-import" bind:value={importText} rows={7} spellcheck="false" placeholder={'{\n  "mcpServers": {\n    "context7": { "url": "https://mcp.context7.com/mcp" }\n  }\n}'} class="mt-2 w-full resize-y rounded-md border border-ink-700 bg-ink-950 p-3 font-mono text-[13px] leading-5 text-ink-100 placeholder:text-ink-500"></textarea>
      <div class="mt-3 flex justify-end gap-2">
        <Button variant="subtle" onclick={() => (showImport = false)}>Cancel</Button>
        <Button variant="primary" disabled={busy || !importText.trim()} onclick={() => void doImport()}>Add servers</Button>
      </div>
    </div>
  {/if}

  {#if showForm}
    <div class="mb-4 rounded-lg border border-ink-700 bg-ink-900 p-4" transition:slide={{ duration: dur(150) }}>
      <div class="flex items-center justify-between">
        <h4 class="font-semibold text-ink-50">{editingId ? "Edit server" : "Add server"}</h4>
        <Button variant="subtle" ariaLabel="Close the server form" onclick={() => (showForm = false)}><Icon name="close" class="h-4 w-4" /></Button>
      </div>
      {#if formRequirement}<p class="mt-2 text-sm text-ink-400">{formRequirement}</p>{/if}
      <div class="mt-3 inline-flex rounded-lg border border-ink-700 p-0.5" role="radiogroup" aria-label="Kind of server">
        {#each [{ id: "http", label: "Web address" }, { id: "stdio", label: "Program on this computer" }] as k (k.id)}
          <button type="button" role="radio" aria-checked={formKind === k.id} onclick={() => (formKind = k.id as "stdio" | "http")} class={`rounded-md px-3 py-1.5 text-sm transition-colors ${formKind === k.id ? "bg-ink-800 font-semibold text-ink-50" : "text-ink-400 hover:text-ink-100"}`}>{k.label}</button>
        {/each}
      </div>
      <div class="mt-4 grid gap-3 sm:grid-cols-2">
        <label class="text-sm text-ink-400 sm:col-span-2">Name<input bind:value={formName} placeholder="For example Context7" class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 text-sm text-ink-100 placeholder:text-ink-500" /></label>
        {#if formKind === "http"}
          <label class="text-sm text-ink-400 sm:col-span-2">Address<input bind:value={formUrl} placeholder="https://mcp.example.com/mcp" spellcheck="false" class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 font-mono text-sm text-ink-100 placeholder:text-ink-500" /></label>
        {:else}
          <label class="text-sm text-ink-400">Command<input bind:value={formCommand} placeholder="npx" spellcheck="false" class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 font-mono text-sm text-ink-100 placeholder:text-ink-500" /></label>
          <label class="text-sm text-ink-400">Folder to start in (optional)<input bind:value={formCwd} placeholder="Leave empty for the default" spellcheck="false" class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 font-mono text-sm text-ink-100 placeholder:text-ink-500" /></label>
          <label class="text-sm text-ink-400 sm:col-span-2">Arguments, one per line<textarea bind:value={formArgs} rows={3} spellcheck="false" placeholder={"-y\n@modelcontextprotocol/server-filesystem\nC:\\Users\\me\\Documents"} class="mt-1 w-full resize-y rounded-md border border-ink-700 bg-ink-950 p-3 font-mono text-[13px] leading-5 text-ink-100 placeholder:text-ink-500"></textarea></label>
        {/if}
      </div>
      <div class="mt-4">
        <div class="flex items-center justify-between">
          <span class="text-sm text-ink-300">{formKind === "http" ? "Headers (for example Authorization)" : "Environment variables (for example API keys)"}</span>
          <Button variant="subtle" onclick={() => formPairs.push({ name: "", value: "", hint: "" })}><Icon name="plus" class="h-4 w-4" />Add</Button>
        </div>
        {#each formPairs as pair, i (i)}
          <div class="mt-2 flex gap-2">
            <input bind:value={pair.name} aria-label="Name" placeholder={formKind === "http" ? "Authorization" : "API_KEY"} spellcheck="false" class="h-9 w-2/5 rounded-md border border-ink-700 bg-ink-950 px-3 font-mono text-sm text-ink-100 placeholder:text-ink-500" />
            <input bind:value={pair.value} type="password" autocomplete="off" aria-label="Value" placeholder={pair.hint ? `Saved ${pair.hint}. Type to replace` : formKind === "http" ? "Bearer your-token" : "value"} class="h-9 min-w-0 flex-1 rounded-md border border-ink-700 bg-ink-950 px-3 font-mono text-sm text-ink-100 placeholder:text-ink-500" />
            <button type="button" aria-label="Remove" onclick={() => formPairs.splice(i, 1)} class="grid h-9 w-9 shrink-0 place-items-center rounded-md text-ink-400 hover:bg-veil/5 hover:text-ink-100"><Icon name="trash" class="h-4 w-4" /></button>
          </div>
        {/each}
        <p class="mt-2 text-xs text-ink-500">Values stay in the config file on this computer and are left out of backups unless you include keys.</p>
      </div>
      <div class="mt-4 flex items-center gap-3 text-sm text-ink-300">
        <Switch checked={formAuto} label="Run this server's tools without asking each time" onchange={(on) => (formAuto = on)} />
        Run this server's tools without asking each time
      </div>
      {#if formError}<div class="mt-3"><ErrorBanner message={formError} onDismiss={() => (formError = null)} /></div>{/if}
      <div class="mt-4 flex justify-end gap-2">
        <Button variant="subtle" onclick={() => (showForm = false)}>Cancel</Button>
        <Button variant="primary" disabled={busy || !formName.trim()} onclick={() => void save()}>{editingId ? "Save" : "Add server"}</Button>
      </div>
    </div>
  {/if}

  {#if servers.length === 0}
    <div class="rounded-lg border border-ink-700 bg-ink-900 p-5 text-center text-sm text-ink-400">No MCP servers yet. Add one below, or paste a config.</div>
  {:else}
    <SettingsGroup>
      {#each servers as server, i (server.id)}
        <div class={`px-[18px] py-3.5 ${i > 0 ? "border-t border-ink-800" : ""}`}>
          <div class="flex flex-col gap-3 sm:flex-row sm:items-center">
            <span class="grid h-9 w-9 shrink-0 place-items-center rounded-md bg-ink-800 text-ink-200"><Icon name={server.kind === "http" ? "globe" : "terminal"} class="h-[18px] w-[18px]" /></span>
            <div class="min-w-0 flex-1">
              <div class="flex flex-wrap items-center gap-2">
                <span class="font-semibold text-ink-50">{server.name}</span>
                {#if server.connected}<StatusBadge label="Connected" tone="ok" dot />{/if}
                {#if server.auto_approve}<StatusBadge label="Runs without asking" tone="warn" />{/if}
              </div>
              <div class="truncate font-mono text-xs text-ink-500" title={server.kind === "http" ? server.url : `${server.command} ${server.args.join(" ")}`}>
                {server.kind === "http" ? server.url : `${server.command} ${server.args.join(" ")}`}
              </div>
            </div>
            <div class="flex shrink-0 flex-wrap items-center gap-1.5">
              <Switch checked={server.enabled} label={`Use ${server.name} in chat`} title="Use in chat" class="mr-1" onchange={(on) => void toggle(server, { enabled: on })} />
              <Button variant="secondary" disabled={testing === server.id} onclick={() => void test(server)}>{testing === server.id ? "Testing…" : "Test"}</Button>
              <Button variant="subtle" onclick={() => openEdit(server)}>Edit</Button>
              <Button variant="danger" onclick={() => (pendingDelete = server)}>Remove</Button>
            </div>
          </div>
          {#if testErrors[server.id]}
            <p class="mt-2 rounded-md bg-rose-500/10 px-3 py-2 text-sm text-rose-300">{testErrors[server.id]}</p>
          {:else if toolsById[server.id]}
            <div class="mt-2 rounded-md bg-ink-950 px-3 py-2 text-sm" transition:slide={{ duration: dur(150) }}>
              <p class="text-emerald-300">It works. {toolsById[server.id].length} tool{toolsById[server.id].length === 1 ? "" : "s"}:</p>
              <ul class="mt-1 max-h-40 space-y-0.5 overflow-y-auto text-ink-400">
                {#each toolsById[server.id] as tool (tool.name)}
                  <li class="truncate"><span class="font-mono text-ink-200">{tool.name}</span>{#if tool.description} · {tool.description}{/if}</li>
                {/each}
              </ul>
            </div>
          {/if}
        </div>
      {/each}
    </SettingsGroup>
  {/if}

  <h3 class="mb-2 mt-8 text-[15px] font-semibold text-ink-50">Popular servers</h3>
  <SettingsGroup>
    {#each presets as preset, i (preset.id)}
      <div class={`flex items-center gap-4 px-[18px] py-3 ${i > 0 ? "border-t border-ink-800" : ""}`}>
        <span class="grid h-9 w-9 shrink-0 place-items-center rounded-md bg-ink-800 text-ink-200"><Icon name={preset.kind === "http" ? "globe" : "terminal"} class="h-[18px] w-[18px]" /></span>
        <span class="min-w-0 flex-1">
          <span class="flex flex-wrap items-center gap-2">
            <span class="font-semibold text-ink-50">{preset.name}</span>
            <span class="text-xs text-ink-500">{preset.kind === "http" ? "Web address" : "Runs on this computer"}</span>
          </span>
          <span class="block text-sm text-ink-400">{preset.description}{preset.requirement ? ` ${preset.requirement}` : ""}</span>
        </span>
        <span class="flex shrink-0 items-center gap-1">
          <Button variant="subtle" ariaLabel={`About ${preset.name}`} title="Open its page" onclick={(e) => openInBrowser(e, preset.homepage)}><Icon name="external" class="h-4 w-4" /></Button>
          {#if added(preset)}
            <StatusBadge label="Added" tone="ok" />
          {:else}
            <Button variant="secondary" onclick={() => openAdd(preset)}>Add</Button>
          {/if}
        </span>
      </div>
    {/each}
  </SettingsGroup>

  {#if artcraft.length > 0}
    <h3 class="mb-2 mt-8 text-[15px] font-semibold text-ink-50">ArtCraft Connect</h3>
    <p class="mb-2 text-sm text-ink-400">The ArtCraft panels already on this computer — documents, slides, sheets, images — can join the chat as tool servers. One click adds each.</p>
    <SettingsGroup>
      {#each artcraft as preset, i (preset.id)}
        <div class={`flex items-center gap-4 px-[18px] py-3 ${i > 0 ? "border-t border-ink-800" : ""}`}>
          <span class="grid h-9 w-9 shrink-0 place-items-center rounded-md bg-ink-800 text-ink-400"><Icon name="code" class="h-[18px] w-[18px]" /></span>
          <span class="min-w-0 flex-1">
            <span class="flex flex-wrap items-center gap-2">
              <span class="font-semibold text-ink-50">{preset.id.replace("artcraft-", "")}</span>
              <span class="rounded-full border border-ink-700 px-2 py-0.5 text-xs text-ink-400">v{preset.version}</span>
              {#if !preset.ready}
                <span class="rounded-full border border-amber-500/40 bg-amber-500/10 px-2 py-0.5 text-xs text-amber-200">server file missing</span>
              {/if}
            </span>
            <span class="block truncate text-sm text-ink-400">{preset.description}</span>
          </span>
          {#if preset.added}
            <StatusBadge label="Added" tone="ok" />
          {:else}
            <Button variant="secondary" disabled={!preset.ready || artcraftBusy !== null} onclick={() => void addArtcraft(preset)}>{artcraftBusy === preset.id ? "Adding…" : "Add"}</Button>
          {/if}
        </div>
      {/each}
    </SettingsGroup>
  {/if}
</div>

{#if pendingDelete}
  <ConfirmDialog
    title="Remove this server?"
    message={`LocalAide will stop ${pendingDelete.name} and forget its settings, including saved tokens.`}
    confirmLabel="Remove server"
    onConfirm={() => pendingDelete && void remove(pendingDelete)}
    onCancel={() => (pendingDelete = null)}
  />
{/if}
