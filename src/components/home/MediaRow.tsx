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
      <div className="flex gap-4 overflow-x-auto pb-2">
        {items.map((item) => (
          <div key={getKey(item)} className="w-[150px] shrink-0">
            {renderItem(item)}
          </div>
        ))}
      </div>
    </section>
  );
}
