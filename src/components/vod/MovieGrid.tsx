import { usePagedMovies } from "../../hooks/useCatalog";
import MovieCard from "./MovieCard";
import PosterGrid from "./PosterGrid";
import type { Movie } from "../../types";

interface MovieGridProps {
  providerIds: string[];
  categoryId: string | null;
  version: number;
  onActivate: (movie: Movie) => void;
  onContextMenu: (movie: Movie, x: number, y: number) => void;
  /** Id of the card whose poster should morph into the detail view, if any. */
  morphId?: string | null;
}

/** Virtualized grid of `MovieCard` items (spec §18), merged across providers. */
export default function MovieGrid({
  providerIds,
  categoryId,
  version,
  onActivate,
  onContextMenu,
  morphId,
}: MovieGridProps) {
  const { total, getItem, ensureRange } = usePagedMovies(
    providerIds,
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
          morphActive={morphId === movie.id}
        />
      )}
    />
  );
}
