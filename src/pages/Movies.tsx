import { useEffect, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import ContextMenu from "../components/common/ContextMenu";
import CategoryPanel from "../components/layout/CategoryPanel";
import MovieDetail from "../components/vod/MovieDetail";
import MovieGrid from "../components/vod/MovieGrid";
import * as api from "../lib/tauri";
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

  const [categories, setCategories] = useState<Category[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [detail, setDetail] = useState<Movie | null>(null);
  const [menu, setMenu] = useState<MenuState | null>(null);
  const location = useLocation();
  const navigate = useNavigate();

  const providerId = activeProvider?.id ?? null;

  useEffect(() => {
    setDetail(null);
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
      // Clear the state so back/refresh doesn't reopen the detail.
      navigate(location.pathname, { replace: true, state: null });
    }
  }, [location.state, location.pathname, navigate]);

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
        <MovieGrid
          providerId={activeProvider.id}
          categoryId={selected}
          version={refreshTick}
          onActivate={setDetail}
          onContextMenu={(movie, x, y) => setMenu({ movie, x, y })}
        />
      </div>
      {detail && (
        <MovieDetail
          providerId={activeProvider.id}
          movie={detail}
          onClose={() => setDetail(null)}
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
          ]}
        />
      )}
    </div>
  );
}
