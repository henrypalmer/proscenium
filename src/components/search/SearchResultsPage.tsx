import { useEffect, useMemo, useRef, useState } from "react";
import { flushSync } from "react-dom";
import { useNavigate, useSearchParams } from "react-router-dom";
import * as api from "../../lib/tauri";
import { startViewTransition } from "../../lib/viewTransition";
import { applyHideKeys, computeSearchHideKeys } from "../../lib/searchDedup";
import { useCatalogStore } from "../../store/catalogStore";
import { usePlayerStore } from "../../store/playerStore";
import { useProgressStore } from "../../store/progressStore";
import CanonicalCard from "../canonical/CanonicalCard";
import ChannelCard from "../live/ChannelCard";
import MovieCard from "../vod/MovieCard";
import SeriesCard from "../vod/SeriesCard";
import SearchBar from "./SearchBar";
import type {
  CanonicalItem,
  CanonicalSearchResults,
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
  const providerIds = useCatalogStore((s) => s.providerIds);
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();

  const hasProviders = providerIds.length > 0;
  const scopeKey = providerIds.join(",");
  const query = searchParams.get("q") ?? "";
  const contentType = parseContentType(searchParams.get("type"));
  const categoryId = searchParams.get("cat");

  const [categories, setCategories] = useState<Category[]>([]);
  const [results, setResults] = useState<SearchResultsData | null>(null);
  const [canonical, setCanonical] = useState<CanonicalSearchResults | null>(null);
  /** Provider hit keys hidden as duplicates of a canonical hit (M44). */
  const [hideKeys, setHideKeys] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(false);
  /** The result card whose poster morphs into the detail view on navigation. */
  const [morph, setMorph] = useState<{ type: "movie" | "series"; id: string } | null>(
    null,
  );
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
    if (!hasProviders || contentType === "all") {
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
    void fetchCategories(providerIds).then(
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scopeKey, contentType]);

  // Watch-progress markers for the movie cards (spec §5.9), mirroring Movies.
  useEffect(() => {
    if (hasProviders) void useProgressStore.getState().loadSection(providerIds, "movie");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scopeKey]);

  useEffect(() => {
    if (query === "") {
      setResults(null);
      setCanonical(null);
      setLoading(false);
      return;
    }
    const seq = ++requestSeq.current;
    setLoading(true);
    // Local provider catalog (FTS5) — only meaningful with providers enabled.
    if (hasProviders) {
      void api
        .search(providerIds, query, contentType, categoryId ?? undefined, RESULT_LIMIT)
        .then(
          (data) => {
            if (requestSeq.current === seq) setResults(data);
          },
          () => {
            if (requestSeq.current === seq)
              setResults({ liveChannels: [], movies: [], series: [] });
          },
        );
    } else {
      setResults({ liveChannels: [], movies: [], series: [] });
    }
    // Canonical (Cinemeta) search runs regardless of providers (M43); it owns the
    // loading flag as the slower, network-bound half.
    void api
      .searchCanonical(query)
      .then(
        (data) => {
          if (requestSeq.current === seq) setCanonical(data);
        },
        () => {
          if (requestSeq.current === seq) setCanonical({ movies: [], series: [] });
        },
      )
      .finally(() => {
        if (requestSeq.current === seq) setLoading(false);
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scopeKey, query, contentType, categoryId, hasProviders]);

  // Hide provider hits that duplicate a canonical ("All Sources") hit (M44),
  // once both sides are in — the canonical entry with its picker is kept.
  useEffect(() => {
    let cancelled = false;
    void computeSearchHideKeys(results, canonical).then((keys) => {
      if (!cancelled) setHideKeys(keys);
    });
    return () => {
      cancelled = true;
    };
  }, [results, canonical]);

  const playChannel = (channel: LiveChannel) => {
    void usePlayerStore.getState().openContent({
      providerId: channel.providerId,
      contentType: "live",
      contentId: channel.id,
      title: channel.name,
    });
  };
  // Name the clicked poster, then navigate; the deferred transition waits for
  // the destination detail to mount so the poster morphs across the route
  // change (Milestone 17).
  const openMovie = (movie: Movie) => {
    flushSync(() => setMorph({ type: "movie", id: movie.id }));
    startViewTransition(() =>
      navigate("/movies", { state: { openMovie: movie } }),
    );
  };
  const openSeries = (series: Series) => {
    flushSync(() => setMorph({ type: "series", id: series.id }));
    startViewTransition(() =>
      navigate("/shows", { state: { openSeries: series } }),
    );
  };
  // Canonical hit → its kind's page, which opens the canonical detail + picker
  // from nav state (M43). No poster morph (canonical cards aren't morph sources).
  const openCanonical = (item: CanonicalItem) => {
    navigate(item.kind === "movie" ? "/movies" : "/shows", {
      state: { openCanonical: item },
    });
  };

  // Canonical (Cinemeta) hits under "All Sources", filtered by the content-type
  // tab (Live has no canonical equivalent).
  const canonicalItems: CanonicalItem[] =
    !canonical || contentType === "live"
      ? []
      : contentType === "movies"
        ? canonical.movies
        : contentType === "series"
          ? canonical.series
          : [...canonical.movies, ...canonical.series];

  // Provider results with canonical duplicates removed (M44).
  const dedupedResults = useMemo(
    () => applyHideKeys(results, hideKeys),
    [results, hideKeys],
  );

  const empty =
    (dedupedResults === null ||
      (dedupedResults.liveChannels.length === 0 &&
        dedupedResults.movies.length === 0 &&
        dedupedResults.series.length === 0)) &&
    canonicalItems.length === 0;

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
        {query === "" ? (
          <p className="py-16 text-center text-sm text-zinc-600">
            Type a query to search live TV, movies, and series.
          </p>
        ) : empty && loading ? (
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
              count={dedupedResults?.liveChannels.length ?? 0}
              layout="list"
              testId="results-page-live"
              items={dedupedResults?.liveChannels ?? []}
              getKey={(c) => `${c.providerId}:${c.id}`}
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
              count={dedupedResults?.movies.length ?? 0}
              layout="grid"
              testId="results-page-movies"
              items={dedupedResults?.movies ?? []}
              getKey={(m) => `${m.providerId}:${m.id}`}
              renderItem={(movie) => (
                <MovieCard
                  movie={movie}
                  onActivate={openMovie}
                  onContextMenu={noop}
                  morphActive={morph?.type === "movie" && morph.id === movie.id}
                />
              )}
            />
            <ResultSection
              title="Series"
              count={dedupedResults?.series.length ?? 0}
              layout="grid"
              testId="results-page-series"
              items={dedupedResults?.series ?? []}
              getKey={(s) => `${s.providerId}:${s.id}`}
              renderItem={(series) => (
                <SeriesCard
                  series={series}
                  onActivate={openSeries}
                  morphActive={morph?.type === "series" && morph.id === series.id}
                />
              )}
            />
            <ResultSection
              title="All Sources"
              count={canonicalItems.length}
              layout="grid"
              testId="results-page-canonical"
              items={canonicalItems}
              getKey={(it) => it.imdbId}
              renderItem={(item) => (
                <CanonicalCard item={item} onActivate={openCanonical} />
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
