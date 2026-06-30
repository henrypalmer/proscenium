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
  const providerIds = useCatalogStore((s) => s.providerIds);
  const refreshTick = useCatalogStore((s) => s.refreshTick);

  const location = useLocation();
  const navigate = useNavigate();
  const navSeries =
    (location.state as { openSeries?: Series } | null)?.openSeries ?? null;

  const [categories, setCategories] = useState<Category[] | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [detail, setDetail] = useState<Series | null>(navSeries);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detailFromNav, setDetailFromNav] = useState(navSeries !== null);
  const [menu, setMenu] = useState<{ series: Series; x: number; y: number } | null>(
    null,
  );
  const [addTo, setAddTo] = useState<{
    id: string;
    providerId: string;
    x: number;
    y: number;
  } | null>(null);

  const hasProviders = providerIds.length > 0;
  const scopeKey = providerIds.join(",");

  const firstCatRun = useRef(true);
  useEffect(() => {
    if (firstCatRun.current) {
      firstCatRun.current = false;
    } else {
      setDetail(null);
      setDetailFromNav(false);
    }
    if (!hasProviders) {
      setCategories([]);
      return;
    }
    let cancelled = false;
    void api.getSeriesCategories(providerIds).then(
      (cats) => {
        if (cancelled) return;
        setCategories(cats);
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scopeKey, refreshTick]);

  useEffect(() => {
    const state = location.state as { openSeries?: Series } | null;
    if (state?.openSeries) {
      setDetail(state.openSeries);
      setDetailFromNav(true);
      navigate(location.pathname, { replace: true, state: null });
    }
  }, [location.state, location.pathname, navigate]);

  const fetchSeriesPage = useCallback(
    (catId: string): Promise<Series[]> =>
      hasProviders
        ? api.getSeries(providerIds, catId, 1, 30).then((r) => r.items)
        : Promise.resolve([]),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [scopeKey],
  );

  if (!hasProviders) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 text-center">
        <p className="text-sm font-medium text-zinc-400">No provider enabled</p>
        <p className="max-w-xs text-xs text-zinc-600">
          Add or enable a provider in Settings to browse series.
        </p>
      </div>
    );
  }

  const openDetail = (series: Series) => {
    setDetailFromNav(false);
    flushSync(() => setSelectedId(series.id));
    startViewTransition(() => setDetail(series));
  };
  const closeDetail = () => {
    if (detailFromNav) navigate(-1);
    else startViewTransition(() => setDetail(null));
  };

  const morphId = detail ? null : selectedId;

  return (
    <div className="relative flex h-full">
      <CategoryPanel
        title="Genres"
        allLabel="All Series"
        categories={categories ?? []}
        selectedId={selected}
        onSelect={setSelected}
        providerIds={providerIds}
        section="series"
      />
      <div className="min-w-0 flex-1">
        {categories !== null &&
          (selected === null && categories.length > 0 ? (
            <GenreRows<Series>
              categories={categories}
              resetKey={`${scopeKey}:${refreshTick}`}
              fetchPage={fetchSeriesPage}
              getKey={(s) => `${s.providerId}:${s.id}`}
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
              providerIds={providerIds}
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
          providerId={detail.providerId}
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
                setAddTo({
                  id: menu.series.id,
                  providerId: menu.series.providerId,
                  x: menu.x,
                  y: menu.y,
                }),
            },
          ]}
        />
      )}
      {addTo && (
        <AddToListMenu
          providerId={addTo.providerId}
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
