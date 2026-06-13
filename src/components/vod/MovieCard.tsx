import { Poster } from "./PosterGrid";
import WatchProgressOverlay from "./WatchProgressOverlay";
import { useWatchProgress } from "../../store/progressStore";
import type { Movie } from "../../types";

interface MovieCardProps {
  movie: Movie;
  providerId: string;
  onActivate: (movie: Movie) => void;
  onContextMenu: (movie: Movie, x: number, y: number) => void;
}

/** Poster, title, year (spec §5.4) with a watch-progress overlay (§5.9). */
export default function MovieCard({
  movie,
  providerId,
  onActivate,
  onContextMenu,
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
      className="group block w-full text-left"
    >
      <Poster
        url={movie.posterUrl}
        title={movie.name}
        overlay={<WatchProgressOverlay progress={progress} />}
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
