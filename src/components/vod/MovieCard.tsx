import { Poster } from "./PosterGrid";
import WatchProgressOverlay from "./WatchProgressOverlay";
import { useWatchProgress } from "../../store/progressStore";
import type { Movie } from "../../types";

interface MovieCardProps {
  movie: Movie;
  providerId: string;
  onActivate: (movie: Movie) => void;
  onContextMenu: (movie: Movie, x: number, y: number) => void;
  /** When true, this card's poster carries the shared-element name so it morphs
   * into the detail view on open (and back on close). */
  morphActive?: boolean;
}

/** Poster, title, year (spec §5.4) with a watch-progress overlay (§5.9). */
export default function MovieCard({
  movie,
  providerId,
  onActivate,
  onContextMenu,
  morphActive,
}: MovieCardProps) {
  const progress = useWatchProgress(providerId, "movie", movie.id);
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
      <p className="mt-0.5 h-4 text-xs text-zinc-500">
        {movie.releaseYear ?? ""}
      </p>
    </button>
  );
}
