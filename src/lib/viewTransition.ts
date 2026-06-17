import { flushSync } from "react-dom";

/**
 * Cross-fade (and shared-element morph) a React state change using the browser's
 * View Transitions API. The WebView runtimes Proscenium targets are evergreen
 * Chromium (WebView2) and recent WebKit (macOS), both of which support this; on
 * anything older — or when the user prefers reduced motion — it degrades to a
 * plain, instant update.
 *
 * The `update` callback must perform the DOM mutation (here, a React state set).
 * It is run inside `flushSync` so React commits synchronously while the browser
 * holds the "before" snapshot, which is what lets the API animate old → new.
 *
 * Performance: the animation is driven by the compositor off the main thread, so
 * it stays smooth even over the large virtualized grids (spec §10) — no
 * per-frame JS and no animation library.
 */
type ViewTransitionDocument = Document & {
  startViewTransition?: (callback: () => void) => unknown;
};

function prefersReducedMotion(): boolean {
  return (
    window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false
  );
}

export function startViewTransition(update: () => void): void {
  const doc = document as ViewTransitionDocument;
  if (typeof doc.startViewTransition !== "function" || prefersReducedMotion()) {
    update();
    return;
  }
  doc.startViewTransition(() => flushSync(update));
}
