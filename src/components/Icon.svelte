<script lang="ts">
  // One icon set: 24px grid, 1.75 stroke, rounded caps and joins, drawn inline so it
  // takes the surrounding text color and needs no icon font or extra package.
  type IconName =
    | "chat"
    | "search"
    | "memory"
    | "models"
    | "settings"
    | "dev"
    | "close"
    | "menu"
    | "alert"
    | "lock"
    | "shield"
    | "check"
    | "arrow-down";

  interface Props {
    name: IconName;
    class?: string;
  }

  let { name, class: cls = "w-4 h-4" }: Props = $props();
</script>

<svg
  xmlns="http://www.w3.org/2000/svg"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.75"
  stroke-linecap="round"
  stroke-linejoin="round"
  class={cls}
  aria-hidden="true"
  focusable="false"
>
  {#if name === "chat"}
    <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
  {:else if name === "search"}
    <circle cx="11" cy="11" r="7" />
    <path d="m20 20-3.5-3.5" />
  {:else if name === "memory"}
    <path d="M6 3h11a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6z" />
    <path d="M9 7h6M9 11h6M9 15h4" />
  {:else if name === "models"}
    <rect x="6" y="6" width="12" height="12" rx="2" />
    <path d="M9 2v4M15 2v4M9 18v4M15 18v4M2 9h4M2 15h4M18 9h4M18 15h4" />
  {:else if name === "settings"}
    <path d="M4 6h9M17 6h3M4 12h3M11 12h9M4 18h11M19 18h1" />
    <circle cx="15" cy="6" r="2" />
    <circle cx="9" cy="12" r="2" />
    <circle cx="17" cy="18" r="2" />
  {:else if name === "dev"}
    <path d="m8 8-4 4 4 4M16 8l4 4-4 4M13.5 5l-3 14" />
  {:else if name === "close"}
    <path d="M6 6l12 12M18 6L6 18" />
  {:else if name === "menu"}
    <path d="M4 7h16M4 12h16M4 17h16" />
  {:else if name === "alert"}
    <path d="M12 4 2.5 20h19L12 4z" />
    <path d="M12 10v4" />
    <circle cx="12" cy="17" r="0.5" />
  {:else if name === "lock"}
    <rect x="5" y="11" width="14" height="9" rx="2" />
    <path d="M8 11V7a4 4 0 0 1 8 0v4" />
  {:else if name === "shield"}
    <path d="M12 3c3 2 6 2.5 8 2.5V12c0 5-3.5 8-8 9-4.5-1-8-4-8-9V5.5C6 5.5 9 5 12 3z" />
  {:else if name === "check"}
    <path d="m5 13 4 4 10-10" />
  {:else if name === "arrow-down"}
    <path d="M12 5v14m-6-6 6 6 6-6" />
  {/if}
</svg>
