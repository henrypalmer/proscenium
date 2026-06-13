import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import * as api from "../../lib/tauri";
import { useCatalogStore } from "../../store/catalogStore";
import { usePlayerStore } from "../../store/playerStore";
import { useSearchStore } from "../../store/searchStore";
import SearchBar from "./SearchBar";
import SearchResults from "./SearchResults";
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

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
        e.preventDefault(); // also suppresses the WebView's native find bar
        // The player overlay has its own keyboard surface; search stays out.
        if (!usePlayerStore.getState().open) setOpen(true);
      } else if (e.key === "Escape" && useSearchStore.getState().open) {
        e.preventDefault();
        setOpen(false);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [setOpen]);

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
            />
          )}
        </div>
      </div>
    </div>
  );
}
