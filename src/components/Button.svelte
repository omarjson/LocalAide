<script lang="ts">
  import type { Snippet } from "svelte";

  type ButtonVariant = "primary" | "secondary" | "subtle" | "danger";

  let {
    variant = "secondary",
    type = "button",
    disabled = false,
    onclick,
    title,
    ariaLabel,
    class: className = "",
    children,
  }: {
    variant?: ButtonVariant;
    type?: "button" | "submit" | "reset";
    disabled?: boolean;
    onclick?: (event: MouseEvent) => void;
    title?: string;
    ariaLabel?: string;
    class?: string;
    children?: Snippet;
  } = $props();

  const variants: Record<ButtonVariant, string> = {
    primary: "bg-ember-600 text-on-ember font-semibold hover:bg-ember-500",
    secondary: "bg-ink-900 border border-ink-700 border-b-edge text-ink-100 hover:bg-ink-800",
    subtle: "text-ink-300 hover:bg-veil/5 hover:text-ink-50",
    danger: "text-rose-400 hover:bg-rose-500/10",
  };
</script>

<button
  {type}
  {disabled}
  {title}
  aria-label={ariaLabel}
  onclick={onclick}
  class={`inline-flex h-8 items-center justify-center gap-2 rounded-md px-3 text-sm whitespace-nowrap transition-colors disabled:cursor-default disabled:opacity-50 ${variants[variant]} ${className}`}
>
  {@render children?.()}
</button>
