<script lang="ts">
  // The ghost mark, drawn in currentColor so it reads as a plain glyph in
  // chrome, message headers and tabs (GOAT's monochrome brand treatment).
  // One eye is punched out; the other is the blue accent. public/logo.svg
  // stays the tiled favicon.
  let { class: className = "" }: { class?: string } = $props();
</script>

<svg
  viewBox="0 0 512 512"
  class={className}
  role="img"
  aria-label="LocalAide logo"
  xmlns="http://www.w3.org/2000/svg"
>
  <mask id="la-ghost-eyes">
    <rect width="512" height="512" fill="#000" />
    <path
      d="M256 92c-88 0-152 66-152 156v170c0 14 14 22 25 15l32-21 32 21c7 5 15 5 22 0l27-20 27 20c7 5 15 5 22 0l32-21 32 21c11 7 25-1 25-15V248c0-90-64-156-152-156z"
      fill="#fff"
    />
    <circle cx="212" cy="236" r="22" fill="#000" />
  </mask>
  <path
    d="M256 92c-88 0-152 66-152 156v170c0 14 14 22 25 15l32-21 32 21c7 5 15 5 22 0l27-20 27 20c7 5 15 5 22 0l32-21 32 21c11 7 25-1 25-15V248c0-90-64-156-152-156z"
    fill="currentColor"
    mask="url(#la-ghost-eyes)"
  />
  <circle cx="300" cy="236" r="11" fill="var(--color-ember-500)" />
</svg>
