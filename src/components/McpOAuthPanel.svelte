<script lang="ts">
  import { onMount } from "svelte";
  import { slide } from "svelte/transition";
  import Button from "./Button.svelte";
  import ErrorBanner from "./ErrorBanner.svelte";
  import Icon from "./Icon.svelte";
  import SettingsGroup from "./SettingsGroup.svelte";
  import StatusBadge from "./StatusBadge.svelte";
  import { api, errorMessage } from "../lib/api/client";
  import { openInBrowser } from "../lib/links";
  import { dur } from "../lib/motion";
  import type {
    McpOAuthConfig,
    McpOAuthStatusDto,
    McpServer,
    McpServerSaveRequest,
  } from "../lib/api/bindings";

  // Public client metadata only. The PKCE verifier and the access token stay
  // on the backend — the token lives in the OS keychain and is never shown.
  let servers = $state<McpServer[]>([]);
  let selectedId = $state("");
  let authUrl = $state("");
  let tokenUrl = $state("");
  let clientId = $state("");
  let scopes = $state("");
  let redirectUri = $state("");
  let audience = $state("");
  let authorizeUrl = $state<string | null>(null);
  let signState = $state<string | null>(null);
  let code = $state("");
  let status = $state<string | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);

  const selected = $derived(servers.find((s) => s.id === selectedId) ?? null);

  const load = async () => {
    try {
      servers = (await api.mcpServers()).filter((s) => s.kind === "http");
      if (!servers.some((s) => s.id === selectedId)) selectedId = servers[0]?.id ?? "";
    } catch (e: unknown) {
      error = errorMessage(e);
    }
  };
  onMount(() => void load());

  // Reflect the saved settings when the user picks another server or a save
  // completes. Never writes a token: the DTO has no token field.
  $effect(() => {
    const oauth = selected?.oauth;
    authUrl = oauth?.authorize_url ?? "";
    tokenUrl = oauth?.token_url ?? "";
    clientId = oauth?.client_id ?? "";
    scopes = (oauth?.scopes ?? []).join(" ");
    redirectUri = oauth?.redirect_uri ?? "";
    audience = oauth?.audience ?? "";
    authorizeUrl = null;
    signState = null;
    code = "";
  });

  const formOauth = (): McpOAuthConfig => ({
    server_id: selectedId,
    client_id: clientId.trim(),
    authorize_url: authUrl.trim(),
    token_url: tokenUrl.trim(),
    redirect_uri: redirectUri.trim(),
    scopes: scopes
      .split(/[\s,]+/)
      .map((s) => s.trim())
      .filter(Boolean),
    audience: audience.trim() || null,
  });

  /** Save through the ordinary MCP command so the sign-in uses what is shown. */
  const persist = async (): Promise<boolean> => {
    const s = selected;
    if (!s) return false;
    busy = true;
    error = null;
    try {
      const req: McpServerSaveRequest = {
        id: s.id,
        name: s.name,
        enabled: s.enabled,
        auto_approve: s.auto_approve,
        kind: s.kind,
        command: s.command,
        args: s.args,
        cwd: s.cwd,
        url: s.url,
        // A hint without a value tells the backend to keep the stored header.
        env: s.env.map((p) => ({ name: p.name })),
        headers: s.headers.map((p) => ({ name: p.name })),
        oauth: formOauth(),
      };
      servers = (await api.saveMcpServer(req)).filter((x) => x.kind === "http");
      return true;
    } catch (e: unknown) {
      error = errorMessage(e);
      return false;
    } finally {
      busy = false;
    }
  };

  const save = async () => {
    status = null;
    if (await persist()) status = "Sign-in settings saved.";
  };

  const startSignIn = async (event: MouseEvent) => {
    status = null;
    if (!selected) return;
    if (!(await persist())) return;
    busy = true;
    try {
      const started = await api.mcpOAuthStart(selectedId);
      authorizeUrl = started.authorize_url;
      signState = started.state;
      status = "Browser opened. After signing in, paste the code from the callback address below.";
      openInBrowser(event, started.authorize_url);
    } catch (e: unknown) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  };

  const finishSignIn = async () => {
    status = null;
    if (!selected || !signState) {
      error = "Press Sign in first, then paste the code from the callback address.";
      return;
    }
    let pasted = code.trim();
    let state = signState;
    if (/^https?:\/\//i.test(pasted)) {
      try {
        const target = new URL(pasted);
        pasted = target.searchParams.get("code") ?? "";
        state = target.searchParams.get("state") || state;
      } catch {
        // Not a URL: treat the text as a bare authorization code.
      }
    }
    if (!pasted) {
      error = "Paste the code from the callback address.";
      return;
    }
    busy = true;
    try {
      const done: McpOAuthStatusDto = await api.mcpOAuthFinish({
        server_id: selected.id,
        state,
        code: pasted,
      });
      // Only the status text is displayed. It contains no token, verifier
      // or authorization code.
      status = done.message;
      authorizeUrl = null;
      signState = null;
      code = "";
    } catch (e: unknown) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  };
