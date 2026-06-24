import { useEffect, useState } from "react";
import * as api from "../../lib/tauri";
import { useCatalogStore } from "../../store/catalogStore";
import { usePlayerStore } from "../../store/playerStore";
import { useProgressStore, useWatchProgress } from "../../store/progressStore";
import { formatDuration, formatTimestamp } from "../../lib/utils";
import AddToListMenu from "../lists/AddToListMenu";
import HeroBackdrop from "./HeroBackdrop";
import { Poster } from "./PosterGrid";
import WatchProgressOverlay from "./WatchProgressOverlay";
import type { Movie, MovieDetail as MovieDetailData } from "../../types";

/** Minimum saved position worth offering a resume for (mirrors playerStore). */
const MIN_RESUME_SECONDS = 5;

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

  // Watch progress (spec §5.9 / Milestone 26): surface resume state on the page
  // itself, not only as a modal after clicking Play. Pull the latest in case it
  // changed since the section was last bulk-loaded.
  const progress = useWatchProgress(providerId, "movie", movie.id);
  useEffect(() => {
    void useProgressStore.getState().syncOne(providerId, "movie", movie.id);
  }, [providerId, movie.id]);
  const inProgress =
    !!progress && !progress.completed && progress.positionSeconds >= MIN_RESUME_SECONDS;

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

  const playArgs = {
    providerId,
    contentType: "movie" as const,
    contentId: movie.id,
    title: movie.name,
  };
  const play = () => void usePlayerStore.getState().openContent(playArgs);
  const resume = () =>
    void usePlayerStore.getState().playDirect(playArgs, progress!.positionSeconds);
  const startOver = () => void usePlayerStore.getState().playDirect(playArgs, 0);

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
      <HeroBackdrop backdropUrl={detail?.backdropUrl ?? null} posterUrl={movie.posterUrl} />
      <div className="relative mx-auto max-w-5xl px-6 pb-12">
        <button
          onClick={onClose}
          data-testid="detail-back"
          className="mt-4 rounded-md bg-zinc-950/40 px-2 py-1 text-sm text-zinc-300 backdrop-blur-sm hover:bg-zinc-900 hover:text-zinc-100"
        >
          ← Back
        </button>
        <div className="flex items-end gap-6 pt-[140px]">
          <div className="w-40 shrink-0 drop-shadow-2xl sm:w-52">
            <Poster
              url={movie.posterUrl}
              title={movie.name}
              vtName="vt-poster"
              overlay={<WatchProgressOverlay progress={progress} showCheck={false} />}
            />
          </div>
          <div className="min-w-0 flex-1 pb-1">
            <h1 className="text-3xl font-bold text-white drop-shadow-md">
              {movie.name}
            </h1>
            <div className="mt-2 flex flex-wrap items-center gap-2 text-sm text-zinc-300">
              {movie.releaseYear && <span>{movie.releaseYear}</span>}
              {duration !== null && <span>· {formatDuration(duration)}</span>}
              {movie.rating && <span>· ★ {movie.rating}</span>}
            </div>
            <div className="mt-3 flex flex-wrap gap-1.5">
              {genres.map((genre) => (
                <span
                  key={genre}
                  className="rounded-full bg-zinc-800/80 px-2.5 py-0.5 text-xs text-zinc-200 backdrop-blur-sm"
                >
                  {genre}
                </span>
              ))}
            </div>
            <div className="mt-6 flex flex-wrap gap-3">
              {inProgress ? (
                <>
                  <button
                    onClick={resume}
                    data-testid="detail-resume"
                    className="rounded-md bg-zinc-100 px-5 py-2 text-sm font-semibold text-zinc-900 hover:bg-white"
                  >
                    ▶ Resume from {formatTimestamp(progress.positionSeconds)}
                  </button>
                  <button
                    onClick={startOver}
                    data-testid="detail-start-over"
                    className="rounded-md border border-zinc-600 bg-zinc-950/40 px-5 py-2 text-sm font-medium text-zinc-100 backdrop-blur-sm hover:bg-zinc-900"
                  >
                    Start over
                  </button>
                </>
              ) : (
                <button
                  onClick={play}
                  data-testid="detail-play"
                  className="rounded-md bg-zinc-100 px-5 py-2 text-sm font-semibold text-zinc-900 hover:bg-white"
                >
                  ▶ Play
                </button>
              )}
              <button
                onClick={() => void openExternal()}
                data-testid="detail-external"
                className="rounded-md border border-zinc-600 bg-zinc-950/40 px-5 py-2 text-sm font-medium text-zinc-100 backdrop-blur-sm hover:bg-zinc-900"
              >
                Open in External Player
              </button>
              <button
                onClick={(e) => {
                  const r = e.currentTarget.getBoundingClientRect();
                  setAddTo({ x: r.left, y: r.bottom });
                }}
                data-testid="detail-add-to-list"
                className="rounded-md border border-zinc-600 bg-zinc-950/40 px-5 py-2 text-sm font-medium text-zinc-100 backdrop-blur-sm hover:bg-zinc-900"
              >
                + Add to list
              </button>
            </div>
          </div>
        </div>
        <div className="mt-8 max-w-3xl">
          <h2 className="mb-2 text-sm font-semibold uppercase tracking-wide text-zinc-500">
            Synopsis
          </h2>
          {detail === null ? (
            <div className="space-y-2">
              <div className="h-3 w-full animate-pulse rounded bg-zinc-900" />
              <div className="h-3 w-11/12 animate-pulse rounded bg-zinc-900" />
              <div className="h-3 w-4/6 animate-pulse rounded bg-zinc-900" />
            </div>
          ) : detail.description ? (
            <p className="prosc-enter text-[15px] leading-relaxed text-zinc-300">
              {detail.description}
            </p>
          ) : (
            <p className="text-sm text-zinc-500">No synopsis is available.</p>
          )}
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
