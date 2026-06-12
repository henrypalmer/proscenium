import { Poster } from "./PosterGrid";
import type { Movie } from "../../types";

interface MovieCardProps {
  movie: Movie;
  onActivate: (movie: Movie) => void;
  onContextMenu: (movie: Movie, x: number, y: number) => void;
}

/** Poster, title, year (spec §5.4). Click opens the detail view. */
export default function MovieCard({
  movie,
  onActivate,
  onContextMenu,
}: MovieCardProps) {
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
      <Poster url={movie.posterUrl} title={movie.name} />
      <p className="mt-2 truncate text-sm text-zinc-200 group-hover:text-white">
        {movie.name}
      </p>
      <p className="mt-0.5 h-4 text-xs text-zinc-500">
        {movie.releaseYear ?? ""}
      </p>
    </button>
  );
}
