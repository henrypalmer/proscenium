import { Poster } from "./PosterGrid";
import type { Series } from "../../types";

interface SeriesCardProps {
  series: Series;
  onActivate: (series: Series) => void;
}

/** Poster and show title (spec §5.4). Click opens the detail view. */
export default function SeriesCard({ series, onActivate }: SeriesCardProps) {
  return (
    <button
      onClick={() => onActivate(series)}
      data-testid="series-card"
      title={series.name}
      className="group block w-full text-left"
    >
      <Poster url={series.posterUrl} title={series.name} />
      <p className="mt-2 truncate text-sm text-zinc-200 group-hover:text-white">
        {series.name}
      </p>
      <p className="mt-0.5 h-4 text-xs text-zinc-500">
        {series.releaseYear ?? ""}
      </p>
    </button>
  );
}
