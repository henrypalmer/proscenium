import { useEffect, type DependencyList } from "react";

/**
 * Shared keyboard plumbing (spec §5, Milestone 23). Centralizes the window-level
 * keydown subscription so shortcuts are wired consistently and the
 * editable-target guard lives in one place instead of being re-implemented per
 * component.
 */

/**
 * True when a key event originates from an editable element (a text input,
 * textarea, select, or contenteditable). Global single-key shortcuts use this
 * to avoid hijacking typing — e.g. the player's `m`/`f`/space keys must not fire
 * while the user is typing in the Live TV filter or a list-name field.
 */
export function isEditableTarget(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el || typeof el.tagName !== "string") return false;
  const tag = el.tagName;
  return (
    tag === "INPUT" ||
    tag === "TEXTAREA" ||
    tag === "SELECT" ||
    el.isContentEditable === true
  );
}

interface WindowKeydownOptions {
  /** Only subscribe while this is true (e.g. scope player keys to an open player). */
  enabled?: boolean;
  /** Skip the handler when focus is in an editable element (spec §5 focus discipline). */
  ignoreEditable?: boolean;
}

/**
 * Subscribe `handler` to window `keydown` for the lifetime of the component (or
 * while `enabled`). With `ignoreEditable`, events from text inputs are dropped
 * before the handler runs — the single place that focus discipline is enforced.
 */
export function useWindowKeydown(
  handler: (e: KeyboardEvent) => void,
  deps: DependencyList,
  { enabled = true, ignoreEditable = false }: WindowKeydownOptions = {},
): void {
  useEffect(() => {
    if (!enabled) return;
    const onKey = (e: KeyboardEvent) => {
      if (ignoreEditable && isEditableTarget(e.target)) return;
      handler(e);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled, ignoreEditable, ...deps]);
}
