<script lang="ts">
  import { slide } from "svelte/transition";
  import Icon, { type IconName } from "./Icon.svelte";
  import { dur, motion } from "../lib/motion";
  import OptionButtons from "./OptionButtons.svelte";
  import FileActions from "./FileActions.svelte";
  import type { ToolStatus } from "../lib/api/bindings";

  interface Props {
    title: string;
    detail: string;
    group: string;
    status: ToolStatus;
    summary: string;
    arguments: string;
    output: string;
    /** Shown while the tool waits for the user. */
    approval?: { preview: string } | null;
    onDecide?: (decision: "allow" | "allow_chat" | "deny") => void;
    /** Subagents: what it did, newest last. */
    steps?: string[];
    /** A skill the model saved or improved, with Undo. */
    skill?: { change_id: string; name: string; action: string; undone?: boolean } | null;
    onUndoSkill?: () => void;
    /** The folder the chat works in, for the file buttons. */
    folder?: string | null;
  }

  let {
    title,
    detail,
    group,
    status,
    summary,
    arguments: args,
    output,
    approval = null,
    onDecide,
    steps = [],
    skill = null,
    onUndoSkill,
    folder = null,
  }: Props = $props();
  const isSubagent = $derived(group === "subagent");
  const currentStep = $derived(steps.at(-1) ?? (status === "running" ? "Starting" : ""));
  // A file the model wrote: open it, open it with another app, or show it.
  const writtenFile = $derived(group === "files" && title === "Write a file" && status === "done" && detail ? detail : null);

  let open = $state(false);

  const ICONS: Record<string, IconName> = {
    web: "globe",
    notes: "memory",
    skills: "skills",
    files: "folder",
    mcp: "plug",
    shell: "terminal",
    subagent: "users",
    todos: "list-check",
  };
  const isShell = $derived(group === "shell");
  // Terminal results start with one line of JSON: exit code, time, and
  // whether the output was cut and saved.
  const shell = $derived.by(() => {
    if (!isShell || !output) return null;
    const nl = output.indexOf("\n");
    const head = nl < 0 ? output : output.slice(0, nl);
    try {
      const meta = JSON.parse(head) as { exit: number | null; ms: number; timed_out: boolean; cut: boolean; saved: string | null; cwd: string };
      return { ...meta, text: nl < 0 ? "" : output.slice(nl + 1) };
    } catch {
      return null;
    }
  });
  const decisions = $derived(
    isShell
      ? [
          { label: "Run", tone: "primary" as const, decision: "allow" as const },
          { label: "Allow for this chat", decision: "allow_chat" as const },
          { label: "Don't run", tone: "quiet" as const, decision: "deny" as const },
        ]
      : [
          { label: "Allow once", tone: "primary" as const, decision: "allow" as const },
          { label: "Allow for this chat", decision: "allow_chat" as const },
          { label: "Don't allow", tone: "quiet" as const, decision: "deny" as const },
        ],
  );
  // Commands show their output straight away; other tools fold it.
  $effect(() => {
    if (isShell && status !== "running" && output) open = true;
  });
  const icon = $derived(ICONS[group] ?? "wrench");
  const waiting = $derived(status === "waiting" && approval !== null);
  const hasDetails = $derived((args.trim() && args.trim() !== "{}") || output.trim().length > 0 || steps.length > 0);

</script>

