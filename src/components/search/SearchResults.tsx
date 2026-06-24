import ChannelCard from "../live/ChannelCard";
import MovieCard from "../vod/MovieCard";
import SeriesCard from "../vod/SeriesCard";
import SearchResultGroup from "./SearchResultGroup";
import type {
  LiveChannel,
  Movie,
  SearchResults as SearchResultsData,
  Series,
} from "../../types";

/** The keyboard-highlighted result across all groups (Milestone 23). */
export interface ActiveResult {
  kind: "live" | "movie" | "series";
  id: string;
}

interface SearchResultsProps {
  providerId: string;
  query: string;
  loading: boolean;
  results: SearchResultsData | null;
  onPlayChannel: (channel: LiveChannel) => void;
  onOpenMovie: (movie: Movie) => void;
  onOpenSeries: (series: Series) => void;
  active?: ActiveResult | null;
}

const noop = () => undefined;

/** The three content-type groups (spec §5.5), the friendly no-results
 * state, and the idle hint before anything has been typed. */
export default function SearchResults({
  providerId,
  query,
  loading,
  results,
  onPlayChannel,
  onOpenMovie,
  onOpenSeries,
  active,
}: SearchResultsProps) {
  if (query === "") {
    return (
      <p className="px-4 py-10 text-center text-sm text-zinc-600">
        Search across live TV, movies, and series.
      </p>
    );
  }

  if (!results) {
    // First fetch for this query is still in flight.
    return (
      <p className="px-4 py-10 text-center text-sm text-zinc-600">Searching…</p>
    );
  }

  const empty =
    results.liveChannels.length === 0 &&
    results.movies.length === 0 &&
    results.series.length === 0;

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
      <SearchResultGroup
        title="Live TV"
        items={results.liveChannels}
        layout="list"
        testId="search-group-live"
        activeId={active?.kind === "live" ? active.id : undefined}
        getKey={(c) => c.id}
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
        getKey={(m) => m.id}
        renderItem={(movie) => (
          <MovieCard
            movie={movie}
            providerId={providerId}
            onActivate={onOpenMovie}
            onContextMenu={noop}
          />
        )}
      />
      <SearchResultGroup
        title="Series"
        items={results.series}
        layout="grid"
        testId="search-group-series"
        activeId={active?.kind === "series" ? active.id : undefined}
        getKey={(s) => s.id}
        renderItem={(series) => (
          <SeriesCard series={series} onActivate={onOpenSeries} />
        )}
      />
    </div>
  );
}
