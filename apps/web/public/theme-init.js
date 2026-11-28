// Runs before the page draws, so a light theme does not flash dark first.
// The choice saved by src/lib/theme.ts wins. With none saved yet, the system
// setting decides. The app then confirms the choice saved in its settings.
(function () {
  var theme = null;
  try {
    theme = window.localStorage.getItem("localaide-theme");
  } catch (e) {
    theme = null;
  }
  if (theme !== "light" && theme !== "dark") {
    var prefersLight = window.matchMedia && window.matchMedia("(prefers-color-scheme: light)").matches;
    theme = prefersLight ? "light" : "dark";
  }
  document.documentElement.dataset.theme = theme;
  var accent = null;
  var font = null;
  var density = null;
  try {
    accent = window.localStorage.getItem("localaide-accent");
    font = window.localStorage.getItem("localaide-font");
    density = window.localStorage.getItem("localaide-density");
  } catch (e) {
    // Storage unavailable: defaults stay.
  }
  if (accent && accent !== "ember") document.documentElement.dataset.accent = accent;
  if (font && font !== "system") document.documentElement.dataset.font = font;
  if (density && density !== "comfortable") document.documentElement.dataset.density = density;
})();
