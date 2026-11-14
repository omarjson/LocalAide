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
})();
