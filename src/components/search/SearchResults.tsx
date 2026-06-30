import CanonicalCard from "../canonical/CanonicalCard";
import ChannelCard from "../live/ChannelCard";
import MovieCard from "../vod/MovieCard";
import SeriesCard from "../vod/SeriesCard";
import SearchResultGroup from "./SearchResultGroup";
import type {
  CanonicalItem,
  LiveChannel,
  Movie,
  SearchResults as SearchResultsData,
  Series,
} from "../../types";

/** The keyboard-highlighted result across all groups (Milestone 23). */
export interface ActiveResult {
  kind: "live" | "movie" | "series" | "canonical";
  id: string;
}

interface SearchResultsProps {
  query: string;
  loading: boolean;
  results: SearchResultsData | null;
  /** Canonical (Cinemeta) hits, pre-filtered by the active content-type tab (M43). */
  canonicalItems: CanonicalItem[];
  onPlayChannel: (channel: LiveChannel) => void;
  onOpenMovie: (movie: Movie) => void;
  onOpenSeries: (series: Series) => void;
  onOpenCanonical: (item: CanonicalItem) => void;
  active?: ActiveResult | null;
}

const noop = () => undefined;

/** The provider content-type groups (spec §5.5) plus the canonical "All Sources"
 * group (M43), the friendly no-results state, and the idle hint before anything
 * has been typed. */
export default function SearchResults({
  query,
  loading,
  results,
  canonicalItems,
  onPlayChannel,
  onOpenMovie,
  onOpenSeries,
  onOpenCanonical,
  active,
}: SearchResultsProps) {
  if (query === "") {
    return (
      <p className="px-4 py-10 text-center text-sm text-zinc-600">
        Search across live TV, movies, and series.
      </p>
    );
  }

  const localEmpty =
    !results ||
    (results.liveChannels.length === 0 &&
      results.movies.length === 0 &&
      results.series.length === 0);
  const empty = localEmpty && canonicalItems.length === 0;

  if (empty && loading) {
    // Nothing to show yet and a fetch is still in flight (the canonical lookup
    // is a network call, so it can trail the instant local results).
    return (
      <p className="px-4 py-10 text-center text-sm text-zinc-600">Searching…</p>
    );
  }

  if (empty && !loading) {
    return (
      <div data-testid="search-no-results" className="px-4 py-10 text-center">
        <p className="text-sm font-medium text-zinc-400">
          No results for “{query}”.
        </p>
        <p className="mt-1 text-xs text-zinc-600">
          Check the spelling or try a broader term.
        </p>
      </div>
    );
  }

  return (
    <div className="divide-y divide-zinc-900">
      {results && (
        <>
          <SearchResultGroup
            title="Live TV"
            items={results.liveChannels}
            layout="list"
            testId="search-group-live"
            activeId={active?.kind === "live" ? active.id : undefined}
            getKey={(c) => `${c.providerId}:${c.id}`}
            renderItem={(channel) => (
              <ChannelCard
                channel={channel}
                showCategory
                onActivate={onPlayChannel}
                onContextMenu={noop}
              />
            )}
          />
          <SearchResultGroup
            title="Movies"
            items={results.movies}
            layout="grid"
            testId="search-group-movies"
            activeId={active?.kind === "movie" ? active.id : undefined}
            getKey={(m) => `${m.providerId}:${m.id}`}
            renderItem={(movie) => (
              <MovieCard movie={movie} onActivate={onOpenMovie} onContextMenu={noop} />
            )}
          />
          <SearchResultGroup
            title="Series"
            items={results.series}
            layout="grid"
            testId="search-group-series"
            activeId={active?.kind === "series" ? active.id : undefined}
            getKey={(s) => `${s.providerId}:${s.id}`}
            renderItem={(series) => (
              <SeriesCard series={series} onActivate={onOpenSeries} />
            )}
          />
        </>
      )}
      <SearchResultGroup
        title="All Sources"
        items={canonicalItems}
        layout="grid"
        testId="search-group-canonical"
        activeId={active?.kind === "canonical" ? active.id : undefined}
        getKey={(it) => it.imdbId}
        renderItem={(item) => (
          <CanonicalCard item={item} onActivate={onOpenCanonical} />
        )}
      />
    </div>
  );
}
