<script lang="ts">
  // A question from the model with tappable answers. `onAnswer` is only set
  // while the question is the newest thing in the chat.
  import Icon from "./Icon.svelte";
  import OptionButtons from "./OptionButtons.svelte";

  interface Props {
    question: string;
    options: string[];
    multiple?: boolean;
    /** Show the question text (tool questions); parsed ones already show it in the answer. */
    showQuestion?: boolean;
    answer?: string | null;
    /** Label of the model's recommended option, when it named one. */
    recommended?: string | null;
    onAnswer?: (text: string) => void;
    onOther?: () => void;
  }

  let { question, options, multiple = false, showQuestion = true, answer = null, recommended = null, onAnswer, onOther }: Props = $props();

  const chosen = $derived(answer ? (multiple ? answer.split(", ") : [answer]) : null);
  const active = $derived(!!onAnswer && !answer);
</script>

<div class={`q ${active ? "q-live" : ""}`}>
  {#if showQuestion}
    <p class="flex items-start gap-2 text-[15px] leading-6 text-ink-50"><Icon name="chat" class="mt-1 h-4 w-4 shrink-0 text-ink-400" /><span>{question}</span></p>
  {/if}
  {#if active || chosen}
    <div class={showQuestion ? "mt-3" : ""}>
      <OptionButtons
        options={options.map((label) => ({ label, recommended: recommended !== null && label === recommended }))}
        {multiple}
        allowOther={active}
        {active}
        {chosen}
        ariaLabel={question}
        onPick={(labels) => onAnswer?.(labels.join(", "))}
        onOther={() => onOther?.()}
      />
    </div>
  {:else if showQuestion}
    <ul class="mt-2 list-inside list-disc text-sm text-ink-300">
      {#each options as option (option)}<li>{option}</li>{/each}
    </ul>
  {/if}
</div>

<style>
  .q {
    border-radius: 0.75rem;
    border: 1px solid var(--color-ink-800);
    background: color-mix(in oklab, var(--color-ink-950) 60%, transparent);
    padding: 0.75rem 0.85rem;
  }
  .q-live {
    border-color: color-mix(in oklab, var(--color-ember-500) 40%, transparent);
  }
</style>
