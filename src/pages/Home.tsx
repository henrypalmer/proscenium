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
  const activeProvider = useCatalogStore((s) => s.activeProvider);
  const refreshTick = useCatalogStore((s) => s.refreshTick);
  const notify = useCatalogStore((s) => s.notify);
  const navigate = useNavigate();

  const providerId = activeProvider?.id ?? null;

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
    x: number;
    y: number;
  } | null>(null);

  // Switching the active provider (Milestone 36) must not leave a previous
  // provider's detail/menu overlay open over the new provider's Home.
  useEffect(() => {
    setDetail(null);
    setMorph(null);
    setMenu(null);
    setSeriesMenu(null);
    setSeriesChoice(null);
    setKwMenu(null);
    setAddTo(null);
  }, [providerId]);

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
    // Custom lists for the "My Lists" row (spec §5.10/§5.11).
    void useListsStore.getState().load(providerId);

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

  // Open the detail as an in-place overlay with the poster morph (Milestone 16
  // pattern): flush the clicked card's shared name in *before* the snapshot,
  // then mount the detail as the transitioned update. Because Home never
  // unmounts, closing morphs the poster straight back into the same card with
  // scroll preserved — unlike a route change, which would refetch and replay.
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

  // Resume a Keep Watching item via the standard §5.9 flow (the resume prompt
  // always appears here because every Keep Watching item is in-progress).
  const resumeItem = (item: ContinueWatchingItem) => {
    if (item.kind === "movie") {
      void usePlayerStore.getState().openContent({
        providerId: pid,
        contentType: "movie",
        contentId: item.movie.id,
        title: item.movie.name,
      });
    } else {
      const { episode, series } = item;
      void usePlayerStore.getState().openContent({
        providerId: pid,
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
  // known series) resume directly; series episodes open the choice popup so the
  // user can resume the last episode or jump to the series page (spec §5.10).
  const onKeepWatchingActivate = (item: ContinueWatchingItem) => {
    if (item.kind === "episode" && item.series) {
      setSeriesChoice({ ...item, series: item.series });
    } else {
      resumeItem(item);
    }
  };

  // Drop a card from the row in place (the row closes up; an empty row is
  // omitted by MediaRow).
  const removeCard = (item: ContinueWatchingItem) =>
    setKeepWatching((prev) => prev.filter((it) => cwKey(it) !== cwKey(item)));

  // Keep Watching → "Mark as watched" (§5.10): set the completion flag so the
  // item leaves the row and shows the §5.9 watched checkmark in the catalog.
  const markWatched = (item: ContinueWatchingItem) => {
    const { contentType, contentId } = progressRef(item);
    const duration = item.progress.durationSeconds;
    void api.markWatched(pid, contentType, contentId, duration);
    useProgressStore.getState().setLocal(pid, contentType, contentId, {
      positionSeconds: duration ?? item.progress.positionSeconds,
      durationSeconds: duration,
      completed: true,
      updatedAt: Math.floor(Date.now() / 1000),
    });
    removeCard(item);
  };

  // Keep Watching → "Remove from list" (§5.10): clear progress entirely so the
  // item shows neither a bar nor a checkmark.
  const removeFromList = (item: ContinueWatchingItem) => {
    const { contentType, contentId } = progressRef(item);
    void api.clearWatchProgress(pid, contentType, contentId);
    useProgressStore.getState().setLocal(pid, contentType, contentId, null);
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
            getKey={(m) => m.id}
            renderItem={(movie) => (
              <MovieCard
                movie={movie}
                providerId={pid}
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
            getKey={(s) => s.id}
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

      {/* Detail rendered like Movies/TV Shows: absolute within this relative
          page (z-20) so it sits *below* the floating nav (z-30), keeping the nav
          visible. Home stays mounted, so closing morphs the poster back. */}
      {detail &&
        (detail.type === "movie" ? (
          <MovieDetail providerId={pid} movie={detail.item} onClose={closeDetail} />
        ) : (
          <SeriesDetail providerId={pid} series={detail.item} onClose={closeDetail} />
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
                setAddTo({ contentType: "movie", id: menu.movie.id, x: menu.x, y: menu.y }),
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
                  x: seriesMenu.x,
                  y: seriesMenu.y,
                }),
            },
          ]}
        />
      )}

      {addTo && (
        <AddToListMenu
          providerId={pid}
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
