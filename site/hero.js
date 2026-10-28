// The hero demo: drag to select an area (like Snapline's overlay) and click black boxes to show what auto-hide covered.
// Also wires the Copy buttons on code blocks. No dependencies.

const POINTS_WIDE = 1512; // a 14-inch MacBook Pro screen, in points; the size badge reads as if the demo were that screen

const screen = document.getElementById("screen");

if (screen) {
  const size = screen.querySelector(".size");
  let start = null;

  // x, y, w, h are fractions of the screen.
  const place = (x, y, w, h) => {
    const s = screen.style;
    s.setProperty("--x", `${x * 100}%`);
    s.setProperty("--y", `${y * 100}%`);
    s.setProperty("--w", `${w * 100}%`);
    s.setProperty("--h", `${h * 100}%`);
    const pointsHigh = POINTS_WIDE * screen.clientHeight / screen.clientWidth;
    size.textContent = `${Math.round(w * POINTS_WIDE)} × ${Math.round(h * pointsHigh)}`;
    screen.classList.toggle("tools-inside", x + w > 0.86);
    screen.classList.toggle("badge-inside", y < 0.08);
  };

  const point = (e) => {
    const r = screen.getBoundingClientRect();
    const clamp = (v) => Math.min(Math.max(v, 0), 1);
    return { x: clamp((e.clientX - r.left) / r.width), y: clamp((e.clientY - r.top) / r.height), r };
  };

  screen.addEventListener("pointerdown", (e) => {
    if (e.button !== 0 || e.target.closest("button")) return;
    start = point(e);
    screen.setPointerCapture(e.pointerId);
  });

  screen.addEventListener("pointermove", (e) => {
    if (!start) return;
    const p = point(e);
    const w = Math.abs(p.x - start.x), h = Math.abs(p.y - start.y);
    if (w * start.r.width < 4 && h * start.r.height < 4) return; // a click, not a drag: keep the current selection
    screen.classList.add("touched", "dragging");
    place(Math.min(p.x, start.x), Math.min(p.y, start.y), w, h);
  });

  const end = () => { start = null; screen.classList.remove("dragging"); };
  screen.addEventListener("pointerup", end);
  screen.addEventListener("pointercancel", end);

  for (const box of screen.querySelectorAll(".redact")) {
    box.addEventListener("click", () => {
      box.setAttribute("aria-pressed", box.getAttribute("aria-pressed") === "true" ? "false" : "true");
    });
  }
}

for (const button of document.querySelectorAll("[data-copy]")) {
  button.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(document.getElementById(button.dataset.copy).textContent);
      button.textContent = "Copied";
    } catch {
      button.textContent = "Select and copy";
    }
    setTimeout(() => { button.textContent = "Copy"; }, 1600);
  });
}
