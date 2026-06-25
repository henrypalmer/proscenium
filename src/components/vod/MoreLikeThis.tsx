import { useEffect, useState } from "react";
import * as api from "../../lib/tauri";
import MediaRow from "../home/MediaRow";
import MovieCard from "./MovieCard";
import SeriesCard from "./SeriesCard";
import type { Movie, Series } from "../../types";

interface MoreLikeThisProps {
  providerId: string;
  contentType: "movie" | "series";
  /** The title whose related row this is (excluded from the results). */
  contentId: string;
  /** Open a related movie's detail (movie content type only). */
  onOpenMovie: (movie: Movie) => void;
  /** Open a related series' detail (series content type only). */
  onOpenSeries: (series: Series) => void;
  onContextMenuMovie: (movie: Movie, x: number, y: number) => void;
  onContextMenuSeries: (series: Series, x: number, y: number) => void;
}

/**
 * "More like this" row on the movie/series detail view (spec §5.4 / §13,
 * Milestone 28): same-genre titles of the same content type, resolved locally
 * via `get_related` (no provider request). Reuses the section's standard cards
 * (with the §5.9 watch-progress overlays) inside the shared `MediaRow`/
 * `ScrollRow`, and is omitted entirely when there are no related titles.
 */
export default function MoreLikeThis({
  providerId,
  contentType,
  contentId,
  onOpenMovie,
  onOpenSeries,
  onContextMenuMovie,
  onContextMenuSeries,
}: MoreLikeThisProps) {
  const [movies, setMovies] = useState<Movie[]>([]);
  const [series, setSeries] = useState<Series[]>([]);

  useEffect(() => {
    let cancelled = false;
    setMovies([]);
    setSeries([]);
    void api.getRelated(providerId, contentType, contentId, 20).then(
      (r) => {
        if (cancelled) return;
        setMovies(r.movies);
        setSeries(r.series);
      },
      () => {
        // Discovery aid only; a failure just omits the row.
      },
    );
    return () => {
      cancelled = true;
    };
  }, [providerId, contentType, contentId]);

  if (contentType === "movie") {
    return (
      <MediaRow
        title="More like this"
        testId="more-like-this"
        items={movies}
        getKey={(m) => m.id}
        renderItem={(movie) => (
          <MovieCard
            movie={movie}
            providerId={providerId}
            onActivate={onOpenMovie}
            onContextMenu={onContextMenuMovie}
          />
        )}
      />
    );
  }
  return (
    <MediaRow
      title="More like this"
      testId="more-like-this"
      items={series}
      getKey={(s) => s.id}
      renderItem={(item) => (
        <SeriesCard
          series={item}
          onActivate={onOpenSeries}
          onContextMenu={onContextMenuSeries}
        />
      )}
    />
  );
}
