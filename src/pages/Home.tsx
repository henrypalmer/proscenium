import { useEffect, useState } from "react";
import { flushSync } from "react-dom";
import { useNavigate } from "react-router-dom";
import ContextMenu from "../components/common/ContextMenu";
import CanonicalCard from "../components/canonical/CanonicalCard";
import CanonicalDetail from "../components/canonical/CanonicalDetail";
import ContinueWatchingSeriesDialog from "../components/home/ContinueWatchingSeriesDialog";
import KeepWatchingCard from "../components/home/KeepWatchingCard";
import MediaRow from "../components/home/MediaRow";
import MyListsRow from "../components/home/MyListsRow";
import SeriesDetail from "../components/vod/SeriesDetail";
import * as api from "../lib/tauri";
import { episodeLabel } from "../lib/utils";
import { startViewTransition } from "../lib/viewTransition";
import { useCatalogStore } from "../store/catalogStore";
import { useListsStore } from "../store/listsStore";
import { usePlayerStore } from "../store/playerStore";
import { useProgressStore } from "../store/progressStore";
import type { CanonicalItem, ContinueWatchingItem, Series } from "../types";

/** Cards per Popular row. */
const ROW_SIZE = 24;

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

/** A Keep Watching episode whose parent series is known — the one case that
 * opens the series choice popup (spec §5.10). */
type SeriesChoice = Extract<ContinueWatchingItem, { kind: "episode" }> & {
  series: Series;
};

export default function Home() {
  const providerIds = useCatalogStore((s) => s.providerIds);
  const refreshTick = useCatalogStore((s) => s.refreshTick);
  const navigate = useNavigate();

  const hasProviders = providerIds.length > 0;
  const scopeKey = providerIds.join(",");

  // Popular rows are the Cinemeta-backed canonical catalog (Milestone 40).
  const [popularMovies, setPopularMovies] = useState<CanonicalItem[]>([]);
  const [popularSeries, setPopularSeries] = useState<CanonicalItem[]>([]);
  /** The card whose poster morphs in/out of the detail overlay (View
   * Transitions); kept set after close so the reverse morph lands back. */
  const [morph, setMorph] = useState<{ kind: "movie" | "series"; id: string } | null>(
    null,
  );
  /** Canonical detail overlay (Popular click). Home stays mounted so scroll is
   * preserved and the poster morphs back on close. */
  const [canonical, setCanonical] = useState<CanonicalItem | null>(null);
  /** Provider series detail — opened only from a Keep Watching "go to series". */
  const [providerSeries, setProviderSeries] = useState<Series | null>(null);
  const [keepWatching, setKeepWatching] = useState<ContinueWatchingItem[]>([]);
  const [seriesChoice, setSeriesChoice] = useState<SeriesChoice | null>(null);
  const [kwMenu, setKwMenu] = useState<{
    item: ContinueWatchingItem;
    x: number;
    y: number;
  } | null>(null);

  // Canonical Popular rows — provider-agnostic, so they load regardless of the
  // enabled set.
  useEffect(() => {
    let cancelled = false;
    void api.getCanonicalCatalog("movie", undefined, undefined, 0).then(
      (items) => {
        if (!cancelled) setPopularMovies(items.slice(0, ROW_SIZE));
      },
      () => {
        if (!cancelled) setPopularMovies([]);
      },
    );
    void api.getCanonicalCatalog("series", undefined, undefined, 0).then(
      (items) => {
        if (!cancelled) setPopularSeries(items.slice(0, ROW_SIZE));
      },
      () => {
        if (!cancelled) setPopularSeries([]);
      },
    );
    return () => {
      cancelled = true;
    };
  }, []);

  // Keep Watching + My Lists — provider/list data (re-loads on provider set or
  // refresh changes).
  useEffect(() => {
    // Custom lists for the "My Lists" row (global since M39).
    void useListsStore.getState().load();
    if (!hasProviders) {
      setKeepWatching([]);
      return;
    }
    let cancelled = false;
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

  // Open the canonical detail as an in-place overlay with the poster morph.
  const openCanonical = (item: CanonicalItem) => {
    flushSync(() => setMorph({ kind: item.kind, id: item.imdbId }));
    startViewTransition(() => setCanonical(item));
  };
  const closeCanonical = () => startViewTransition(() => setCanonical(null));
  const openProviderSeries = (series: Series) =>
    startViewTransition(() => setProviderSeries(series));
  const closeProviderSeries = () =>
    startViewTransition(() => setProviderSeries(null));

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

  const morphActive = (kind: "movie" | "series", id: string) =>
    canonical === null && morph?.kind === kind && morph.id === id;

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
            getKey={(m) => m.imdbId}
            renderItem={(movie) => (
              <CanonicalCard
                item={movie}
                onActivate={openCanonical}
                morphActive={morphActive("movie", movie.imdbId)}
              />
            )}
          />
          <MediaRow
            title="Popular Series"
            testId="home-popular-series"
            items={popularSeries}
            getKey={(s) => s.imdbId}
            renderItem={(series) => (
              <CanonicalCard
                item={series}
                onActivate={openCanonical}
                morphActive={morphActive("series", series.imdbId)}
              />
            )}
          />

          {empty && (
            <div className="flex h-72 flex-col items-center justify-center gap-2 text-center">
              <p className="text-sm font-medium text-zinc-400">Nothing here yet</p>
              <p className="max-w-sm text-xs text-zinc-600">
                Browse Movies and Series, or start watching something — your
                Popular picks and Keep Watching will show up here.
              </p>
            </div>
          )}
        </div>
      </div>

      {canonical && <CanonicalDetail item={canonical} onClose={closeCanonical} />}
      {providerSeries && (
        <SeriesDetail
          providerId={providerSeries.providerId}
          series={providerSeries}
          onClose={closeProviderSeries}
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
            openProviderSeries(seriesChoice.series);
            setSeriesChoice(null);
          }}
          onClose={() => setSeriesChoice(null)}
        />
      )}
    </div>
  );
}
