// Colour theme: "system" (the default, follows the device setting), "light" or "dark".
// Loaded in <head> without defer, so a saved choice applies before the page is drawn (no flash of the other theme).
// The choice is kept in this browser's localStorage only; "system" removes it.

(() => {
  const KEY = "snapline-theme";
  const root = document.documentElement;

  const saved = () => {
    try { return localStorage.getItem(KEY); } catch { return null; }   // storage can be blocked
  };

  const apply = (choice) => {
    if (choice === "light" || choice === "dark") root.dataset.theme = choice;
    else { delete root.dataset.theme; choice = "system"; }
    for (const button of document.querySelectorAll("[data-theme-choice]")) {
      button.setAttribute("aria-pressed", String(button.dataset.themeChoice === choice));
    }
  };

  apply(saved());

  document.addEventListener("DOMContentLoaded", () => {
    apply(saved());   // mark the right button now that the buttons exist
    for (const button of document.querySelectorAll("[data-theme-choice]")) {
      button.addEventListener("click", () => {
        const choice = button.dataset.themeChoice;
        try {
          if (choice === "system") localStorage.removeItem(KEY);
          else localStorage.setItem(KEY, choice);
        } catch { /* still switch for this page view */ }
        apply(choice);
      });
    }
  });
})();
