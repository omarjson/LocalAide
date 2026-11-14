<script lang="ts">
  import ErrorBanner from "../components/ErrorBanner.svelte";
  import Icon from "../components/Icon.svelte";
  import { api, errorMessage as errMsg } from "../lib/api/client";
  import type { Skill, SkillSuggestion } from "../lib/api/bindings";
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";

  let skills = $state<Skill[]>([]);
  let suggestions = $state<SkillSuggestion[]>([]);
  let loading = $state(true);
  let busy = $state(false);
  let actionError = $state<string | null>(null);
  let savedNotice = $state<string | null>(null);

  // Editor state: null path = creating a new skill.
  let editing = $state<string | null>(null);
  let name = $state("");
  let description = $state("");
  let tags = $state("");
  let body = $state("");
  let showEditor = $state(false);

  const flash = (msg: string) => {
    savedNotice = msg;
    setTimeout(() => (savedNotice = null), 2200);
  };

  const refresh = async () => {
    loading = true;
    try {
      skills = await api.skillsList();
      suggestions = await api.skillsSuggestions();
    } catch (e) {
      actionError = errMsg(e);
    } finally {
      loading = false;
    }
  };

  onMount(refresh);

  const openCreate = (prefill?: string) => {
    editing = null;
    name = "";
    description = "";
    tags = "";
    body = prefill ?? "";
    showEditor = true;
    actionError = null;
  };

  const openEdit = async (skill: Skill) => {
    try {
      const full = await api.skillsRead(skill.path);
      editing = skill.path;
      name = full.name;
      description = full.description;
      tags = full.tags.join(", ");
      body = full.body;
      showEditor = true;
      actionError = null;
    } catch (e) {
      actionError = errMsg(e);
    }
  };

  const save = async () => {
    if (!name.trim() || !body.trim()) return;
    busy = true;
    actionError = null;
    try {
      const tagList = tags
        .split(",")
        .map((t) => t.trim())
        .filter(Boolean);
      if (editing) {
        await api.skillsUpdate(editing, name, description, tagList, body);
        flash("Skill saved.");
      } else {
        await api.skillsCreate(name, description, tagList, body);
        flash("Skill created.");
      }
      showEditor = false;
      await refresh();
    } catch (e) {
      actionError = errMsg(e);
    } finally {
      busy = false;
    }
  };

  const remove = async (skill: Skill) => {
    busy = true;
    actionError = null;
    try {
      await api.skillsDelete(skill.path);
      flash("Skill deleted.");
      await refresh();
    } catch (e) {
      actionError = errMsg(e);
    } finally {
      busy = false;
    }
  };

  const copyBody = async (skill: Skill) => {
    try {
      await navigator.clipboard.writeText(skill.body);
      flash("Prompt copied — paste it into chat or the system prompt.");
    } catch {
      actionError = "Clipboard copy was blocked by the webview.";
    }
  };
</script>

