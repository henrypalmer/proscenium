import { useEffect, useRef, useState, type ReactNode } from "react";

/** Spec §5.5: each group shows at most 5 results inline. */
export const INLINE_LIMIT = 5;

interface SearchResultGroupProps<T> {
  title: string;
  items: T[];
  /** "list" for channel rows, "grid" for poster cards. */
  layout: "list" | "grid";
  testId: string;
  getKey: (item: T) => string;
  renderItem: (item: T) => ReactNode;
  /** Key of the keyboard-highlighted item in this group, if any (Milestone 23). */
  activeId?: string;
}

/** One content-type result group: header with count, up to 5 inline items,
 * and a "Show all [N] results" expander beyond that. */
export default function SearchResultGroup<T>({
  title,
  items,
  layout,
  testId,
  getKey,
  renderItem,
  activeId,
}: SearchResultGroupProps<T>) {
  const [expanded, setExpanded] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  // A new query (new items array) collapses the group again.
  useEffect(() => {
    setExpanded(false);
  }, [items]);

  // Keep the keyboard-highlighted item in view as the user arrows through.
  useEffect(() => {
    if (activeId == null) return;
    containerRef.current
      ?.querySelector('[data-active="true"]')
      ?.scrollIntoView({ block: "nearest" });
  }, [activeId]);

  if (items.length === 0) return null;
  const visible = expanded ? items : items.slice(0, INLINE_LIMIT);

  return (
    <section data-testid={testId} className="px-4 py-3">
      <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-zinc-500">
        {title}
        <span className="ml-2 font-normal normal-case text-zinc-600">
          {items.length}
        </span>
      </h3>
      <div
        ref={containerRef}
        className={
          layout === "grid"
            ? "grid grid-cols-[repeat(auto-fill,minmax(120px,1fr))] gap-4"
            : "overflow-hidden rounded-lg border border-zinc-900"
        }
      >
        {visible.map((item) => {
          const isActive = activeId === getKey(item);
          return (
            <div
              key={getKey(item)}
              data-active={isActive ? "true" : undefined}
              className={
                isActive ? "rounded-lg ring-2 ring-zinc-200" : undefined
              }
            >
              {renderItem(item)}
            </div>
          );
        })}
      </div>
      {items.length > INLINE_LIMIT && (
        <button
          onClick={() => setExpanded((e) => !e)}
          data-testid={`${testId}-show-all`}
          className="mt-2 text-xs font-medium text-zinc-400 transition-colors hover:text-zinc-100"
        >
          {expanded ? "Show fewer" : `Show all ${items.length} results`}
        </button>
      )}
    </section>
  );
}
