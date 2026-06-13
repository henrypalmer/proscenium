import { useEffect, useState, type ReactNode } from "react";

/** Spec §5.5: each group shows at most 5 results inline. */
const INLINE_LIMIT = 5;

interface SearchResultGroupProps<T> {
  title: string;
  items: T[];
  /** "list" for channel rows, "grid" for poster cards. */
  layout: "list" | "grid";
  testId: string;
  getKey: (item: T) => string;
  renderItem: (item: T) => ReactNode;
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
}: SearchResultGroupProps<T>) {
  const [expanded, setExpanded] = useState(false);

  // A new query (new items array) collapses the group again.
  useEffect(() => {
    setExpanded(false);
  }, [items]);

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
        className={
          layout === "grid"
            ? "grid grid-cols-[repeat(auto-fill,minmax(120px,1fr))] gap-4"
            : "overflow-hidden rounded-lg border border-zinc-900"
        }
      >
        {visible.map((item) => (
          <div key={getKey(item)}>{renderItem(item)}</div>
        ))}
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
