import { Poster } from "./PosterGrid";
import WatchProgressOverlay from "./WatchProgressOverlay";
import { useWatchProgress } from "../../store/progressStore";
import { useProviderBadge } from "../../lib/useProviderBadge";
import type { Movie } from "../../types";

interface MovieCardProps {
  movie: Movie;
  onActivate: (movie: Movie) => void;
  onContextMenu: (movie: Movie, x: number, y: number) => void;
  /** When true, this card's poster carries the shared-element name so it morphs
   * into the detail view on open (and back on close). */
  morphActive?: boolean;
}

/** Poster, title, year (spec §5.4) with a watch-progress overlay (§5.9) and a
 * provider badge when several providers are enabled (Milestone 39). */
export default function MovieCard({
  movie,
  onActivate,
  onContextMenu,
  morphActive,
}: MovieCardProps) {
  const progress = useWatchProgress(movie.providerId, "movie", movie.id);
  const badge = useProviderBadge(movie.providerId);
  return (
    <button
      onClick={() => onActivate(movie)}
      onContextMenu={(e) => {
        e.preventDefault();
        onContextMenu(movie, e.clientX, e.clientY);
      }}
      data-testid="movie-card"
      title={movie.name}
      className="group relative block w-full text-left transition-transform duration-200 ease-out hover:z-10 hover:scale-[1.04] active:scale-[0.98] motion-reduce:transition-none motion-reduce:hover:scale-100"
    >
      <Poster
        url={movie.posterUrl}
        title={movie.name}
        overlay={<WatchProgressOverlay progress={progress} />}
        vtName={morphActive ? "vt-poster" : undefined}
      />
      <p className="mt-2 truncate text-sm text-zinc-200 group-hover:text-white">
        {movie.name}
      </p>
      <p className="mt-0.5 flex h-4 items-center gap-1.5 text-xs text-zinc-500">
        <span>{movie.releaseYear ?? ""}</span>
        {badge && (
          <span className="min-w-0 truncate rounded bg-zinc-800 px-1 text-[10px] text-zinc-400">
            {badge}
          </span>
        )}
      </p>
    </button>
  );
}
