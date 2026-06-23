import { useVirtualizer } from "@tanstack/react-virtual";
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { useCatalogStore } from "../../store/catalogStore";
import Placeholder from "../common/Placeholder";

/**
 * Lazy 2:3 poster image with `Placeholder` fallback for missing or failed
 * art (spec §10 image loading). Shared by MovieCard and SeriesCard.
 */
export function Poster({
  url,
  title,
  overlay,
  vtName,
}: {
  url: string | null;
  title: string;
  /** Optional overlay (e.g. watch-progress bar/checkmark) drawn over the art. */
  overlay?: ReactNode;
  /** When set, names this poster as a View Transitions shared element so it
   * morphs between its grid cell and the detail view (see viewTransition.ts). */
  vtName?: string;
}) {
  const [state, setState] = useState<"loading" | "loaded" | "error">(
    url ? "loading" : "error",
  );
  return (
    <div
      className="relative aspect-[2/3] w-full overflow-hidden rounded-lg bg-zinc-800"
      style={vtName ? { viewTransitionName: vtName } : undefined}
    >
      <Placeholder label={title} />
      {url && state !== "error" && (
        <img
          src={url}
          alt=""
          loading="lazy"
          decoding="async"
          onLoad={() => setState("loaded")}
          onError={() => setState("error")}
          className={`absolute inset-0 h-full w-full object-cover transition-opacity duration-150 ${
            state === "loaded" ? "opacity-100" : "opacity-0"
          }`}
        />
      )}
      {overlay}
    </div>
  );
}

/** Gap between cells and the grid's outer padding (px). */
const GAP = 16;
/** Target cell width; the column count adapts to the container. */
const TARGET_CELL_WIDTH = 176;
/** Caption block under the 2:3 poster (title + year lines). */
const CAPTION_HEIGHT = 52;
const INITIAL_SKELETON_ROWS = 6;

interface PosterGridProps<T> {
  total: number | null;
  getItem: (index: number) => T | undefined;
  ensureRange: (startIndex: number, endIndex: number) => void;
  renderCard: (item: T) => ReactNode;
  /** Scrolls back to the top when this changes (category/catalog switch). */
  resetKey: string;
  /** Noun for the empty state, e.g. "movies" / "shows". */
  emptyNoun: string;
  /** True when a genre filter is active (changes the empty-state copy). */
  filtered: boolean;
}

/**
 * Shared virtualized poster grid for Movies and TV Shows (spec §10): rows are
 * windowed via @tanstack/react-virtual, the column count adapts to the
 * container width, and unfetched cells render poster-shaped skeletons with
 * no layout shift on resolution.
 */
