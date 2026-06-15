import { useEffect, useRef, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import * as api from "../../lib/tauri";
import { useCatalogStore } from "../../store/catalogStore";
import { usePlayerStore } from "../../store/playerStore";
import { useProgressStore } from "../../store/progressStore";
import ChannelCard from "../live/ChannelCard";
import MovieCard from "../vod/MovieCard";
import SeriesCard from "../vod/SeriesCard";
import SearchBar from "./SearchBar";
import type {
  Category,
  LiveChannel,
  Movie,
  SearchContentType,
  SearchResults as SearchResultsData,
  Series,
} from "../../types";

/** The full results screen requests the backend's maximum (spec §5.5: not
 * capped at the overlay's 5-per-group inline preview). */
const RESULT_LIMIT = 500;

const CONTENT_TYPES: SearchContentType[] = ["all", "live", "movies", "series"];

function parseContentType(raw: string | null): SearchContentType {
  return CONTENT_TYPES.includes(raw as SearchContentType)
    ? (raw as SearchContentType)
    : "all";
}

const noop = () => undefined;

/**
 * Full-screen search results (spec §5.5): reached by pressing Enter in the
 * search overlay. Sectioned by content type with the full result set; the
 * committed query and filters live in the URL so they survive refine/back and
 * can be adjusted in place.
 */
export default function SearchResultsPage() {
  const activeProvider = useCatalogStore((s) => s.activeProvider);
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();

  const providerId = activeProvider?.id ?? null;
  const query = searchParams.get("q") ?? "";
  const contentType = parseContentType(searchParams.get("type"));
  const categoryId = searchParams.get("cat");

  const [categories, setCategories] = useState<Category[]>([]);
  const [results, setResults] = useState<SearchResultsData | null>(null);
  const [loading, setLoading] = useState(false);
  /** Bumped per search so stale responses can't overwrite newer ones. */
  const requestSeq = useRef(0);

  // Single writer for the query/filter URL state. Changing content type drops
  // the genre filter, which belongs to the previous type.
  const commit = (q: string, type: SearchContentType, cat: string | null) => {
    const params = new URLSearchParams();
    if (q) params.set("q", q);
    if (type !== "all") params.set("type", type);
    if (cat) params.set("cat", cat);
    setSearchParams(params, { replace: true });
  };

  // Genre/category options for the selected content type (spec §5.5 filters).
  useEffect(() => {
    if (!providerId || contentType === "all") {
      setCategories([]);
      return;
    }
    const fetchCategories =
      contentType === "live"
        ? api.getLiveCategories
        : contentType === "movies"
          ? api.getVodCategories
          : api.getSeriesCategories;
    let cancelled = false;
    void fetchCategories(providerId).then(
      (cats) => {
        if (!cancelled) setCategories(cats);
      },
      () => {
        if (!cancelled) setCategories([]);
      },
    );
    return () => {
      cancelled = true;
    };
  }, [providerId, contentType]);

  // Watch-progress markers for the movie cards (spec §5.9), mirroring Movies.
  useEffect(() => {
    if (providerId) void useProgressStore.getState().loadSection(providerId, "movie");
  }, [providerId]);

  useEffect(() => {
    if (!providerId || query === "") {
      setResults(null);
      setLoading(false);
      return;
    }
    const seq = ++requestSeq.current;
    setLoading(true);
    void api
      .search(providerId, query, contentType, categoryId ?? undefined, RESULT_LIMIT)
      .then(
        (data) => {
          if (requestSeq.current !== seq) return;
          setResults(data);
          setLoading(false);
        },
        () => {
          if (requestSeq.current !== seq) return;
          setResults({ liveChannels: [], movies: [], series: [] });
          setLoading(false);
        },
      );
  }, [providerId, query, contentType, categoryId]);

  const playChannel = (channel: LiveChannel) => {
    if (!providerId) return;
    void usePlayerStore.getState().openContent({
      providerId,
      contentType: "live",
      contentId: channel.id,
      title: channel.name,
    });
  };
  const openMovie = (movie: Movie) =>
    navigate("/movies", { state: { openMovie: movie } });
  const openSeries = (series: Series) =>
    navigate("/shows", { state: { openSeries: series } });

  const empty =
    results !== null &&
    results.liveChannels.length === 0 &&
    results.movies.length === 0 &&
    results.series.length === 0;

  return (
    <div className="flex h-full flex-col">
      <SearchBar
        key={contentType}
        initialText={query}
        onQueryChange={(q) => commit(q, contentType, categoryId)}
        onSubmit={(q) => commit(q, contentType, categoryId)}
        contentType={contentType}
        onContentTypeChange={(type) => commit(query, type, null)}
        categories={categories}
        categoryId={categoryId}
        onCategoryChange={(cat) => commit(query, contentType, cat)}
      />

      <div className="min-h-0 flex-1 overflow-y-auto p-4">
        {!providerId ? (
          <p className="py-16 text-center text-sm text-zinc-600">
            Select a provider in Settings to search its catalog.
          </p>
        ) : query === "" ? (
          <p className="py-16 text-center text-sm text-zinc-600">
            Type a query to search live TV, movies, and TV shows.
          </p>
        ) : results === null ? (
          <p className="py-16 text-center text-sm text-zinc-600">Searching…</p>
        ) : empty && !loading ? (
          <div data-testid="search-page-no-results" className="py-16 text-center">
            <p className="text-sm font-medium text-zinc-400">
              No results for “{query}”.
            </p>
            <p className="mt-1 text-xs text-zinc-600">
              Check the spelling or try a broader term.
            </p>
          </div>
        ) : (
          <div className="space-y-8">
            <ResultSection
              title="Live TV"
              count={results.liveChannels.length}
              layout="list"
              testId="results-page-live"
              items={results.liveChannels}
              getKey={(c) => c.id}
              renderItem={(channel) => (
                <ChannelCard
                  channel={channel}
                  showCategory
                  onActivate={playChannel}
                  onContextMenu={noop}
                />
              )}
            />
            <ResultSection
              title="Movies"
              count={results.movies.length}
              layout="grid"
              testId="results-page-movies"
              items={results.movies}
              getKey={(m) => m.id}
              renderItem={(movie) => (
                <MovieCard
                  movie={movie}
                  providerId={providerId}
                  onActivate={openMovie}
                  onContextMenu={noop}
                />
              )}
            />
            <ResultSection
              title="TV Shows"
              count={results.series.length}
              layout="grid"
              testId="results-page-series"
              items={results.series}
              getKey={(s) => s.id}
              renderItem={(series) => (
                <SeriesCard series={series} onActivate={openSeries} />
              )}
            />
          </div>
        )}
      </div>
    </div>
  );
}

interface ResultSectionProps<T> {
  title: string;
  count: number;
  layout: "list" | "grid";
  testId: string;
  items: T[];
  getKey: (item: T) => string;
  renderItem: (item: T) => React.ReactNode;
}

/** One content-type section, omitted entirely when it has no results
 * (spec §5.5). Unlike the overlay's groups, the full set is rendered. */
function ResultSection<T>({
  title,
  count,
  layout,
  testId,
  items,
  getKey,
  renderItem,
}: ResultSectionProps<T>) {
  if (items.length === 0) return null;
  return (
    <section data-testid={testId}>
      <h2 className="mb-3 text-sm font-semibold uppercase tracking-wide text-zinc-400">
        {title}
        <span className="ml-2 font-normal normal-case text-zinc-600">{count}</span>
      </h2>
      <div
        className={
          layout === "grid"
            ? "grid grid-cols-[repeat(auto-fill,minmax(120px,1fr))] gap-4"
            : "overflow-hidden rounded-lg border border-zinc-900"
        }
      >
        {items.map((item) => (
          <div key={getKey(item)}>{renderItem(item)}</div>
        ))}
      </div>
    </section>
  );
}