<div class={`tool ${waiting ? "tool-waiting" : ""}`}>
  <button
    type="button"
    class="tool-head"
    aria-expanded={hasDetails ? open : undefined}
    disabled={!hasDetails}
    onclick={() => (open = !open)}
  >
    <span class={`tool-icon ${status === "running" ? "tool-icon-live" : ""}`}><Icon name={icon} class="h-3.5 w-3.5" /></span>
    <span class="min-w-0 flex-1 text-left">
      <span class="block truncate"><span class="font-medium text-ink-100">{title}</span>{#if detail}<span class="ml-1.5 text-ink-400">{detail}</span>{/if}</span>
      {#if isSubagent && currentStep}
        <span class={`block truncate text-xs ${status === "running" ? "text-ink-300" : "text-ink-500"}`} aria-live={status === "running" ? "polite" : undefined}>{status === "running" ? currentStep : `${steps.length} ${steps.length === 1 ? "step" : "steps"} · last: ${currentStep}`}</span>
      {/if}
    </span>
    <span class="flex shrink-0 items-center gap-1.5 text-xs">
      {#if status === "running"}
        <span class="h-3.5 w-3.5 animate-spin rounded-full border-2 border-ink-700 border-t-ember-500" aria-hidden="true"></span>
        <span class="text-ink-400">Working</span>
      {:else if status === "waiting"}
        <span class="rounded-full bg-ember-500/15 px-2 py-0.5 font-medium text-ember-200">Needs your OK</span>
      {:else if status === "done"}
        <span class="max-w-[12rem] truncate text-ink-500">{summary}</span>
        <Icon name="check" class="h-3.5 w-3.5 text-emerald-400" />
      {:else if status === "denied"}
        <span class="text-ink-500">Not allowed</span>
        <Icon name="x-circle" class="h-3.5 w-3.5 text-ink-500" />
      {:else}
        <span class="max-w-[14rem] truncate text-rose-300" title={summary}>{summary || "Failed"}</span>
        <Icon name="alert" class="h-3.5 w-3.5 text-rose-300" />
      {/if}
      {#if hasDetails}
        <Icon name="chevron-down" class={`h-3.5 w-3.5 text-ink-500 transition-transform duration-200 ${open ? "rotate-180" : ""}`} />
      {/if}
    </span>
  </button>

  {#if skill}
    <div class="flex flex-wrap items-center gap-2 border-t border-ink-800 px-3 py-2 text-sm">
      <Icon name="sparkle" class="h-4 w-4 shrink-0 text-ember-300" />
      <span class="min-w-0 flex-1 text-ink-300">
        {#if skill.undone}
          Undone. <span class="text-ink-500">The skill “{skill.name}” is as it was before.</span>
        {:else}
          LocalAide {skill.action === "created" ? "saved a new skill" : "improved the skill"} <span class="font-medium text-ink-100">“{skill.name}”</span> from this chat.
        {/if}
      </span>
      {#if !skill.undone && onUndoSkill}
        <button type="button" class="undo-btn" onclick={onUndoSkill}><Icon name="undo" class="h-3.5 w-3.5" />Undo</button>
      {/if}
    </div>
  {/if}

  {#if writtenFile}
    <div class="border-t border-ink-800 px-3 py-2">
      <FileActions path={writtenFile} {folder} />
    </div>
  {/if}

  {#if open && isSubagent}
    <div class="space-y-2 border-t border-ink-800 px-3 pb-3 pt-2.5" transition:slide={{ duration: dur(motion.base) }}>
      {#if steps.length > 0}
        <ol class="space-y-1">
          {#each steps as step, i (i)}
            <li class="flex items-start gap-2 text-[13px] leading-5 text-ink-400"><span class="mt-[7px] h-1.5 w-1.5 shrink-0 rounded-full bg-ink-600"></span><span class="min-w-0 break-words">{step}</span></li>
          {/each}
        </ol>
      {/if}
      {#if output.trim()}
        <div>
          <div class="mb-1 text-xs font-semibold text-ink-500">Report</div>
          <pre class="tool-pre max-h-72">{output}</pre>
        </div>
      {/if}
    </div>
  {:else if waiting && approval}
    <div class="border-t border-ink-800 px-3 pb-3 pt-2.5" transition:slide={{ duration: dur(motion.base) }}>
      <p class="text-sm font-medium text-ink-100">{isShell ? "Run this command?" : "LocalAide wants to do this."} <span class="font-normal text-ink-400">Nothing happens until you choose.</span></p>
      {#if approval.preview.trim()}
        <pre class={`mt-2 max-h-48 ${isShell ? "term" : "tool-pre"}`}>{approval.preview}</pre>
      {/if}
      <div class="mt-3">
        <OptionButtons
          options={decisions}
          ariaLabel={isShell ? "Run this command?" : `Allow: ${title}`}
          onPick={([label]) => onDecide?.(decisions.find((d) => d.label === label)?.decision ?? "deny")}
        />
      </div>
    </div>
  {/if}

  {#if open && shell}
    <div class="border-t border-ink-800 px-3 pb-3 pt-2.5" transition:slide={{ duration: dur(motion.base) }}>
      <div class="term">
        <div class="term-cmd"><span class="text-ember-300">$</span> {detail}{#if shell.cwd}<span class="ml-2 text-ink-500">in workspace/{shell.cwd}</span>{/if}</div>
        {#if shell.text.trim()}<div class="term-out">{shell.text}</div>{:else}<div class="term-out text-ink-500">(no output)</div>{/if}
      </div>
      <div class="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-ink-500">
        <span class={`rounded px-1.5 py-0.5 font-mono ${shell.exit === 0 ? "bg-emerald-500/10 text-emerald-300" : "bg-rose-500/10 text-rose-300"}`}>{shell.timed_out ? "time limit" : `exit ${shell.exit ?? "?"}`}</span>
        <span>{shell.ms < 1000 ? `${shell.ms} ms` : `${(shell.ms / 1000).toFixed(1)} s`}</span>
        {#if shell.cut}<span>Output shortened{shell.saved ? `; full text in workspace/${shell.saved}` : ""}</span>{/if}
      </div>
    </div>
  {:else if open && isShell && output && status !== "running"}
    <div class="border-t border-ink-800 px-3 pb-3 pt-2.5" transition:slide={{ duration: dur(motion.base) }}>
      <div class="term"><div class="term-cmd"><span class="text-ember-300">$</span> {detail}</div></div>
      <p class="mt-2 flex items-start gap-2 text-[13px] leading-5 text-rose-300"><Icon name="shield" class="mt-0.5 h-4 w-4 shrink-0" />{output}</p>
    </div>
  {:else if open && hasDetails && !isSubagent}
    <div class="space-y-2 border-t border-ink-800 px-3 pb-3 pt-2.5" transition:slide={{ duration: dur(motion.base) }}>
      {#if args.trim() && args.trim() !== "{}"}
        <div>
          <div class="mb-1 text-xs font-semibold text-ink-500">Input</div>
          <pre class="tool-pre max-h-40">{args}</pre>
        </div>
      {/if}
      {#if output.trim()}
        <div>
          <div class="mb-1 text-xs font-semibold text-ink-500">Result</div>
          <pre class="tool-pre max-h-64">{output}</pre>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .tool {
    overflow: hidden;
    border-radius: 0.7rem;
    border: 1px solid var(--color-ink-800);
    background: color-mix(in oklab, var(--color-veil) 3%, transparent);
    transition: border-color 150ms ease-out, background-color 150ms ease-out;
  }
  .tool:not(.tool-waiting):hover {
    background: color-mix(in oklab, var(--color-veil) 5%, transparent);
  }
  .tool-waiting {
    border-color: color-mix(in oklab, var(--color-ember-500) 50%, transparent);
  }
  .tool-head {
    display: flex;
    width: 100%;
    align-items: center;
    gap: 0.6rem;
    padding: 0.45rem 0.65rem;
    font-size: 0.875rem;
    transition: background-color 150ms ease-out;
  }
  .tool-head:not(:disabled):hover {
    background: color-mix(in oklab, var(--color-veil) 5%, transparent);
  }
  .tool-head:disabled {
    cursor: default;
  }
  .tool-icon {
    display: grid;
    height: 1.5rem;
    width: 1.5rem;
    flex-shrink: 0;
    place-items: center;
    border-radius: 0.4rem;
    background: var(--color-ink-800);
    color: var(--color-ink-300);
  }
  .tool-icon-live {
    color: var(--color-ember-300);
  }
  .term {
    overflow: auto;
    max-height: 22rem;
    border-radius: 0.5rem;
    border: 1px solid var(--color-ink-800);
    background: #0c0d0f;
    padding: 0.55rem 0.7rem;
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 12.5px;
    line-height: 1.3rem;
    color: #d6d8dc;
    white-space: pre-wrap;
    word-break: break-word;
  }
  div.term {
    white-space: normal;
  }
  .term-cmd,
  .term-out {
    white-space: pre-wrap;
  }
  .term-cmd {
    color: #f3f4f6;
    font-weight: 500;
  }
  .term-out {
    margin-top: 0.35rem;
    color: #b9bcc2;
  }
  .undo-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    height: 1.75rem;
    border-radius: 0.45rem;
    border: 1px solid var(--color-ink-700);
    padding: 0 0.6rem;
    font-size: 0.8125rem;
    color: var(--color-ink-200);
    transition: background-color 150ms ease-out, border-color 150ms ease-out;
  }
  .undo-btn:hover {
    border-color: var(--color-edge);
    background: color-mix(in oklab, var(--color-veil) 6%, transparent);
  }
  .tool-pre {
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-word;
    border-radius: 0.5rem;
    border: 1px solid var(--color-ink-800);
    background: var(--color-ink-950);
    padding: 0.5rem 0.65rem;
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 12.5px;
    line-height: 1.25rem;
    color: var(--color-ink-300);
  }
</style>
