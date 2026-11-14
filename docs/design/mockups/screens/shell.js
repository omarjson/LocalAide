// Shared shell for the screen mockups: theme from ?theme=dark, nav pane, status bar.
const theme = new URLSearchParams(location.search).get("theme");
if (theme === "dark") document.documentElement.dataset.theme = "dark";
const icons = {
  Chat: '<path d="M4 5h16v11H9l-5 4z"/>',
  Search: '<circle cx="11" cy="11" r="6"/><path d="M20 20l-4.5-4.5"/>',
  Memory: '<path d="M6 3h9l3 3v15H6z"/><path d="M9 10h6M9 14h6"/>',
  Skills: '<path d="M12 3l1.8 4.6L18 9l-4.2 1.4L12 15l-1.8-4.6L6 9l4.2-1.4z"/>',
  Models: '<rect x="6" y="6" width="12" height="12" rx="2"/><path d="M9 3v3M15 3v3M9 18v3M15 18v3M3 9h3M3 15h3M18 9h3M18 15h3"/>',
  Settings: '<path d="M4 7h10M18 7h2M4 17h4M12 17h8"/><circle cx="16" cy="7" r="2"/><circle cx="10" cy="17" r="2"/>',
};
window.svg = (paths) => `<svg class="i" viewBox="0 0 24 24">${paths}</svg>`;
window.shell = (active, content) => {
  const item = (n) => `<button class="nav ${n === active ? "on" : ""}">${svg(icons[n])}${n}</button>`;
  document.body.innerHTML = `
  <div class="win">
    <nav class="pane">
      <div class="brand"><i></i>LocalAide</div>
      ${["Chat", "Search", "Memory", "Skills", "Models"].map(item).join("")}
      <div class="foot">${item("Settings")}</div>
    </nav>
    <section class="layer">${content}</section>
  </div>
  <footer class="status-bar"><span>Computer memory: 9.2 GB free of 16 GB</span><span>Models can use up to 6.4 GB</span><span class="r">Press ? for keyboard shortcuts</span></footer>`;
};
