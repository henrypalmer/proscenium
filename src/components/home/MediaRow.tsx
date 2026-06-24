import type { ReactNode } from "react";
import ScrollRow from "../common/ScrollRow";

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
      {/* ScrollRow keeps the negative-margin/padding breathing room so
          hovered/scaled cards aren't clipped, and adds hover scroll chevrons. */}
      <ScrollRow>
        {items.map((item, i) => (
          <div
            key={getKey(item)}
            className="prosc-enter w-[180px] shrink-0"
            style={{ animationDelay: `${Math.min(i, 10) * 30}ms` }}
          >
            {renderItem(item)}
          </div>
        ))}
      </ScrollRow>
    </section>
  );
}
