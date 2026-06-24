import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import * as api from "../../lib/tauri";
import { useWindowKeydown } from "../../lib/keyboard";
import { useCatalogStore } from "../../store/catalogStore";
import { usePlayerStore } from "../../store/playerStore";
import { useSearchStore } from "../../store/searchStore";
import SearchBar from "./SearchBar";
import SearchResults, { type ActiveResult } from "./SearchResults";
import { INLINE_LIMIT } from "./SearchResultGroup";
import type {
  Category,
  LiveChannel,
  Movie,
  SearchContentType,
  SearchResults as SearchResultsData,
  Series,
} from "../../types";

/** Per-group fetch budget; the UI shows 5 inline and the rest behind the
 * "Show all" expander (spec §5.5). */
const RESULT_LIMIT = 100;

/**
 * Global search modal (spec §5.5). Always mounted inside the router so the
 * Cmd/Ctrl+F shortcut works from any section; the panel itself only renders
 * while open, which also resets its state between searches.
 */
export default function SearchOverlay() {
  const open = useSearchStore((s) => s.open);
  const setOpen = useSearchStore((s) => s.setOpen);

  useWindowKeydown(
    (e) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
        e.preventDefault(); // also suppresses the WebView's native find bar
        // The player overlay has its own keyboard surface; search stays out.
        if (!usePlayerStore.getState().open) setOpen(true);
      } else if (e.key === "Escape" && useSearchStore.getState().open) {
        e.preventDefault();
        setOpen(false);
      }
    },
    [setOpen],
  );

  if (!open) return null;
  return <SearchPanel onClose={() => setOpen(false)} />;
}

function SearchPanel({ onClose }: { onClose: () => void }) {
  const activeProvider = useCatalogStore((s) => s.activeProvider);
  const navigate = useNavigate();

  const [query, setQuery] = useState("");
  const [contentType, setContentType] = useState<SearchContentType>("all");
  const [categoryId, setCategoryId] = useState<string | null>(null);
  const [categories, setCategories] = useState<Category[]>([]);
  const [results, setResults] = useState<SearchResultsData | null>(null);
  const [loading, setLoading] = useState(false);
  /** Bumped per search so stale responses can't overwrite newer ones. */
  const requestSeq = useRef(0);

  const providerId = activeProvider?.id ?? null;

  // Genre/category options for the selected content type (spec §5.5 filters).
  useEffect(() => {
    setCategoryId(null);
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
    onClose();
    void usePlayerStore.getState().openContent({
      providerId,
      contentType: "live",
      contentId: channel.id,
      title: channel.name,
    });
  };
  const openMovie = (movie: Movie) => {
    onClose();
    navigate("/movies", { state: { openMovie: movie } });
  };
  const openSeries = (series: Series) => {
    onClose();
    navigate("/shows", { state: { openSeries: series } });
  };

  // Spec §5.5: Enter commits the search — close the overlay and navigate to
  // the full, sectioned results screen, carrying the active filters in the URL.
  // A blank/whitespace query does nothing.
  const submitSearch = (committed: string) => {
    if (committed === "") return;
    const params = new URLSearchParams({ q: committed });
    if (contentType !== "all") params.set("type", contentType);
    if (categoryId) params.set("cat", categoryId);
    onClose();
    navigate(`/search?${params.toString()}`);
  };

  // --- Keyboard navigation of the result list (spec §5.5, Milestone 23) ---
  // A flat sequence over the inline-visible results (the first 5 of each group,
  // matching the rendered preview) so ↑/↓ move a single highlight across groups.
  type NavItem =
    | { kind: "live"; item: LiveChannel }
    | { kind: "movie"; item: Movie }
    | { kind: "series"; item: Series };

  const navItems = useMemo<NavItem[]>(() => {
    if (!results) return [];
    return [
      ...results.liveChannels
        .slice(0, INLINE_LIMIT)
        .map((item): NavItem => ({ kind: "live", item })),
      ...results.movies
        .slice(0, INLINE_LIMIT)
        .map((item): NavItem => ({ kind: "movie", item })),
      ...results.series
        .slice(0, INLINE_LIMIT)
        .map((item): NavItem => ({ kind: "series", item })),
    ];
  }, [results]);

  const [activeIndex, setActiveIndex] = useState(-1);
  // Reset the highlight whenever the result set changes (new query/filter).
  useEffect(() => setActiveIndex(-1), [navItems]);

  const activateNav = (nav: NavItem) => {
    if (nav.kind === "live") playChannel(nav.item);
    else if (nav.kind === "movie") openMovie(nav.item);
    else openSeries(nav.item);
  };

  // Combobox keys on the input: ↑/↓ move the highlight, Enter opens the
  // highlighted result (otherwise the input commits the full search).
  const handleNavKey = (e: React.KeyboardEvent<HTMLInputElement>): boolean => {
    if (navItems.length === 0) return false;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      setActiveIndex((i) => Math.min(i + 1, navItems.length - 1));
      return true;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      setActiveIndex((i) => Math.max(i - 1, -1));
      return true;
    }
    if (e.key === "Enter" && activeIndex >= 0 && activeIndex < navItems.length) {
      e.preventDefault();
      activateNav(navItems[activeIndex]);
      return true;
    }
    return false;
  };

  const activeItem =
    activeIndex >= 0 && activeIndex < navItems.length ? navItems[activeIndex] : null;
  const activeResult: ActiveResult | null = activeItem
    ? { kind: activeItem.kind, id: activeItem.item.id }
    : null;

  return (
    <div
      data-testid="search-overlay"
      className="fixed inset-0 z-50 flex justify-center bg-black/60 backdrop-blur-sm"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="mt-14 flex h-fit max-h-[80vh] w-full max-w-3xl flex-col overflow-hidden rounded-xl border border-zinc-800 bg-zinc-950 shadow-2xl">
        <SearchBar
          onQueryChange={setQuery}
          onSubmit={submitSearch}
          onKeyNav={handleNavKey}
          contentType={contentType}
          onContentTypeChange={setContentType}
          categories={categories}
          categoryId={categoryId}
          onCategoryChange={setCategoryId}
        />
        <div className="min-h-24 overflow-y-auto">
          {!providerId ? (
            <p className="px-4 py-10 text-center text-sm text-zinc-600">
              Select a provider in Settings to search its catalog.
            </p>
          ) : (
            <SearchResults
              providerId={providerId}
              query={query}
              loading={loading}
              results={results}
              onPlayChannel={playChannel}
              onOpenMovie={openMovie}
              onOpenSeries={openSeries}
              active={activeResult}
            />
          )}
        </div>
      </div>
    </div>
  );
}
