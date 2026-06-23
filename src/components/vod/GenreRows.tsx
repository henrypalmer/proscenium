import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { Category } from "../../types";

/** Card width — matches the Home rows for a consistent feel. */
const CARD_W = "w-[180px]";

/** Popular first (case-insensitive whole-word match, §5.10), then the rest
 * alphabetically ascending — Popular is pulled out so it isn't repeated. */
function orderGenres(categories: Category[]): Category[] {
  const popular = categories.find((c) => /\bpopular\b/i.test(c.name));
  const rest = categories
    .filter((c) => c.id !== popular?.id)
    .sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase()));
  return popular ? [popular, ...rest] : rest;
}

interface GenreRowsProps<T> {
  categories: Category[];
  /** Re-run every row when this changes (provider / catalog switch). */
  resetKey: string;
  /** Fetch the first (capped) page of items for one genre. */
  fetchPage: (categoryId: string) => Promise<T[]>;
  getKey: (item: T) => string;
  renderCard: (item: T) => ReactNode;
  /** Jump to that genre's full grid (selects it in the side panel). */
  onSelectGenre: (categoryId: string) => void;
}

/**
 * The "All Movies/All Shows" overview (spec §5.4, Milestone 19): a vertical
 * stack of per-genre horizontal card strips — Popular first, then A–Z. Each
 * row lazy-loads its items when it nears the viewport so a many-genre catalog
 * doesn't fire one request per genre up front (§10). Empty genres are omitted.
 */
export default function GenreRows<T>({
  categories,
  resetKey,
  fetchPage,
  getKey,
  renderCard,
  onSelectGenre,
}: GenreRowsProps<T>) {
  const ordered = useMemo(() => orderGenres(categories), [categories]);
  return (
    <div className="h-full overflow-y-auto px-4 pb-10" data-testid="genre-rows">
      <div className="space-y-8 pt-4">
        {ordered.map((cat) => (
          <GenreRow
            key={`${resetKey}:${cat.id}`}
            category={cat}
            fetchPage={fetchPage}
            getKey={getKey}
            renderCard={renderCard}
            onSelectGenre={onSelectGenre}
          />
        ))}
      </div>
    </div>
  );
}

function GenreRow<T>({
  category,
  fetchPage,
  getKey,
  renderCard,
  onSelectGenre,
}: {
  category: Category;
  fetchPage: (categoryId: string) => Promise<T[]>;
  getKey: (item: T) => string;
  renderCard: (item: T) => ReactNode;
  onSelectGenre: (categoryId: string) => void;
}) {
  const ref = useRef<HTMLElement>(null);
  /** null = not yet loaded (skeleton); [] = loaded-empty (row omitted). */
  const [items, setItems] = useState<T[] | null>(null);
  const [visible, setVisible] = useState(false);

  // Reveal (and fetch) only when the row nears the viewport.
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const obs = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          setVisible(true);
          obs.disconnect();
        }
      },
      { rootMargin: "400px 0px" },
    );
    obs.observe(el);
    return () => obs.disconnect();
  }, []);

  useEffect(() => {
    if (!visible) return;
    let cancelled = false;
    void fetchPage(category.id).then(
      (its) => {
        if (!cancelled) setItems(its);
      },
      () => {
        if (!cancelled) setItems([]);
      },
    );
    return () => {
      cancelled = true;
    };
  }, [visible, category.id, fetchPage]);

  // Loaded and empty → omit the row entirely (the ref stayed mounted long
  // enough for the observer to fire and the fetch to resolve).
  if (items !== null && items.length === 0) return null;

  return (
    <section ref={ref} data-testid="genre-row" data-genre={category.name}>
      <button
        onClick={() => onSelectGenre(category.id)}
        data-testid="genre-row-title"
        className="group mb-3 flex max-w-full items-center gap-2 text-base font-semibold text-zinc-200 hover:text-white"
      >
        <span className="truncate">{category.name}</span>
        <span className="shrink-0 text-xs font-normal text-zinc-500 opacity-0 transition-opacity group-hover:opacity-100">
          See all ›
        </span>
      </button>
      {/* Negative margin + padding mirrors the Home rows so hovered/scaled
          cards keep their shape under overflow-x:auto (spec §9). */}
      <div className="-mx-2 flex gap-4 overflow-x-auto px-2 py-2">
        {items === null
          ? Array.from({ length: 8 }).map((_, i) => (
              <div key={i} className={`${CARD_W} shrink-0`}>
                <div className="aspect-[2/3] w-full animate-pulse rounded-lg bg-zinc-800" />
              </div>
            ))
          : items.map((item, i) => (
              <div
                key={getKey(item)}
                className={`prosc-enter ${CARD_W} shrink-0`}
                style={{ animationDelay: `${Math.min(i, 10) * 30}ms` }}
              >
                {renderCard(item)}
              </div>
            ))}
      </div>
    </section>
  );
}
