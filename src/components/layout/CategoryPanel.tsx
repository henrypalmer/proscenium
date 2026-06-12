import { useMemo, useState } from "react";
import type { Category } from "../../types";

interface CategoryPanelProps {
  title: string;
  allLabel: string;
  categories: Category[];
  selectedId: string | null; // null = "All"
  onSelect: (id: string | null) => void;
}

type SortMode = "alpha" | "provider";

/**
 * Secondary sidebar listing categories/genres (spec §5.3): alphabetical by
 * default with provider-defined ordering as the alternative, and the
 * special "All" entry pinned on top.
 */
export default function CategoryPanel({
  title,
  allLabel,
  categories,
  selectedId,
  onSelect,
}: CategoryPanelProps) {
  const [sort, setSort] = useState<SortMode>("alpha");

  const sorted = useMemo(() => {
    if (sort === "provider") return categories; // backend order = sort_order
    return [...categories].sort((a, b) =>
      a.name.toLowerCase().localeCompare(b.name.toLowerCase()),
    );
  }, [categories, sort]);

  const itemClass = (active: boolean) =>
    `block w-full truncate rounded-md px-3 py-1.5 text-left text-sm transition-colors ${
      active ? "bg-zinc-800 font-medium text-white" : "text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100"
    }`;

  return (
    <nav
      className="flex w-56 shrink-0 flex-col border-r border-zinc-800"
      data-testid="category-panel"
    >
      <div className="flex items-center justify-between px-4 pb-1 pt-3">
        <span className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
          {title}
        </span>
        <button
          onClick={() => setSort(sort === "alpha" ? "provider" : "alpha")}
          title={
            sort === "alpha"
              ? "Sorted A–Z — switch to provider order"
              : "Provider order — switch to A–Z"
          }
          className="rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase text-zinc-500 hover:bg-zinc-900 hover:text-zinc-200"
        >
          {sort === "alpha" ? "A–Z" : "Provider"}
        </button>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto p-2 pt-1">
        <button
          className={itemClass(selectedId === null)}
          onClick={() => onSelect(null)}
        >
          {allLabel}
        </button>
        {sorted.map((category) => (
          <button
            key={category.id}
            className={itemClass(selectedId === category.id)}
            onClick={() => onSelect(category.id)}
            title={category.name}
          >
            {category.name}
          </button>
        ))}
      </div>
    </nav>
  );
}
