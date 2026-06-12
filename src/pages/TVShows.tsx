import { useEffect, useState } from "react";
import CategoryPanel from "../components/layout/CategoryPanel";
import SeriesDetail from "../components/vod/SeriesDetail";
import SeriesGrid from "../components/vod/SeriesGrid";
import * as api from "../lib/tauri";
import { useCatalogStore } from "../store/catalogStore";
import type { Category, Series } from "../types";

export default function TVShows() {
  const activeProvider = useCatalogStore((s) => s.activeProvider);
  const refreshTick = useCatalogStore((s) => s.refreshTick);

  const [categories, setCategories] = useState<Category[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [detail, setDetail] = useState<Series | null>(null);

  const providerId = activeProvider?.id ?? null;

  useEffect(() => {
    setDetail(null);
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

  if (!activeProvider) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 text-center">
        <p className="text-sm font-medium text-zinc-400">No provider selected</p>
        <p className="max-w-xs text-xs text-zinc-600">
          Add or select a provider in Settings to browse TV shows.
        </p>
      </div>
    );
  }

  return (
    <div className="relative flex h-full">
      <CategoryPanel
        title="Genres"
        allLabel="All Shows"
        categories={categories}
        selectedId={selected}
        onSelect={setSelected}
      />
      <div className="min-w-0 flex-1">
        <SeriesGrid
          providerId={activeProvider.id}
          categoryId={selected}
          version={refreshTick}
          onActivate={setDetail}
        />
      </div>
      {detail && (
        <SeriesDetail
          providerId={activeProvider.id}
          series={detail}
          onClose={() => setDetail(null)}
        />
      )}
    </div>
  );
}
