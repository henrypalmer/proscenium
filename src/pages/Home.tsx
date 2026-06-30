import { useEffect, useState } from "react";
import { flushSync } from "react-dom";
import { useNavigate } from "react-router-dom";
import ContextMenu from "../components/common/ContextMenu";
import ContinueWatchingSeriesDialog from "../components/home/ContinueWatchingSeriesDialog";
import KeepWatchingCard from "../components/home/KeepWatchingCard";
import MediaRow from "../components/home/MediaRow";
import MyListsRow from "../components/home/MyListsRow";
import AddToListMenu from "../components/lists/AddToListMenu";
import MovieCard from "../components/vod/MovieCard";
import MovieDetail from "../components/vod/MovieDetail";
import SeriesCard from "../components/vod/SeriesCard";
import SeriesDetail from "../components/vod/SeriesDetail";
import * as api from "../lib/tauri";
import { episodeLabel } from "../lib/utils";
import { startViewTransition } from "../lib/viewTransition";
import { useCatalogStore } from "../store/catalogStore";
import { useListsStore } from "../store/listsStore";
import { usePlayerStore } from "../store/playerStore";
import { useProgressStore } from "../store/progressStore";
import type {
  Category,
  ContinueWatchingItem,
  ListContentType,
  Movie,
  Series,
} from "../types";

/** Cards per Popular row. */
const ROW_SIZE = 30;

/** The provider's "Popular" category (spec §5.10): a case-insensitive
 * whole-word match against the category names. */
function findPopular(categories: Category[]): Category | undefined {
  return categories.find((c) => /\bpopular\b/i.test(c.name));
}

/** Stable key for a Keep Watching item (movie vs. episode). */
function cwKey(item: ContinueWatchingItem): string {
  return item.kind === "movie" ? `movie-${item.movie.id}` : `ep-${item.episode.id}`;
}

/** The watch-progress (type, id) addressing a Keep Watching item. */
function progressRef(item: ContinueWatchingItem) {
  return item.kind === "movie"
    ? { contentType: "movie" as const, contentId: item.movie.id }
    : { contentType: "episode" as const, contentId: item.episode.id };
}

/** The provider a Keep Watching item plays from (Milestone 39). */
function providerOf(item: ContinueWatchingItem): string {
  return item.kind === "movie" ? item.movie.providerId : item.episode.providerId;
}

interface MenuState {
  movie: Movie;
  x: number;
  y: number;
}

/** A Keep Watching episode whose parent series is known — the one case that
 * opens the series choice popup (spec §5.10). */
type SeriesChoice = Extract<ContinueWatchingItem, { kind: "episode" }> & {
  series: Series;
};

