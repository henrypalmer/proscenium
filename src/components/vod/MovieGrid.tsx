import { usePagedMovies } from "../../hooks/useCatalog";
import MovieCard from "./MovieCard";
import PosterGrid from "./PosterGrid";
import type { Movie } from "../../types";

interface MovieGridProps {
  providerId: string;
  categoryId: string | null;
  version: number;
  onActivate: (movie: Movie) => void;
  onContextMenu: (movie: Movie, x: number, y: number) => void;
}

/** Virtualized grid of `MovieCard` items (spec §18). */
export default function MovieGrid({
  providerId,
  categoryId,
  version,
  onActivate,
  onContextMenu,
}: MovieGridProps) {
  const { total, getItem, ensureRange } = usePagedMovies(
    providerId,
    categoryId,
    version,
  );
  return (
    <PosterGrid<Movie>
      total={total}
      getItem={getItem}
      ensureRange={ensureRange}
      resetKey={`${categoryId ?? "all"}:${version}`}
      emptyNoun="movies"
      filtered={categoryId !== null}
      renderCard={(movie) => (
        <MovieCard
          movie={movie}
          onActivate={onActivate}
          onContextMenu={onContextMenu}
        />
      )}
    />
  );
}
