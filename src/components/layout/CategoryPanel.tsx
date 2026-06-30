import { useEffect, useMemo, useState } from "react";
import * as api from "../../lib/tauri";
import type { Category } from "../../types";

interface CategoryPanelProps {
  title: string;
  allLabel: string;
  categories: Category[];
  selectedId: string | null; // null = "All"
  onSelect: (id: string | null) => void;
  /** Enabled provider set; custom ordering applies only with a single provider
   * (spec §13, Milestone 29 / Milestone 39). */
  providerIds: string[];
  /** "live" | "movie" | "series" — scopes the custom order per section. */
  section: string;
}

type SortMode = "alpha" | "provider";

/**
 * Secondary sidebar listing categories/genres (spec §5.3): alphabetical by
 * default with provider-defined ordering as the alternative, and the special
 * "All" entry pinned on top. In provider order the list is drag-reorderable and
 * the custom order is persisted per provider+section (spec §13, Milestone 29).
 */
export default function CategoryPanel({
  title,
  allLabel,
  categories,
  selectedId,
  onSelect,
  providerIds,
  section,
}: CategoryPanelProps) {
  // Custom category ordering is per-provider (spec §13, M29); with several
  // providers merged we fall back to the default order (Milestone 39).
  const orderProviderId = providerIds.length === 1 ? providerIds[0] : null;
  const [sort, setSort] = useState<SortMode>("alpha");
  // Defaults to expanded; collapsing is transient — the panel remounts per
  // section so it always reopens expanded (spec §5.3, Milestone 19).
  const [collapsed, setCollapsed] = useState(false);
  // User's custom category order (ids), applied in provider mode (Milestone 29).
  const [customOrder, setCustomOrder] = useState<string[]>([]);
  const [dragId, setDragId] = useState<string | null>(null);

  useEffect(() => {
    if (!orderProviderId) {
      setCustomOrder([]);
      return;
    }
    let cancelled = false;
    void api.getCategoryOrder(orderProviderId, section).then(
      (ids) => {
        if (!cancelled) setCustomOrder(ids);
      },
      () => {
        if (!cancelled) setCustomOrder([]);
      },
    );
    return () => {
      cancelled = true;
    };
  }, [orderProviderId, section]);

  const sorted = useMemo(() => {
    if (sort === "alpha") {
      return [...categories].sort((a, b) =>
        a.name.toLowerCase().localeCompare(b.name.toLowerCase()),
      );
    }
    // Provider order, overlaid with the user's custom order where present.
    // Categories not in the custom order keep their backend position (stable
    // sort tail), so newly-added genres appear after the curated ones.
    if (customOrder.length === 0) return categories;
    const pos = new Map(customOrder.map((id, i) => [id, i]));
    return [...categories].sort(
      (a, b) => (pos.get(a.id) ?? Infinity) - (pos.get(b.id) ?? Infinity),
    );
  }, [categories, sort, customOrder]);

  const reorderable = sort === "provider" && orderProviderId !== null;

  const reorder = (fromId: string, toId: string) => {
    if (fromId === toId || !orderProviderId) return;
    const ids = sorted.map((c) => c.id);
    const from = ids.indexOf(fromId);
    const to = ids.indexOf(toId);
    if (from < 0 || to < 0) return;
    ids.splice(from, 1);
    ids.splice(to, 0, fromId);
    setCustomOrder(ids);
    void api.setCategoryOrder(orderProviderId, section, ids);
  };

  const itemClass = (active: boolean) =>
    `block w-full truncate rounded-md px-3 py-1.5 text-left text-sm transition-colors ${
      active ? "bg-zinc-800 font-medium text-white" : "text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100"
    }`;

  // Collapsed: a thin rail with an expand chevron, anchored where the panel was
  // (content area widens to fill the freed space).
  if (collapsed) {
    return (
      <nav
        className="flex w-10 shrink-0 flex-col items-center border-r border-zinc-800 pt-3"
        data-testid="category-panel"
        data-collapsed="true"
      >
        <button
          onClick={() => setCollapsed(false)}
          title={`Show ${title}`}
          data-testid="category-panel-toggle"
          className="rounded-md p-1.5 text-base leading-none text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100"
        >
          <span aria-hidden>»</span>
          <span className="sr-only">Show {title}</span>
        </button>
      </nav>
    );
  }

  return (
    <nav
      className="flex w-56 shrink-0 flex-col border-r border-zinc-800"
      data-testid="category-panel"
      data-collapsed="false"
    >
      <div className="flex items-center justify-between gap-1 px-4 pb-1 pt-3">
        <span className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
          {title}
        </span>
        <div className="flex items-center gap-1">
          <button
            onClick={() => setSort(sort === "alpha" ? "provider" : "alpha")}
            title={
              sort === "alpha"
                ? "Sorted A–Z — switch to provider order (drag to reorder)"
                : "Provider order — drag genres to reorder; switch to A–Z"
            }
            className="rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase text-zinc-500 hover:bg-zinc-900 hover:text-zinc-200"
          >
            {sort === "alpha" ? "A–Z" : "Provider"}
          </button>
          <button
            onClick={() => setCollapsed(true)}
            title={`Hide ${title}`}
            data-testid="category-panel-toggle"
            className="rounded-md px-1.5 py-0.5 text-base leading-none text-zinc-500 hover:bg-zinc-900 hover:text-zinc-200"
          >
            <span aria-hidden>«</span>
            <span className="sr-only">Hide {title}</span>
          </button>
        </div>
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
            className={`group flex items-center gap-1.5 ${itemClass(
              selectedId === category.id,
            )} ${dragId === category.id ? "opacity-50" : ""}`}
            onClick={() => onSelect(category.id)}
            title={reorderable ? "Drag to reorder" : category.name}
            draggable={reorderable}
            onDragStart={reorderable ? () => setDragId(category.id) : undefined}
            onDragOver={reorderable ? (e) => e.preventDefault() : undefined}
            onDrop={
              reorderable
                ? () => {
                    if (dragId) reorder(dragId, category.id);
                    setDragId(null);
                  }
                : undefined
            }
            onDragEnd={reorderable ? () => setDragId(null) : undefined}
          >
            {reorderable && (
              <span
                aria-hidden
                className="shrink-0 cursor-grab text-zinc-600 opacity-0 transition-opacity group-hover:opacity-100"
              >
                ⠿
              </span>
            )}
            <span className="truncate">{category.name}</span>
          </button>
        ))}
      </div>
    </nav>
  );
}
