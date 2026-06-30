import { useCallback, useEffect, useRef, useState } from "react";
import * as api from "../../lib/tauri";
import CanonicalCard from "./CanonicalCard";
import type { CanonicalItem } from "../../types";

interface Props {
  kind: "movie" | "series";
  /** `null` = Popular (no genre filter). */
  genre: string | null;
  onActivate: (item: CanonicalItem) => void;
  /** imdbId of the card to name as the morph shared element, or null. */
  morphId: string | null;
  emptyNoun: string;
}

const SKELETON = 12;

/**
 * Paged canonical poster grid (M40 slice 1). Cinemeta returns ~50 per page, so
 * this loads incrementally on scroll rather than virtualizing a 12k catalog.
 */
export default function CanonicalGrid({
  kind,
  genre,
  onActivate,
  morphId,
  emptyNoun,
}: Props) {
  // `null` = first page still loading (render skeletons).
  const [items, setItems] = useState<CanonicalItem[] | null>(null);
  const [loadingMore, setLoadingMore] = useState(false);
  const [done, setDone] = useState(false);
  const skipRef = useRef(0);
  const sentinelRef = useRef<HTMLDivElement>(null);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Reset + first page whenever the kind or genre changes.
  useEffect(() => {
    let cancelled = false;
    setItems(null);
    setDone(false);
    skipRef.current = 0;
    scrollRef.current?.scrollTo({ top: 0 });
    void api.getCanonicalCatalog(kind, genre ?? undefined, undefined, 0).then(
      (page) => {
        if (cancelled) return;
        setItems(page);
        skipRef.current = page.length;
        setDone(page.length === 0);
      },
      () => {
        if (!cancelled) {
          setItems([]);
          setDone(true);
        }
      },
    );
    return () => {
      cancelled = true;
    };
  }, [kind, genre]);

  const loadMore = useCallback(() => {
    if (loadingMore || done || items === null) return;
    setLoadingMore(true);
    void api.getCanonicalCatalog(kind, genre ?? undefined, undefined, skipRef.current).then(
      (page) => {
        setItems((prev) => {
          const seen = new Set((prev ?? []).map((i) => i.imdbId));
          return [...(prev ?? []), ...page.filter((i) => !seen.has(i.imdbId))];
        });
        skipRef.current += page.length;
        if (page.length === 0) setDone(true);
        setLoadingMore(false);
      },
      () => {
        setDone(true);
        setLoadingMore(false);
      },
    );
  }, [kind, genre, loadingMore, done, items]);

  // Auto-load the next page as the sentinel nears the viewport.
  useEffect(() => {
    const el = sentinelRef.current;
    if (!el) return;
    const obs = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) loadMore();
      },
      { root: scrollRef.current, rootMargin: "400px" },
    );
    obs.observe(el);
    return () => obs.disconnect();
  }, [loadMore]);

  if (items !== null && items.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 text-center">
        <p className="text-sm font-medium text-zinc-400">No {emptyNoun} found</p>
        <p className="max-w-sm text-xs text-zinc-600">
          {genre
            ? `No ${emptyNoun} in ${genre} right now.`
            : `The canonical catalog is unavailable right now.`}
        </p>
      </div>
    );
  }

  const cells: (CanonicalItem | undefined)[] =
    items ?? Array.from({ length: SKELETON });

  return (
    <div
      ref={scrollRef}
      data-testid="canonical-grid"
      className="h-full overflow-y-auto px-4 pb-10"
    >
      <div className="grid grid-cols-[repeat(auto-fill,minmax(132px,1fr))] gap-4 pt-4 sm:grid-cols-[repeat(auto-fill,minmax(160px,1fr))]">
        {cells.map((it, i) =>
          it ? (
            <div
              key={it.imdbId}
              className="prosc-enter"
              style={{ animationDelay: `${Math.min(i, 16) * 25}ms` }}
            >
              <CanonicalCard
                item={it}
                onActivate={onActivate}
                morphActive={morphId === it.imdbId}
              />
            </div>
          ) : (
            <div
              key={`sk-${i}`}
              data-testid="canonical-skeleton"
              className="aspect-[2/3] animate-pulse rounded-lg bg-zinc-800"
            />
          ),
        )}
      </div>
      <div ref={sentinelRef} className="h-10" />
      {loadingMore && (
        <p className="py-4 text-center text-xs text-zinc-600">Loading…</p>
      )}
    </div>
  );
}