export default function Home() {
  const providerIds = useCatalogStore((s) => s.providerIds);
  const refreshTick = useCatalogStore((s) => s.refreshTick);
  const notify = useCatalogStore((s) => s.notify);
  const navigate = useNavigate();

  const hasProviders = providerIds.length > 0;
  const scopeKey = providerIds.join(",");

  const [popularMovies, setPopularMovies] = useState<Movie[]>([]);
  const [popularSeries, setPopularSeries] = useState<Series[]>([]);
  /** The card whose poster morphs in/out of the detail overlay. Kept set after
   * close so the reverse morph lands back on the same card (View Transitions). */
  const [morph, setMorph] = useState<{ type: "movie" | "series"; id: string } | null>(
    null,
  );
  /** Detail shown as an in-place overlay (not a route change) so Home stays
   * mounted — scroll is preserved and the poster morphs back on close. */
  const [detail, setDetail] = useState<
    { type: "movie"; item: Movie } | { type: "series"; item: Series } | null
  >(null);
  const [keepWatching, setKeepWatching] = useState<ContinueWatchingItem[]>([]);
  const [menu, setMenu] = useState<MenuState | null>(null);
  const [seriesChoice, setSeriesChoice] = useState<SeriesChoice | null>(null);
  const [kwMenu, setKwMenu] = useState<{
    item: ContinueWatchingItem;
    x: number;
    y: number;
  } | null>(null);
  const [seriesMenu, setSeriesMenu] = useState<{
    series: Series;
    x: number;
    y: number;
  } | null>(null);
  const [addTo, setAddTo] = useState<{
    contentType: ListContentType;
    id: string;
    providerId: string;
    x: number;
    y: number;
  } | null>(null);

  useEffect(() => {
    if (!hasProviders) {
      setPopularMovies([]);
      setPopularSeries([]);
      setKeepWatching([]);
      return;
    }
    let cancelled = false;

    // Watch-progress markers for the Popular Movies cards (spec §5.9), merged.
    void useProgressStore.getState().loadSection(providerIds, "movie");
    // Custom lists for the "My Lists" row (spec §5.10/§5.11); global since M39.
    void useListsStore.getState().load();

    void (async () => {
      try {
        const cats = await api.getVodCategories(providerIds);
        const cat = findPopular(cats);
        const movies = cat
          ? (await api.getMovies(providerIds, cat.id, 1, ROW_SIZE)).items
          : [];
        if (!cancelled) setPopularMovies(movies);
      } catch {
        if (!cancelled) setPopularMovies([]);
      }
    })();

    void (async () => {
      try {
        const cats = await api.getSeriesCategories(providerIds);
        const cat = findPopular(cats);
        const series = cat
          ? (await api.getSeries(providerIds, cat.id, 1, ROW_SIZE)).items
          : [];
        if (!cancelled) setPopularSeries(series);
      } catch {
        if (!cancelled) setPopularSeries([]);
      }
    })();

    void api.getContinueWatching(providerIds, 20).then(
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scopeKey, refreshTick]);

  if (!hasProviders) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 text-center">
        <p className="text-sm font-medium text-zinc-400">No provider enabled</p>
        <p className="max-w-xs text-xs text-zinc-600">
          Add or enable a provider in Settings to see your Home screen.
        </p>
      </div>
    );
  }

  // Open the detail as an in-place overlay with the poster morph (Milestone 16
  // pattern). Because Home never unmounts, closing morphs the poster straight
  // back into the same card with scroll preserved.
  const openMovie = (movie: Movie) => {
    flushSync(() => setMorph({ type: "movie", id: movie.id }));
    startViewTransition(() => setDetail({ type: "movie", item: movie }));
  };
  const openSeries = (series: Series) => {
    flushSync(() => setMorph({ type: "series", id: series.id }));
    startViewTransition(() => setDetail({ type: "series", item: series }));
  };
  const closeDetail = () => startViewTransition(() => setDetail(null));
  const playMovie = (movie: Movie) =>
    void usePlayerStore.getState().openContent({
      providerId: movie.providerId,
      contentType: "movie",
      contentId: movie.id,
      title: movie.name,
    });
  const playMovieExternal = async (movie: Movie) => {
    try {
      const url = await api.resolveStreamUrl(movie.providerId, "movie", movie.id);
      await api.openInExternalPlayer(url);
    } catch (e) {
      notify(String(e), "error");
    }
  };

  // Resume a Keep Watching item via the standard §5.9 flow.
  const resumeItem = (item: ContinueWatchingItem) => {
    if (item.kind === "movie") {
      void usePlayerStore.getState().openContent({
        providerId: item.movie.providerId,
        contentType: "movie",
        contentId: item.movie.id,
        title: item.movie.name,
      });
    } else {
      const { episode, series } = item;
      void usePlayerStore.getState().openContent({
        providerId: episode.providerId,
        contentType: "episode",
        contentId: episode.id,
        title: episodeLabel(
          series?.name ?? "",
          episode.season,
          episode.episode,
          episode.title,
        ),
      });
    }
  };

  // Clicking a Keep Watching card: movies (and catalog-orphaned episodes with no
  // known series) resume directly; series episodes open the choice popup.
  const onKeepWatchingActivate = (item: ContinueWatchingItem) => {
    if (item.kind === "episode" && item.series) {
      setSeriesChoice({ ...item, series: item.series });
    } else {
      resumeItem(item);
    }
  };

  // Drop a card from the row in place (an empty row is omitted by MediaRow).
  const removeCard = (item: ContinueWatchingItem) =>
    setKeepWatching((prev) => prev.filter((it) => cwKey(it) !== cwKey(item)));

  // Keep Watching → "Mark as watched" (§5.10).
  const markWatched = (item: ContinueWatchingItem) => {
    const { contentType, contentId } = progressRef(item);
    const providerId = providerOf(item);
    const duration = item.progress.durationSeconds;
    void api.markWatched(providerId, contentType, contentId, duration);
    useProgressStore.getState().setLocal(providerId, contentType, contentId, {
      positionSeconds: duration ?? item.progress.positionSeconds,
      durationSeconds: duration,
      completed: true,
      updatedAt: Math.floor(Date.now() / 1000),
    });
    removeCard(item);
  };

  // Keep Watching → "Remove from list" (§5.10): clear progress entirely.
  const removeFromList = (item: ContinueWatchingItem) => {
    const { contentType, contentId } = progressRef(item);
    const providerId = providerOf(item);
    void api.clearWatchProgress(providerId, contentType, contentId);
    useProgressStore.getState().setLocal(providerId, contentType, contentId, null);
    removeCard(item);
  };

  const empty =
    popularMovies.length === 0 &&
    popularSeries.length === 0 &&
    keepWatching.length === 0;

  return (
    <div className="relative h-full">
      <div className="h-full overflow-y-auto px-4 pb-10">
        <div className="space-y-8">
          {/* Keep Watching leads when there is in-progress content (spec §5.10);
              an empty row is omitted by MediaRow, so Popular Movies becomes top. */}
          <MediaRow
            title="Keep Watching"
            testId="home-keep-watching"
            items={keepWatching}
            getKey={cwKey}
            renderItem={(item) => (
              <KeepWatchingCard
                item={item}
                onActivate={onKeepWatchingActivate}
                onMenu={(it, x, y) => setKwMenu({ item: it, x, y })}
              />
            )}
          />
          <MyListsRow onOpenList={(id) => navigate(`/list/${id}`)} />
          <MediaRow
            title="Popular Movies"
            testId="home-popular-movies"
            items={popularMovies}
            getKey={(m) => `${m.providerId}:${m.id}`}
            renderItem={(movie) => (
              <MovieCard
                movie={movie}
                onActivate={openMovie}
                onContextMenu={(m, x, y) => setMenu({ movie: m, x, y })}
                morphActive={
                  detail === null && morph?.type === "movie" && morph.id === movie.id
                }
              />
            )}
          />
          <MediaRow
            title="Popular Series"
            testId="home-popular-series"
            items={popularSeries}
            getKey={(s) => `${s.providerId}:${s.id}`}
            renderItem={(series) => (
              <SeriesCard
                series={series}
                onActivate={openSeries}
                onContextMenu={(s, x, y) => setSeriesMenu({ series: s, x, y })}
                morphActive={
                  detail === null && morph?.type === "series" && morph.id === series.id
                }
              />
            )}
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
      </div>

      {detail &&
        (detail.type === "movie" ? (
          <MovieDetail
            providerId={detail.item.providerId}
            movie={detail.item}
            onClose={closeDetail}
          />
        ) : (
          <SeriesDetail
            providerId={detail.item.providerId}
            series={detail.item}
            onClose={closeDetail}
          />
        ))}

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
            {
              label: "Add to list…",
              onSelect: () =>
                setAddTo({
                  contentType: "movie",
                  id: menu.movie.id,
                  providerId: menu.movie.providerId,
                  x: menu.x,
                  y: menu.y,
                }),
            },
          ]}
        />
      )}

      {seriesMenu && (
        <ContextMenu
          x={seriesMenu.x}
          y={seriesMenu.y}
          onClose={() => setSeriesMenu(null)}
          items={[
            { label: "Open", onSelect: () => openSeries(seriesMenu.series) },
            {
              label: "Add to list…",
              onSelect: () =>
                setAddTo({
                  contentType: "series",
                  id: seriesMenu.series.id,
                  providerId: seriesMenu.series.providerId,
                  x: seriesMenu.x,
                  y: seriesMenu.y,
                }),
            },
          ]}
        />
      )}

      {addTo && (
        <AddToListMenu
          providerId={addTo.providerId}
          contentType={addTo.contentType}
          contentId={addTo.id}
          x={addTo.x}
          y={addTo.y}
          onClose={() => setAddTo(null)}
        />
      )}

      {kwMenu && (
        <ContextMenu
          x={kwMenu.x}
          y={kwMenu.y}
          onClose={() => setKwMenu(null)}
          items={[
            { label: "Mark as watched", onSelect: () => markWatched(kwMenu.item) },
            { label: "Remove from list", onSelect: () => removeFromList(kwMenu.item) },
          ]}
        />
      )}

      {seriesChoice && (
        <ContinueWatchingSeriesDialog
          series={seriesChoice.series}
          episode={seriesChoice.episode}
          resumeSeconds={seriesChoice.progress.positionSeconds}
          onResume={() => {
            resumeItem(seriesChoice);
            setSeriesChoice(null);
          }}
          onGoToSeries={() => {
            openSeries(seriesChoice.series);
            setSeriesChoice(null);
          }}
          onClose={() => setSeriesChoice(null)}
        />
      )}
    </div>
  );
}
