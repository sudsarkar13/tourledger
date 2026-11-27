// The launch screen from index.html, taken away once there is something to show.

/** Short enough not to be in the way, long enough not to flash past. */
const SHORTEST_MS = 700;
const FADE_MS = 320;

/** Motion is switched off here or by the system. */
export const calm = () =>
  document.documentElement.classList.contains("reduced") ||
  window.matchMedia("(prefers-reduced-motion: reduce)").matches;

export function dismissSplash(): void {
  const splash = document.getElementById("splash");
  if (!splash || splash.classList.contains("done")) return;
  const still = calm();
  // performance.now() counts from the page starting, so real loading time counts.
  setTimeout(
    () => {
      splash.classList.add("done");
      setTimeout(() => splash.remove(), still ? 0 : FADE_MS);
    },
    still ? 0 : Math.max(0, SHORTEST_MS - performance.now()),
  );
}
