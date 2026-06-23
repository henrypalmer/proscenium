import { useEffect, useState } from "react";
import * as api from "../../lib/tauri";
import { useCatalogStore } from "../../store/catalogStore";
import { usePlayerStore } from "../../store/playerStore";
import { formatDuration } from "../../lib/utils";
import AddToListMenu from "../lists/AddToListMenu";
import { Poster } from "./PosterGrid";
import type { Movie, MovieDetail as MovieDetailData } from "../../types";

interface MovieDetailProps {
  providerId: string;
  /** Base row from the grid — rendered immediately while metadata loads. */
  movie: Movie;
  onClose: () => void;
}

/**
 * Movie detail panel (spec §5.4): poster, title/year/genre tags, synopsis
 * (when the provider supplies one via vod_info), play and external player
 * buttons. Overlays the grid; Esc or Back returns without losing scroll.
 */
export default function MovieDetail({
  providerId,
  movie,
  onClose,
}: MovieDetailProps) {
  const notify = useCatalogStore((s) => s.notify);
  const [detail, setDetail] = useState<MovieDetailData | null>(null);
  const [addTo, setAddTo] = useState<{ x: number; y: number } | null>(null);

  useEffect(() => {
    let cancelled = false;
    void api.getMovieDetail(providerId, movie.id).then(
      (d) => {
        if (!cancelled) setDetail(d);
      },
      () => {
        // Metadata is optional; the base row already renders.
      },
    );
    return () => {
      cancelled = true;
    };
  }, [providerId, movie.id]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      // The player overlay owns Esc while it is open.
      if (e.key === "Escape" && !usePlayerStore.getState().open) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const play = () =>
    void usePlayerStore.getState().openContent({
      providerId,
      contentType: "movie",
      contentId: movie.id,
      title: movie.name,
    });

  const openExternal = async () => {
    try {
      const url = await api.resolveStreamUrl(providerId, "movie", movie.id);
      await api.openInExternalPlayer(url);
    } catch (e) {
      notify(String(e), "error");
    }
  };

  const genres = (detail?.genre ?? movie.categoryName)
    .split(",")
    .map((g) => g.trim())
    .filter(Boolean);
  const duration = detail?.durationSeconds ?? null;

  return (
    <div
      data-testid="movie-detail"
      className="absolute inset-0 z-20 overflow-y-auto bg-zinc-950"
    >
      <div className="mx-auto max-w-4xl p-6">
        <button
          onClick={onClose}
          data-testid="detail-back"
          className="mb-4 rounded-md px-2 py-1 text-sm text-zinc-400 hover:bg-zinc-900 hover:text-zinc-100"
        >
          ← Back
        </button>
        <div className="flex gap-6">
          <div className="w-48 shrink-0 sm:w-56">
            <Poster url={movie.posterUrl} title={movie.name} vtName="vt-poster" />
          </div>
          <div className="min-w-0 flex-1">
            <h1 className="text-2xl font-semibold text-white">{movie.name}</h1>
            <div className="mt-2 flex flex-wrap items-center gap-2 text-sm text-zinc-400">
              {movie.releaseYear && <span>{movie.releaseYear}</span>}
              {duration !== null && <span>· {formatDuration(duration)}</span>}
              {movie.rating && <span>· ★ {movie.rating}</span>}
            </div>
            <div className="mt-3 flex flex-wrap gap-1.5">
              {genres.map((genre) => (
                <span
                  key={genre}
                  className="rounded-full bg-zinc-800 px-2.5 py-0.5 text-xs text-zinc-300"
                >
                  {genre}
                </span>
              ))}
            </div>
            <div className="mt-6 flex gap-3">
              <button
                onClick={play}
                data-testid="detail-play"
                className="rounded-md bg-zinc-100 px-5 py-2 text-sm font-semibold text-zinc-900 hover:bg-white"
              >
                ▶ Play
              </button>
              <button
                onClick={() => void openExternal()}
                data-testid="detail-external"
                className="rounded-md border border-zinc-700 px-5 py-2 text-sm font-medium text-zinc-200 hover:bg-zinc-900"
              >
                Open in External Player
              </button>
              <button
                onClick={(e) => {
                  const r = e.currentTarget.getBoundingClientRect();
                  setAddTo({ x: r.left, y: r.bottom });
                }}
                data-testid="detail-add-to-list"
                className="rounded-md border border-zinc-700 px-5 py-2 text-sm font-medium text-zinc-200 hover:bg-zinc-900"
              >
                + Add to list
              </button>
            </div>
            {detail === null ? (
              <div className="mt-6 space-y-2">
                <div className="h-3 w-full animate-pulse rounded bg-zinc-900" />
                <div className="h-3 w-5/6 animate-pulse rounded bg-zinc-900" />
              </div>
            ) : (
              detail.description && (
                <p className="prosc-enter mt-6 max-w-prose text-sm leading-relaxed text-zinc-400">
                  {detail.description}
                </p>
              )
            )}
          </div>
        </div>
      </div>
      {addTo && (
        <AddToListMenu
          providerId={providerId}
          contentType="movie"
          contentId={movie.id}
          x={addTo.x}
          y={addTo.y}
          onClose={() => setAddTo(null)}
        />
      )}
    </div>
  );
}
