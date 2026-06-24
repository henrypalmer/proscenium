import { useCallback, useEffect, useRef, useState } from "react";
import { flushSync } from "react-dom";
import { useLocation, useNavigate } from "react-router-dom";
import ContextMenu from "../components/common/ContextMenu";
import AddToListMenu from "../components/lists/AddToListMenu";
import CategoryPanel from "../components/layout/CategoryPanel";
import GenreRows from "../components/vod/GenreRows";
import SeriesCard from "../components/vod/SeriesCard";
import SeriesDetail from "../components/vod/SeriesDetail";
import SeriesGrid from "../components/vod/SeriesGrid";
import * as api from "../lib/tauri";
import { startViewTransition } from "../lib/viewTransition";
import { useCatalogStore } from "../store/catalogStore";
import type { Category, Series } from "../types";

export default function TVShows() {
  const activeProvider = useCatalogStore((s) => s.activeProvider);
  const refreshTick = useCatalogStore((s) => s.refreshTick);

  const location = useLocation();
  const navigate = useNavigate();
  // Home/Search navigate here with a series to open immediately. Initialize the
  // detail from that state so it is present on the first *synchronous* render,
  // which is what lets the poster morph across the route change (Milestone 17).
  const navSeries =
    (location.state as { openSeries?: Series } | null)?.openSeries ?? null;

  // `null` = categories not loaded yet (render nothing — avoids a grey
  // skeleton-grid flash before GenreRows takes over); `[]` = loaded-but-empty.
  const [categories, setCategories] = useState<Category[] | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [detail, setDetail] = useState<Series | null>(navSeries);
  /** Card whose poster morphs in/out of the detail view (View Transitions). */
  const [selectedId, setSelectedId] = useState<string | null>(null);
  /** True when the open detail was reached by navigation (Home/Search) rather
   * than a click within this section's grid — closing it then goes back. */
  const [detailFromNav, setDetailFromNav] = useState(navSeries !== null);
  const [menu, setMenu] = useState<{ series: Series; x: number; y: number } | null>(
    null,
  );
  const [addTo, setAddTo] = useState<{ id: string; x: number; y: number } | null>(null);

  const providerId = activeProvider?.id ?? null;

  // Skip the detail reset on the very first run so a nav-provided detail
  // survives mount; later provider/refresh changes still close any open detail.
  const firstCatRun = useRef(true);
  useEffect(() => {
    if (firstCatRun.current) {
      firstCatRun.current = false;
    } else {
      setDetail(null);
      setDetailFromNav(false);
    }
    if (!providerId) {
      setCategories([]);
      return;
    }
    let cancelled = false;
    void api.getSeriesCategories(providerId).then(
      (cats) => {
        if (cancelled) return;
        setCategories(cats);
        // Drop a selection that disappeared with the latest refresh.
        setSelected((current) =>
          current && !cats.some((c) => c.id === current) ? null : current,
        );
      },
      () => {
        if (!cancelled) setCategories([]);
      },
    );
    return () => {
      cancelled = true;
    };
  }, [providerId, refreshTick]);

  // A search result navigated here asking for a detail view (Milestone 6).
  // Declared after the categories effect: that one resets the detail on
  // mount and must not clobber the requested view.
  useEffect(() => {
    const state = location.state as { openSeries?: Series } | null;
    if (state?.openSeries) {
      setDetail(state.openSeries);
      setDetailFromNav(true);
      // Clear the state so back/refresh doesn't reopen the detail.
      navigate(location.pathname, { replace: true, state: null });
    }
  }, [location.state, location.pathname, navigate]);

  // Per-genre strip fetcher for the "All" overview (memoized so a row only
  // refetches when the provider changes, not on every parent render).
  const fetchSeriesPage = useCallback(
    (catId: string): Promise<Series[]> =>
      providerId
        ? api.getSeries(providerId, catId, 1, 30).then((r) => r.items)
        : Promise.resolve([]),
    [providerId],
  );

  if (!activeProvider) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 text-center">
        <p className="text-sm font-medium text-zinc-400">No provider selected</p>
        <p className="max-w-xs text-xs text-zinc-600">
          Add or select a provider in Settings to browse series.
        </p>
      </div>
    );
  }

  // The clicked poster morphs into the detail's poster via View Transitions:
  // the grid card is flushed to carry the shared name *before* the snapshot,
  // then the detail mount is the transitioned update.
  const openDetail = (series: Series) => {
    setDetailFromNav(false);
    flushSync(() => setSelectedId(series.id));
    startViewTransition(() => setDetail(series));
  };
  // Closing returns to the previous page when we arrived via navigation
  // (e.g. Home or Search), otherwise it morphs back into the grid card.
  const closeDetail = () => {
    if (detailFromNav) navigate(-1);
    else startViewTransition(() => setDetail(null));
  };

  // The grid card carries the shared-element name only while the detail is
  // closed, so the name is never on two elements at once during a transition.
  const morphId = detail ? null : selectedId;

  return (
    <div className="relative flex h-full">
      <CategoryPanel
        title="Genres"
        allLabel="All Series"
        categories={categories ?? []}
        selectedId={selected}
        onSelect={setSelected}
      />
      <div className="min-w-0 flex-1">
        {/* While categories load, render nothing (no grey skeleton flash). Then:
            "All Series" → per-genre row stack (M19); a selected genre → the full
            virtualized grid; no genres → the grid's all-series fallback. */}
        {categories !== null &&
          (selected === null && categories.length > 0 ? (
            <GenreRows<Series>
              categories={categories}
              resetKey={`${activeProvider.id}:${refreshTick}`}
              fetchPage={fetchSeriesPage}
              getKey={(s) => s.id}
              onSelectGenre={setSelected}
              renderCard={(series) => (
                <SeriesCard
                  series={series}
                  onActivate={openDetail}
                  onContextMenu={(s, x, y) => setMenu({ series: s, x, y })}
                  morphActive={morphId === series.id}
                />
              )}
            />
          ) : (
            <SeriesGrid
              providerId={activeProvider.id}
              categoryId={selected}
              version={refreshTick}
              onActivate={openDetail}
              onContextMenu={(series, x, y) => setMenu({ series, x, y })}
              morphId={morphId}
            />
          ))}
      </div>
      {detail && (
        <SeriesDetail
          providerId={activeProvider.id}
          series={detail}
          onClose={closeDetail}
        />
      )}
      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
          items={[
            { label: "Open", onSelect: () => openDetail(menu.series) },
            {
              label: "Add to list…",
              onSelect: () =>
                setAddTo({ id: menu.series.id, x: menu.x, y: menu.y }),
            },
          ]}
        />
      )}
      {addTo && (
        <AddToListMenu
          providerId={activeProvider.id}
          contentType="series"
          contentId={addTo.id}
          x={addTo.x}
          y={addTo.y}
          onClose={() => setAddTo(null)}
        />
      )}
    </div>
  );
}
