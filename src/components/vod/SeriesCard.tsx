import { Poster } from "./PosterGrid";
import type { Series } from "../../types";

interface SeriesCardProps {
  series: Series;
  onActivate: (series: Series) => void;
  onContextMenu?: (series: Series, x: number, y: number) => void;
}

/** Poster and show title (spec §5.4). Click opens the detail view. */
export default function SeriesCard({
  series,
  onActivate,
  onContextMenu,
}: SeriesCardProps) {
  return (
    <button
      onClick={() => onActivate(series)}
      onContextMenu={
        onContextMenu
          ? (e) => {
              e.preventDefault();
              onContextMenu(series, e.clientX, e.clientY);
            }
          : undefined
      }
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