</script>

<div class="mt-9">
  <h2 class="text-xl font-semibold text-ink-50">Sign in to an MCP server</h2>
  <p class="mt-1 max-w-[70ch] text-sm leading-6 text-ink-400">
    For HTTP servers that use OAuth 2.1 with PKCE. Save the public client details, open the sign-in page, then paste the code from the callback address. The access token is kept in your OS keychain — it is never shown here and never written to the config file.
  </p>

  {#if error}<div class="mt-4"><ErrorBanner message={error} onDismiss={() => (error = null)} /></div>{/if}
  {#if status}
    <div class="mt-4 rounded-lg border border-emerald-500/20 bg-emerald-500/10 p-3 text-sm text-emerald-300" role="status">
      {status}
    </div>
  {/if}

  {#if servers.length === 0}
    <p class="mt-4 rounded-lg border border-ink-700 bg-ink-900 p-4 text-sm text-ink-400">
      Add an HTTP MCP server above, then come back to set up sign-in.
    </p>
  {:else}
    <SettingsGroup>
      <div class="space-y-4 px-[18px] py-4">
        <label class="block text-sm text-ink-400">
          Server
          <select bind:value={selectedId} class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 text-sm text-ink-100">
            {#each servers as server (server.id)}
              <option value={server.id}>{server.name}</option>
            {/each}
          </select>
        </label>

        <div class="grid gap-3 sm:grid-cols-2">
          <label class="text-sm text-ink-400 sm:col-span-2">
            Authorization URL
            <input bind:value={authUrl} spellcheck="false" placeholder="https://auth.example.com/authorize" class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 font-mono text-sm text-ink-100 placeholder:text-ink-500" />
          </label>
          <label class="text-sm text-ink-400 sm:col-span-2">
            Token URL
            <input bind:value={tokenUrl} spellcheck="false" placeholder="https://auth.example.com/token" class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 font-mono text-sm text-ink-100 placeholder:text-ink-500" />
          </label>
          <label class="text-sm text-ink-400">
            Client ID
            <input bind:value={clientId} spellcheck="false" placeholder="public-client-id" class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 font-mono text-sm text-ink-100 placeholder:text-ink-500" />
          </label>
          <label class="text-sm text-ink-400">
            Scopes (space separated)
            <input bind:value={scopes} spellcheck="false" placeholder="mcp.read mcp.write" class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 font-mono text-sm text-ink-100 placeholder:text-ink-500" />
          </label>
          <label class="text-sm text-ink-400">
            Redirect URI (optional)
            <input bind:value={redirectUri} spellcheck="false" placeholder="http://127.0.0.1:8765/callback" class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 font-mono text-sm text-ink-100 placeholder:text-ink-500" />
          </label>
          <label class="text-sm text-ink-400">
            Audience (optional)
            <input bind:value={audience} spellcheck="false" placeholder="Leave empty for most servers" class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 font-mono text-sm text-ink-100 placeholder:text-ink-500" />
          </label>
        </div>

        <p class="text-xs text-ink-500">
          Both URLs must use HTTPS, or plain HTTP on this computer. The callback address is
          <span class="font-mono text-ink-400">{redirectUri.trim() || "http://127.0.0.1:8765/callback"}</span>.
        </p>

        <div class="flex flex-wrap items-center gap-2">
          {#if selected?.oauth}<StatusBadge label="Sign-in set up" tone="ok" />{/if}
          <Button variant="secondary" disabled={busy} onclick={() => void save()}>Save settings</Button>
          <Button variant="primary" disabled={busy} onclick={(e) => void startSignIn(e)}>
            <Icon name="external" class="h-4 w-4" />Sign in
          </Button>
        </div>

        {#if signState}
          <div class="border-t border-ink-800 pt-4" transition:slide={{ duration: dur(150) }}>
            <label class="block text-sm text-ink-400">
              Finish sign-in — paste the code (or the whole callback address)
              <input bind:value={code} spellcheck="false" autocomplete="off" placeholder="code from the browser's callback address" class="mt-1 h-10 w-full rounded-md border border-ink-700 bg-ink-950 px-3 font-mono text-sm text-ink-100 placeholder:text-ink-500" />
            </label>
            <p class="mt-2 text-xs text-ink-500">
              The page can't connect to the callback address; copy its address bar and paste it here. Only the code is used.
            </p>
            {#if authorizeUrl}
              <p class="mt-2 text-xs text-ink-500">If no browser opened, open this link yourself: <span class="break-all font-mono text-ink-400">{authorizeUrl}</span></p>
            {/if}
            <div class="mt-3">
              <Button variant="primary" disabled={busy || !code.trim()} onclick={() => void finishSignIn()}>Finish sign-in</Button>
            </div>
          </div>
        {/if}
      </div>
    </SettingsGroup>
  {/if}
</div>
