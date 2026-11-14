<script lang="ts">
  // One icon set: 24px grid, 1.75 stroke, rounded caps and joins, drawn inline so it
  // takes the surrounding text color and needs no icon font or extra package.
  export type IconName =
    | "chat"
    | "search"
    | "memory"
    | "skills"
    | "models"
    | "settings"
    | "dev"
    | "close"
    | "menu"
    | "alert"
    | "lock"
    | "shield"
    | "check"
    | "arrow-down"
    | "plus"
    | "file"
    | "file-plus"
    | "folder"
    | "folder-plus"
    | "more"
    | "sliders"
    | "refresh"
    | "download"
    | "trash"
    | "info"
    | "sun"
    | "computer"
    | "cloud"
    | "database"
    | "globe"
    | "chevron-right"
    | "external";

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
  {:else if name === "skills"}
    <path d="m12 3 1.9 4.6L18.5 9.5l-4.6 1.9L12 16l-1.9-4.6L5.5 9.5l4.6-1.9z" />
    <path d="M18 15.5 19 18l2.5 1L19 20l-1 2.5L17 20l-2.5-1 2.5-1z" />
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
  {:else if name === "plus"}
    <path d="M12 5v14M5 12h14" />
  {:else if name === "file"}
    <path d="M6 3h8l4 4v14H6z" />
    <path d="M14 3v5h4M9 13h6M9 17h6" />
  {:else if name === "file-plus"}
    <path d="M6 3h8l4 4v14H6z" />
    <path d="M14 3v5h4M12 12v6M9 15h6" />
  {:else if name === "folder"}
    <path d="M3 7h7l2 2h9v10H3z" />
  {:else if name === "folder-plus"}
    <path d="M3 7h7l2 2h9v10H3z" />
    <path d="M12 12v5M9.5 14.5h5" />
  {:else if name === "more"}
    <circle cx="5" cy="12" r="1" />
    <circle cx="12" cy="12" r="1" />
    <circle cx="19" cy="12" r="1" />
  {:else if name === "sliders"}
    <path d="M4 7h10M18 7h2M4 17h4M12 17h8" />
    <circle cx="16" cy="7" r="2" />
    <circle cx="10" cy="17" r="2" />
  {:else if name === "refresh"}
    <path d="M20 11a8 8 0 0 0-14.6-4L3 10" />
    <path d="M3 5v5h5M4 13a8 8 0 0 0 14.6 4L21 14" />
    <path d="M21 19v-5h-5" />
  {:else if name === "download"}
    <path d="M12 3v12m-5-5 5 5 5-5M5 21h14" />
  {:else if name === "trash"}
    <path d="M4 7h16M10 11v6M14 11v6M6 7l1 14h10l1-14M9 7V4h6v3" />
  {:else if name === "info"}
    <circle cx="12" cy="12" r="9" />
    <path d="M12 11v5M12 8h.01" />
  {:else if name === "sun"}
    <circle cx="12" cy="12" r="4" />
    <path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" />
  {:else if name === "computer"}
    <rect x="3" y="4" width="18" height="12" rx="2" />
    <path d="M8 20h8M12 16v4" />
  {:else if name === "cloud"}
    <path d="M7 18a4 4 0 0 1 0-8 5 5 0 0 1 9.6-1.5A3.5 3.5 0 1 1 17.5 18z" />
  {:else if name === "database"}
    <ellipse cx="12" cy="5" rx="7" ry="3" />
    <path d="M5 5v7c0 1.7 3.1 3 7 3s7-1.3 7-3V5M5 12v7c0 1.7 3.1 3 7 3s7-1.3 7-3v-7" />
  {:else if name === "globe"}
    <circle cx="12" cy="12" r="9" />
    <path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18" />
  {:else if name === "chevron-right"}
    <path d="m9 6 6 6-6 6" />
  {:else if name === "external"}
    <path d="M14 4h6v6M20 4l-9 9" />
    <path d="M18 13v6a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h6" />
  {/if}
</svg>