export default function PosterGrid<T>({
  total,
  getItem,
  ensureRange,
  renderCard,
  resetKey,
  emptyNoun,
  filtered,
}: PosterGridProps<T>) {
  const parentRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  const refreshing = useCatalogStore((s) => s.refreshing);
  const refresh = useCatalogStore((s) => s.refresh);

  // First-paint entrance (spec §9 / Milestone 17): the initial top rows of a
  // freshly-loaded dataset fade in with a capped stagger. Gated so it fires only
  // on the first paint of each dataset (reset on category/catalog switch) and
  // only for the top few rows — never for cells recycled during scroll, which
  // always have indices past the threshold.
  const [firstPaint, setFirstPaint] = useState(true);
  useEffect(() => {
    setFirstPaint(true);
    const t = setTimeout(() => setFirstPaint(false), 700);
    return () => clearTimeout(t);
  }, [resetKey]);

  useLayoutEffect(() => {
    const el = parentRef.current;
    if (!el) return;
    const observer = new ResizeObserver(() => setWidth(el.clientWidth));
    observer.observe(el);
    setWidth(el.clientWidth);
    return () => observer.disconnect();
  }, []);

  const columns = Math.max(2, Math.floor((width - GAP) / TARGET_CELL_WIDTH));
  const cellWidth = width > 0 ? (width - GAP * (columns + 1)) / columns : TARGET_CELL_WIDTH;
  // 2:3 poster + caption.
  const rowHeight = cellWidth * 1.5 + CAPTION_HEIGHT + GAP;

  const itemCount = total ?? columns * INITIAL_SKELETON_ROWS;
  const rowCount = Math.ceil(itemCount / columns);

  const virtualizer = useVirtualizer({
    count: rowCount,
    getScrollElement: () => parentRef.current,
    estimateSize: () => rowHeight,
    overscan: 4,
  });
  const virtualRows = virtualizer.getVirtualItems();

  // Re-measure all rows when the geometry changes (resize / column change).
  useEffect(() => {
    virtualizer.measure();
  }, [virtualizer, rowHeight, columns]);

  // Fetch the pages backing the currently visible cell range.
  useEffect(() => {
    if (total === null || virtualRows.length === 0) return;
    ensureRange(
      virtualRows[0].index * columns,
      virtualRows[virtualRows.length - 1].index * columns + columns - 1,
    );
  }, [total, virtualRows, columns, ensureRange]);

  useEffect(() => {
    parentRef.current?.scrollTo({ top: 0 });
  }, [resetKey]);

  if (total === 0) {
    // Spec §12: empty catalog → instructional empty state with Refresh.
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
        <p className="text-sm font-medium text-zinc-400">
          No {emptyNoun} here yet
        </p>
        <p className="max-w-sm text-xs text-zinc-600">
          {filtered
            ? `This genre has no ${emptyNoun}.`
            : `Refresh the catalog to pull ${emptyNoun} from your provider.`}
        </p>
        {!filtered && (
          <button
            onClick={() => void refresh()}
            disabled={refreshing}
            className="rounded-md bg-zinc-100 px-4 py-1.5 text-xs font-semibold text-zinc-900 hover:bg-white disabled:opacity-50"
          >
            {refreshing ? "Refreshing…" : "Refresh"}
          </button>
        )}
      </div>
    );
  }

  return (
    <div
      ref={parentRef}
      data-testid="poster-grid"
      className="h-full overflow-y-auto"
    >
      <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
        {width > 0 &&
          virtualRows.map((row) => {
            const startIndex = row.index * columns;
            const cells = Array.from(
              { length: Math.min(columns, itemCount - startIndex) },
              (_, i) => startIndex + i,
            );
            return (
              <div
                key={row.key}
                data-index={row.index}
                style={{
                  position: "absolute",
                  top: 0,
                  left: 0,
                  width: "100%",
                  height: row.size,
                  transform: `translateY(${row.start}px)`,
                  display: "flex",
                  gap: GAP,
                  padding: `0 ${GAP}px`,
                  paddingTop: GAP,
                  boxSizing: "border-box",
                }}
              >
                {cells.map((index) => {
                  const item = total === null ? undefined : getItem(index);
                  // Animate only the initial top rows on first paint; recycled
                  // cells during scroll have indices past this threshold.
                  const entering = firstPaint && index < columns * 3;
                  return (
                    <div
                      key={index}
                      className={entering ? "prosc-enter" : undefined}
                      style={{
                        width: cellWidth,
                        flexShrink: 0,
                        animationDelay: entering
                          ? `${Math.min(index, 16) * 25}ms`
                          : undefined,
                      }}
                    >
                      {item ? (
                        renderCard(item)
                      ) : (
                        <PosterSkeleton posterHeight={cellWidth * 1.5} />
                      )}
                    </div>
                  );
                })}
              </div>
            );
          })}
      </div>
    </div>
  );
}

/** Poster-shaped loading placeholder matching the card cell exactly. */
function PosterSkeleton({ posterHeight }: { posterHeight: number }) {
  return (
    <div data-testid="poster-skeleton">
      <div
        className="w-full animate-pulse rounded-lg bg-zinc-800"
        style={{ height: posterHeight }}
      />
      <div className="mt-2 h-3 w-3/4 animate-pulse rounded bg-zinc-800" />
      <div className="mt-1.5 h-3 w-1/3 animate-pulse rounded bg-zinc-800" />
    </div>
  );
}
