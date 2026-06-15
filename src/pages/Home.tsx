import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import ContextMenu from "../components/common/ContextMenu";
import KeepWatchingCard from "../components/home/KeepWatchingCard";
import MediaRow from "../components/home/MediaRow";
import MovieCard from "../components/vod/MovieCard";
import SeriesCard from "../components/vod/SeriesCard";
import * as api from "../lib/tauri";
import { useCatalogStore } from "../store/catalogStore";
import { usePlayerStore } from "../store/playerStore";
import { useProgressStore } from "../store/progressStore";
import type { Category, ContinueWatchingItem, Movie, Series } from "../types";

/** Cards per Popular row. */
const ROW_SIZE = 30;

/** The provider's "Popular" category (spec §5.10): a case-insensitive
 * whole-word match against the category names. */
function findPopular(categories: Category[]): Category | undefined {
  return categories.find((c) => /\bpopular\b/i.test(c.name));
}

interface MenuState {
  movie: Movie;
  x: number;
  y: number;
}

export default function Home() {
  const activeProvider = useCatalogStore((s) => s.activeProvider);
  const refreshTick = useCatalogStore((s) => s.refreshTick);
  const notify = useCatalogStore((s) => s.notify);
  const navigate = useNavigate();

  const providerId = activeProvider?.id ?? null;

  const [popularMovies, setPopularMovies] = useState<Movie[]>([]);
  const [popularSeries, setPopularSeries] = useState<Series[]>([]);
  const [keepWatching, setKeepWatching] = useState<ContinueWatchingItem[]>([]);
  const [menu, setMenu] = useState<MenuState | null>(null);

  useEffect(() => {
    if (!providerId) {
      setPopularMovies([]);
      setPopularSeries([]);
      setKeepWatching([]);
      return;
    }
    let cancelled = false;

    // Watch-progress markers for the Popular Movies cards (spec §5.9).
    void useProgressStore.getState().loadSection(providerId, "movie");

    void (async () => {
      try {
        const cats = await api.getVodCategories(providerId);
        const cat = findPopular(cats);
        const movies = cat
          ? (await api.getMovies(providerId, cat.id, 1, ROW_SIZE)).items
          : [];
        if (!cancelled) setPopularMovies(movies);
      } catch {
        if (!cancelled) setPopularMovies([]);
      }
    })();

    void (async () => {
      try {
        const cats = await api.getSeriesCategories(providerId);
        const cat = findPopular(cats);
        const series = cat
          ? (await api.getSeries(providerId, cat.id, 1, ROW_SIZE)).items
          : [];
        if (!cancelled) setPopularSeries(series);
      } catch {
        if (!cancelled) setPopularSeries([]);
      }
    })();

    void api.getContinueWatching(providerId, 20).then(
      (items) => {
        if (!cancelled) setKeepWatching(items);
      },
      () => {
        if (!cancelled) setKeepWatching([]);
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
          Add or select a provider in Settings to see your Home screen.
        </p>
      </div>
    );
  }

  const pid = activeProvider.id;

  const openMovie = (movie: Movie) =>
    navigate("/movies", { state: { openMovie: movie } });
  const openSeries = (series: Series) =>
    navigate("/shows", { state: { openSeries: series } });
  const playMovie = (movie: Movie) =>
    void usePlayerStore.getState().openContent({
      providerId: pid,
      contentType: "movie",
      contentId: movie.id,
      title: movie.name,
    });
  const playMovieExternal = async (movie: Movie) => {
    try {
      const url = await api.resolveStreamUrl(pid, "movie", movie.id);
      await api.openInExternalPlayer(url);
    } catch (e) {
      notify(String(e), "error");
    }
  };

  // Keep Watching click resumes via the standard §5.9 flow (resume prompt when
  // there is meaningful progress, which there always is here).
  const resume = (item: ContinueWatchingItem) => {
    if (item.kind === "movie") {
      void usePlayerStore.getState().openContent({
        providerId: pid,
        contentType: "movie",
        contentId: item.movie.id,
        title: item.movie.name,
      });
    } else {
      const { episode, series } = item;
      const title = series
        ? `${series.name} · S${episode.season}E${episode.episode}`
        : episode.title;
      void usePlayerStore.getState().openContent({
        providerId: pid,
        contentType: "episode",
        contentId: episode.id,
        title,
      });
    }
  };

  const empty =
    popularMovies.length === 0 &&
    popularSeries.length === 0 &&
    keepWatching.length === 0;

  return (
    <div className="px-6 pb-10">
      <div className="mx-auto max-w-6xl space-y-8">
        <MediaRow
          title="Popular Movies"
          testId="home-popular-movies"
          items={popularMovies}
          getKey={(m) => m.id}
          renderItem={(movie) => (
            <MovieCard
              movie={movie}
              providerId={pid}
              onActivate={openMovie}
              onContextMenu={(m, x, y) => setMenu({ movie: m, x, y })}
            />
          )}
        />
        <MediaRow
          title="Popular Series"
          testId="home-popular-series"
          items={popularSeries}
          getKey={(s) => s.id}
          renderItem={(series) => (
            <SeriesCard series={series} onActivate={openSeries} />
          )}
        />
        <MediaRow
          title="Keep Watching"
          testId="home-keep-watching"
          items={keepWatching}
          getKey={(item) =>
            item.kind === "movie" ? `movie-${item.movie.id}` : `ep-${item.episode.id}`
          }
          renderItem={(item) => <KeepWatchingCard item={item} onActivate={resume} />}
        />

        {empty && (
          <div className="flex h-72 flex-col items-center justify-center gap-2 text-center">
            <p className="text-sm font-medium text-zinc-400">Nothing here yet</p>
            <p className="max-w-sm text-xs text-zinc-600">
              Start watching something, or browse Movies and Series — your
              Popular picks and Keep Watching will show up here.
            </p>
          </div>
        )}
      </div>

      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
          items={[
            { label: "Play", onSelect: () => playMovie(menu.movie) },
            {
              label: "Open in External Player",
              onSelect: () => void playMovieExternal(menu.movie),
            },
          ]}
        />
      )}
    </div>
  );
}