<div class="max-w-4xl mx-auto space-y-6">
  <div class="flex items-center justify-between">
    <div>
      <h2 class="text-lg font-semibold tracking-tight text-ink-100">Skills</h2>
      <p class="text-xs text-ink-400">
        Saved prompts you reuse — markdown files under ~/.aiassistant/skills/. Type / in chat to pull one in.
      </p>
    </div>
    <button
      onclick={() => openCreate()}
      class="px-3 py-1.5 text-xs font-medium rounded-xl bg-ember-600 hover:bg-ember-500 text-on-ember transition-colors"
    >
      + New skill
    </button>
  </div>

  {#if savedNotice}
    <div class="text-xs text-emerald-300 font-mono" in:fade={{ duration: 120 }}>{savedNotice}</div>
  {/if}
  {#if actionError}
    <ErrorBanner message={actionError} onDismiss={() => (actionError = null)} />
  {/if}

  {#if suggestions.length > 0 && !showEditor}
    <div class="border border-violet-500/30 rounded-2xl p-4 bg-violet-500/5 space-y-3">
      <div class="flex items-center gap-2 text-sm font-medium text-violet-200">
        <Icon name="skills" class="w-4 h-4" />
        Suggested from your recent chats
      </div>
      <p class="text-xs text-ink-400">
        You have reused these custom prompts several times this session. Save one as a skill? (Local usage counts only.)
      </p>
      {#each suggestions.slice(0, 3) as s (s.prompt)}
        <div class="border border-edge rounded-xl p-3 bg-ink-950/60 flex items-center justify-between gap-3">
          <div class="min-w-0">
            <div class="text-xs text-ink-300 truncate font-mono">{s.prompt}</div>
            <div class="text-[11px] text-ink-500 font-mono">used {s.use_count} times</div>
          </div>
          <button
            onclick={() => openCreate(s.prompt)}
            class="shrink-0 px-3 py-1.5 text-xs font-medium rounded-lg border border-violet-500/40 text-violet-300 hover:bg-violet-500/10 transition-colors"
          >
            Save as skill
          </button>
        </div>
      {/each}
    </div>
  {/if}

  {#if showEditor}
    <div class="border border-edge rounded-2xl p-5 bg-ink-900/60 space-y-3">
      <div class="flex items-center justify-between">
        <div class="font-medium text-ink-100">{editing ? "Edit skill" : "New skill"}</div>
        <button
          onclick={() => (showEditor = false)}
          class="text-xs text-ink-400 hover:text-ink-200 transition-colors"
        >
          Cancel
        </button>
      </div>
      <div class="grid grid-cols-2 gap-3">
        <label class="block">
          <span class="text-xs text-ink-400">Name</span>
          <input
            bind:value={name}
            placeholder="code-review"
            class="mt-1 w-full bg-ink-950 border border-edge rounded-lg px-3 py-2 text-sm text-ink-200 placeholder-ink-600 focus:outline-none focus:border-edge font-mono"
          />
        </label>
        <label class="block">
          <span class="text-xs text-ink-400">Tags (comma separated)</span>
          <input
            bind:value={tags}
            placeholder="coding, quality"
            class="mt-1 w-full bg-ink-950 border border-edge rounded-lg px-3 py-2 text-sm text-ink-200 placeholder-ink-600 focus:outline-none focus:border-edge font-mono"
          />
        </label>
      </div>
      <label class="block">
        <span class="text-xs text-ink-400">Description</span>
        <input
          bind:value={description}
          placeholder="What this skill does, shown in the slash menu"
          class="mt-1 w-full bg-ink-950 border border-edge rounded-lg px-3 py-2 text-sm text-ink-200 placeholder-ink-600 focus:outline-none focus:border-edge"
        />
      </label>
      <label class="block">
        <span class="text-xs text-ink-400">Prompt body</span>
        <textarea
          bind:value={body}
          rows="8"
          placeholder="The prompt text inserted into chat when this skill is picked…"
          class="mt-1 w-full bg-ink-950 border border-edge rounded-lg px-3 py-2 text-sm text-ink-200 placeholder-ink-600 focus:outline-none focus:border-edge font-mono resize-y"
        ></textarea>
      </label>
      <div class="flex justify-end">
        <button
          onclick={save}
          disabled={busy || !name.trim() || !body.trim()}
          class="px-4 py-2 text-xs font-medium rounded-xl bg-ember-600 hover:bg-ember-500 disabled:opacity-40 text-on-ember transition-colors"
        >
          {busy ? "Saving…" : editing ? "Save changes" : "Create skill"}
        </button>
      </div>
    </div>
  {/if}

  {#if loading}
    <div class="py-10 text-center text-xs text-ink-400 font-mono">Loading skills…</div>
  {:else if skills.length === 0 && !showEditor}
    <div class="border border-edge rounded-2xl p-8 bg-ink-900/40 text-center space-y-2">
      <Icon name="skills" class="w-8 h-8 mx-auto text-ink-600" />
      <p class="text-sm text-ink-300">No skills yet.</p>
      <p class="text-xs text-ink-400 max-w-md mx-auto">
        A skill is a reusable prompt — a code review checklist, a summarizer, your own editing style. Create one here,
        then type / in the chat box to use it. Everything stays in local markdown files.
      </p>
    </div>
  {:else}
    <div class="grid grid-cols-1 gap-3">
      {#each skills as s (s.path)}
        <div class="border border-edge rounded-xl p-4 bg-ink-900/40 hover:border-edge transition-colors">
          <div class="flex items-start justify-between gap-3">
            <div class="min-w-0">
              <div class="flex items-center gap-2 flex-wrap">
                <span class="font-medium text-sm text-ink-100 font-mono">/{s.name}</span>
                {#each s.tags as t (t)}
                  <span class="text-[10px] font-mono px-1.5 py-0.5 rounded border border-edge text-ink-400">{t}</span>
                {/each}
              </div>
              {#if s.description}
                <div class="text-xs text-ink-400 mt-1">{s.description}</div>
              {/if}
              <div class="text-xs text-ink-500 mt-2 line-clamp-2 font-mono">{s.body}</div>
            </div>
            <div class="flex items-center gap-1.5 shrink-0">
              <button
                onclick={() => copyBody(s)}
                class="px-2.5 py-1.5 text-xs font-medium rounded-lg border border-edge hover:bg-ink-800 text-ink-300 transition-colors"
                title="Copy the prompt body"
              >
                Copy
              </button>
              <button
                onclick={() => openEdit(s)}
                class="px-2.5 py-1.5 text-xs font-medium rounded-lg border border-edge hover:bg-ink-800 text-ink-300 transition-colors"
              >
                Edit
              </button>
              <button
                onclick={() => remove(s)}
                disabled={busy}
                class="px-2.5 py-1.5 text-xs font-medium rounded-lg text-rose-400 hover:bg-rose-500/10 border border-rose-500/20 transition-colors disabled:opacity-40"
              >
                Delete
              </button>
            </div>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>
