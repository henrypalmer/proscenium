import { useCallback, useEffect, useRef, useState } from "react";
import { flushSync } from "react-dom";
import { useLocation, useNavigate } from "react-router-dom";
import ContextMenu from "../components/common/ContextMenu";
import AddToListMenu from "../components/lists/AddToListMenu";
import CategoryPanel from "../components/layout/CategoryPanel";
import GenreRows from "../components/vod/GenreRows";
import MovieCard from "../components/vod/MovieCard";
import MovieDetail from "../components/vod/MovieDetail";
import MovieGrid from "../components/vod/MovieGrid";
import * as api from "../lib/tauri";
import { startViewTransition } from "../lib/viewTransition";
import { useCatalogStore } from "../store/catalogStore";
import { usePlayerStore } from "../store/playerStore";
import { useProgressStore } from "../store/progressStore";
import type { Category, Movie } from "../types";

interface MenuState {
  movie: Movie;
  x: number;
  y: number;
}

export default function Movies() {
  const activeProvider = useCatalogStore((s) => s.activeProvider);
  const refreshTick = useCatalogStore((s) => s.refreshTick);
  const notify = useCatalogStore((s) => s.notify);

  const location = useLocation();
  const navigate = useNavigate();
  // Home/Search navigate here with a movie to open immediately. Initialize the
  // detail from that state so it is present on the first *synchronous* render —
  // the View Transitions snapshot is taken right after the navigation commits
  // (before effects run), so this is what lets the poster morph across the route
  // change (Milestone 17).
  const navMovie = (location.state as { openMovie?: Movie } | null)?.openMovie ?? null;

  const [categories, setCategories] = useState<Category[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [detail, setDetail] = useState<Movie | null>(navMovie);
  /** Card whose poster morphs in/out of the detail view (View Transitions). */
  const [selectedId, setSelectedId] = useState<string | null>(null);
  /** True when the open detail was reached by navigation (Home/Search) rather
   * than a click within this section's grid — closing it then goes back. */
  const [detailFromNav, setDetailFromNav] = useState(navMovie !== null);
  const [menu, setMenu] = useState<MenuState | null>(null);
  const [addTo, setAddTo] = useState<{ id: string; x: number; y: number } | null>(null);

  const providerId = activeProvider?.id ?? null;

  // Skip the detail reset on the very first run so a nav-provided detail
  // (initialized above) survives mount; later provider/refresh changes still
  // close any open detail.
  const firstCatRun = useRef(true);
  useEffect(() => {
    if (firstCatRun.current) {
      firstCatRun.current = false;
    } else {
      setDetail(null);
      setDetailFromNav(false);
    }
    if (!providerId) {
      setCategories([]);
      return;
    }
    // Watch progress for the movie grid overlays (spec §5.9).
    void useProgressStore.getState().loadSection(providerId, "movie");
    let cancelled = false;
    void api.getVodCategories(providerId).then(
      (cats) => {
        if (cancelled) return;
        setCategories(cats);
        // Drop a selection that disappeared with the latest refresh.
        setSelected((current) =>
          current && !cats.some((c) => c.id === current) ? null : current,
        );
      },
      () => {
        if (!cancelled) setCategories([]);
      },
    );
    return () => {
      cancelled = true;
    };
  }, [providerId, refreshTick]);

  // A search result navigated here asking for a detail view (Milestone 6).
  // Declared after the categories effect: that one resets the detail on
  // mount and must not clobber the requested view.
  useEffect(() => {
    const state = location.state as { openMovie?: Movie } | null;
    if (state?.openMovie) {
      setDetail(state.openMovie);
      setDetailFromNav(true);
      // Clear the state so back/refresh doesn't reopen the detail.
      navigate(location.pathname, { replace: true, state: null });
    }
  }, [location.state, location.pathname, navigate]);

  // Per-genre strip fetcher for the "All" overview (memoized so a row only
  // refetches when the provider changes, not on every parent render).
  const fetchMoviePage = useCallback(
    (catId: string): Promise<Movie[]> =>
      providerId
        ? api.getMovies(providerId, catId, 1, 30).then((r) => r.items)
        : Promise.resolve([]),
    [providerId],
  );

  if (!activeProvider) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-2 text-center">
        <p className="text-sm font-medium text-zinc-400">No provider selected</p>
        <p className="max-w-xs text-xs text-zinc-600">
          Add or select a provider in Settings to browse movies.
        </p>
      </div>
    );
  }

  // Open a detail from a grid click (closing returns to the grid). The clicked
  // poster morphs into the detail's poster via View Transitions: the grid card
  // is flushed to carry the shared name *before* the "before" snapshot, then the
  // detail mount is the transitioned update.
  const openDetail = (movie: Movie) => {
    setDetailFromNav(false);
    flushSync(() => setSelectedId(movie.id));
    startViewTransition(() => setDetail(movie));
  };
  // Closing returns to the previous page when we arrived via navigation
  // (e.g. Home or Search), otherwise it morphs back into the grid card.
  const closeDetail = () => {
    if (detailFromNav) navigate(-1);
    else startViewTransition(() => setDetail(null));
  };

  // The grid card carries the shared-element name only while the detail is
  // closed, so the name is never on two elements at once during a transition.
  const morphId = detail ? null : selectedId;

  const providerIdForPlayback = activeProvider.id;
  const play = (movie: Movie) =>
    void usePlayerStore.getState().openContent({
      providerId: providerIdForPlayback,
      contentType: "movie",
      contentId: movie.id,
      title: movie.name,
    });
  const openExternal = async (movie: Movie) => {
    try {
      const url = await api.resolveStreamUrl(
        providerIdForPlayback,
        "movie",
        movie.id,
      );
      await api.openInExternalPlayer(url);
    } catch (e) {
      notify(String(e), "error");
    }
  };

  return (
    <div className="relative flex h-full">
      <CategoryPanel
        title="Genres"
        allLabel="All Movies"
        categories={categories}
        selectedId={selected}
        onSelect={setSelected}
      />
      <div className="min-w-0 flex-1">
        {/* "All Movies" → per-genre row stack (M19); a selected genre → the
            existing full virtualized grid. Falls back to the grid when the
            provider exposes no genres. */}
        {selected === null && categories.length > 0 ? (
          <GenreRows<Movie>
            categories={categories}
            resetKey={`${activeProvider.id}:${refreshTick}`}
            fetchPage={fetchMoviePage}
            getKey={(m) => m.id}
            onSelectGenre={setSelected}
            renderCard={(movie) => (
              <MovieCard
                movie={movie}
                providerId={activeProvider.id}
                onActivate={openDetail}
                onContextMenu={(m, x, y) => setMenu({ movie: m, x, y })}
                morphActive={morphId === movie.id}
              />
            )}
          />
        ) : (
          <MovieGrid
            providerId={activeProvider.id}
            categoryId={selected}
            version={refreshTick}
            onActivate={openDetail}
            onContextMenu={(movie, x, y) => setMenu({ movie, x, y })}
            morphId={morphId}
          />
        )}
      </div>
      {detail && (
        <MovieDetail
          providerId={activeProvider.id}
          movie={detail}
          onClose={closeDetail}
        />
      )}
      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          onClose={() => setMenu(null)}
          items={[
            { label: "Play", onSelect: () => play(menu.movie) },
            {
              label: "Open in External Player",
              onSelect: () => void openExternal(menu.movie),
            },
            {
              label: "Add to list…",
              onSelect: () =>
                setAddTo({ id: menu.movie.id, x: menu.x, y: menu.y }),
            },
          ]}
        />
      )}
      {addTo && (
        <AddToListMenu
          providerId={activeProvider.id}
          contentType="movie"
          contentId={addTo.id}
          x={addTo.x}
          y={addTo.y}
          onClose={() => setAddTo(null)}
        />
      )}
    </div>
  );
}
