import { useEffect, useState } from "react";
import { flushSync } from "react-dom";
import CategoryPanel from "../layout/CategoryPanel";
import MovieDetail from "../vod/MovieDetail";
import MovieGrid from "../vod/MovieGrid";
import SeriesDetail from "../vod/SeriesDetail";
import SeriesGrid from "../vod/SeriesGrid";
import * as api from "../../lib/tauri";
import { startViewTransition } from "../../lib/viewTransition";
import { useCatalogStore } from "../../store/catalogStore";
import type { Category, Movie, Series } from "../../types";

/**
 * Provider-centric VOD browse (the pre-M40 catalog), reachable via the "My
 * Providers" tab. Canonical browse is the default, but un-matchable provider VOD
 * (workouts/PPV/concerts) and anything Cinemeta lacks stays browsable here
 * (spec §19 M40 — provider-centric browse remains). Live TV is unaffected.
 */
export default function ProviderBrowse({ kind }: { kind: "movie" | "series" }) {
  const providerIds = useCatalogStore((s) => s.providerIds);
  const refreshTick = useCatalogStore((s) => s.refreshTick);
  const scopeKey = providerIds.join(",");

  const [categories, setCategories] = useState<Category[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [movie, setMovie] = useState<Movie | null>(null);
  const [series, setSeries] = useState<Series | null>(null);
  const [morphId, setMorphId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    const load =
      kind === "movie"
        ? api.getVodCategories(providerIds)
        : api.getSeriesCategories(providerIds);
    void load.then(
      (c) => {
        if (!cancelled) setCategories(c);
      },
      () => {
        if (!cancelled) setCategories([]);
      },
    );
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [kind, scopeKey, refreshTick]);

  if (providerIds.length === 0) {
    return (
      <div className="flex h-full items-center justify-center text-sm text-zinc-500">
        No provider enabled.
      </div>
    );
  }

  const openMovie = (m: Movie) => {
    flushSync(() => setMorphId(m.id));
    startViewTransition(() => setMovie(m));
  };
  const openSeries = (s: Series) => {
    flushSync(() => setMorphId(s.id));
    startViewTransition(() => setSeries(s));
  };

  return (
    <div className="relative flex h-full">
      <CategoryPanel
        title="Genres"
        allLabel={kind === "movie" ? "All Movies" : "All Series"}
        categories={categories}
        selectedId={selected}
        onSelect={setSelected}
        providerIds={providerIds}
        section={kind}
      />
      <div className="min-w-0 flex-1">
        {kind === "movie" ? (
          <MovieGrid
            providerIds={providerIds}
            categoryId={selected}
            version={refreshTick}
            onActivate={openMovie}
            onContextMenu={() => {}}
            morphId={movie ? null : morphId}
          />
        ) : (
          <SeriesGrid
            providerIds={providerIds}
            categoryId={selected}
            version={refreshTick}
            onActivate={openSeries}
            onContextMenu={() => {}}
            morphId={series ? null : morphId}
          />
        )}
      </div>
      {movie && (
        <MovieDetail
          providerId={movie.providerId}
          movie={movie}
          onClose={() => startViewTransition(() => setMovie(null))}
        />
      )}
      {series && (
        <SeriesDetail
          providerId={series.providerId}
          series={series}
          onClose={() => startViewTransition(() => setSeries(null))}
        />
      )}
    </div>
  );
}
