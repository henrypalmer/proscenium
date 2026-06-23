import type { ReactNode } from "react";

interface MediaRowProps<T> {
  title: string;
  items: T[];
  getKey: (item: T) => string;
  renderItem: (item: T) => ReactNode;
  testId?: string;
}

/**
 * A labeled, horizontally-scrollable strip of cards for the Home screen
 * (spec §5.10). Each item is rendered side by side at a fixed width; the row
 * scrolls horizontally and is omitted entirely when it has no items.
 */
export default function MediaRow<T>({
  title,
  items,
  getKey,
  renderItem,
  testId,
}: MediaRowProps<T>) {
  if (items.length === 0) return null;
  return (
    <section data-testid={testId}>
      <h2 className="mb-3 text-base font-semibold text-zinc-200">
        {title}
        <span className="ml-2 text-sm font-normal text-zinc-600">
          {items.length}
        </span>
      </h2>
      {/* Negative margin + padding gives hovered/scaled cards vertical and
          horizontal breathing room: overflow-x:auto forces overflow-y to auto,
          which would otherwise clip a scaled card (spec §9 Motion). */}
      <div className="-mx-2 flex gap-4 overflow-x-auto px-2 py-2">
        {items.map((item, i) => (
          <div
            key={getKey(item)}
            className="prosc-enter w-[180px] shrink-0"
            style={{ animationDelay: `${Math.min(i, 10) * 30}ms` }}
          >
            {renderItem(item)}
          </div>
        ))}
      </div>
    </section>
  );
}
