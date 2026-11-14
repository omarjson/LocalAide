<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import Button from "../components/Button.svelte";
  import ConfirmDialog from "../components/ConfirmDialog.svelte";
  import EmptyState from "../components/EmptyState.svelte";
  import ErrorBanner from "../components/ErrorBanner.svelte";
  import Icon from "../components/Icon.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import TextField from "../components/TextField.svelte";
  import { api, errorMessage as errMsg } from "../lib/api/client";
  import type { Skill, SkillSuggestion } from "../lib/api/bindings";

  let skills = $state<Skill[]>([]);
  let suggestions = $state<SkillSuggestion[]>([]);
  let loading = $state(true);
  let busy = $state(false);
  let actionError = $state<string | null>(null);
  let savedNotice = $state<string | null>(null);
  let editing = $state<string | null>(null);
  let name = $state("");
  let description = $state("");
  let tags = $state("");
  let body = $state("");
  let showEditor = $state(false);
  let pendingDeletePath = $state<string | null>(null);

  const flash = (message: string) => {
    savedNotice = message;
    setTimeout(() => savedNotice = null, 2200);
  };

  const refresh = async () => {
    loading = true;
    try {
      skills = await api.skillsList();
      suggestions = await api.skillsSuggestions();
      actionError = null;
    } catch (e: unknown) {
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
    } catch (e: unknown) {
      actionError = errMsg(e);
    }
  };

  const save = async () => {
    if (!name.trim() || !body.trim()) return;
    busy = true;
    actionError = null;
    try {
      const tagList = tags.split(",").map((tag) => tag.trim()).filter(Boolean);
      if (editing) {
        await api.skillsUpdate(editing, name, description, tagList, body);
        flash("Skill saved.");
      } else {
        await api.skillsCreate(name, description, tagList, body);
        flash("Skill created.");
      }
      showEditor = false;
      await refresh();
    } catch (e: unknown) {
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
    } catch (e: unknown) {
      actionError = errMsg(e);
    } finally {
      busy = false;
    }
  };

  const askRemove = (skill: Skill) => {
    pendingDeletePath = skill.path;
  };

  const confirmRemove = async () => {
    const path = pendingDeletePath;
    pendingDeletePath = null;
    const skill = skills.find((item) => item.path === path);
    if (skill) await remove(skill);
  };

  const copyBody = async (skill: Skill) => {
    try {
      await navigator.clipboard.writeText(skill.body);
      flash("Prompt copied.");
    } catch (e: unknown) {
      actionError = errMsg(e);
    }
  };
</script>

<div class="mx-auto max-w-5xl pb-10">
  <PageHeader title="Skills" description="Save prompts you use often, then pick them from the chat box.">
    {#snippet actions()}
      <Button variant="primary" onclick={() => openCreate()}><Icon name="plus" class="h-[18px] w-[18px]" />New skill</Button>
    {/snippet}
  </PageHeader>

  {#if savedNotice}<div class="mb-4 text-sm text-emerald-300" in:fade={{ duration: 120 }}>{savedNotice}</div>{/if}
  {#if actionError}<div class="mb-4"><ErrorBanner message={actionError} onRetry={() => void refresh()} onDismiss={() => actionError = null} /></div>{/if}

  {#if suggestions.length > 0 && !showEditor}
    <div class="mb-5 rounded-lg border border-violet-500/30 bg-violet-500/5 p-4">
      <div class="flex items-center gap-2 font-semibold text-violet-200"><Icon name="skills" class="h-[18px] w-[18px]" />Suggested prompts</div>
      <p class="mt-1 text-sm text-ink-400">You used these prompts more than once. Save one for later?</p>
      <div class="mt-3 space-y-2">
        {#each suggestions.slice(0, 3) as suggestion (suggestion.prompt)}
          <div class="flex items-center justify-between gap-3 rounded-md border border-ink-700 bg-ink-950 p-3">
            <div class="min-w-0"><div class="truncate text-sm text-ink-300">{suggestion.prompt}</div><div class="text-sm text-ink-400">Used {suggestion.use_count} times</div></div>
            <Button variant="secondary" class="shrink-0" onclick={() => openCreate(suggestion.prompt)}>Save as skill</Button>
          </div>
        {/each}
      </div>
    </div>
  {/if}

  {#if showEditor}
    <div class="mb-5 rounded-lg border border-ink-700 bg-ink-900 p-4">
      <div class="flex items-center justify-between"><h2 class="font-semibold text-ink-50">{editing ? "Edit skill" : "New skill"}</h2><Button variant="subtle" ariaLabel="Close skill editor" onclick={() => showEditor = false}><Icon name="close" class="h-4 w-4" /></Button></div>
      <div class="mt-4 space-y-3">
        <div class="grid gap-3 sm:grid-cols-2"><TextField bind:value={name} label="Name" placeholder="Example: review a document" /><TextField bind:value={tags} label="Tags" placeholder="Work, writing" /></div>
        <TextField bind:value={description} label="Description" placeholder="What this prompt helps with" />
        <label class="block text-sm text-ink-400">Prompt<textarea bind:value={body} rows={8} placeholder="Write the prompt here…" class="mt-1 w-full resize-y rounded-md border border-ink-700 bg-ink-950 px-3 py-2 text-sm leading-6 text-ink-100 outline-none placeholder:text-ink-400 focus:border-edge"></textarea></label>
        <div class="flex justify-end gap-2"><Button variant="subtle" onclick={() => showEditor = false}>Cancel</Button><Button variant="primary" disabled={busy || !name.trim() || !body.trim()} onclick={() => void save()}>{busy ? "Saving…" : editing ? "Save changes" : "Create skill"}</Button></div>
      </div>
    </div>
  {/if}

  {#if loading}
    <div class="rounded-lg border border-ink-700 bg-ink-900 px-4 py-12 text-center text-sm text-ink-400">Loading skills…</div>
  {:else if skills.length === 0 && !showEditor}
    <EmptyState icon="skills" title="No skills yet" description="Save a prompt here, then type a slash in chat to use it.">
      {#snippet action()}<Button variant="primary" onclick={() => openCreate()}>Create a skill</Button>{/snippet}
    </EmptyState>
  {:else}
    <div class="space-y-3">
      {#each skills as skill (skill.path)}
        <div class="rounded-lg border border-ink-700 bg-ink-900 p-4 transition-colors hover:bg-veil/5">
          <div class="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
            <div class="min-w-0">
              <div class="flex flex-wrap items-center gap-2"><span class="font-semibold text-ink-50">/{skill.name}</span>{#each skill.tags as tag}<span class="rounded-full border border-ink-700 px-2 py-0.5 text-sm text-ink-400">{tag}</span>{/each}</div>
              {#if skill.description}<p class="mt-1 text-sm text-ink-400">{skill.description}</p>{/if}
              <p class="mt-2 line-clamp-2 text-sm leading-6 text-ink-300">{skill.body}</p>
            </div>
            <div class="flex shrink-0 items-center gap-2"><Button variant="secondary" onclick={() => void copyBody(skill)}>Copy</Button><Button variant="secondary" onclick={() => void openEdit(skill)}>Edit</Button><Button variant="danger" disabled={busy} onclick={() => askRemove(skill)}>Delete</Button></div>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

{#if pendingDeletePath}
  <ConfirmDialog title="Delete this skill?" message="This removes the saved prompt from this computer. You cannot undo it." confirmLabel="Delete skill" onConfirm={() => void confirmRemove()} onCancel={() => pendingDeletePath = null} />
{/if}
