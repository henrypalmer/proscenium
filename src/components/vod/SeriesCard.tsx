import { Poster } from "./PosterGrid";
import { useProviderBadge } from "../../lib/useProviderBadge";
import type { Series } from "../../types";

interface SeriesCardProps {
  series: Series;
  onActivate: (series: Series) => void;
  onContextMenu?: (series: Series, x: number, y: number) => void;
  /** When true, this card's poster carries the shared-element name so it morphs
   * into the detail view on open (and back on close). */
  morphActive?: boolean;
}

/** Poster and show title (spec §5.4). Click opens the detail view. */
export default function SeriesCard({
  series,
  onActivate,
  onContextMenu,
  morphActive,
}: SeriesCardProps) {
  const badge = useProviderBadge(series.providerId);
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
      className="group relative block w-full text-left transition-transform duration-200 ease-out hover:z-10 hover:scale-[1.04] active:scale-[0.98] motion-reduce:transition-none motion-reduce:hover:scale-100"
    >
      <Poster
        url={series.posterUrl}
        title={series.name}
        vtName={morphActive ? "vt-poster" : undefined}
      />
      <p className="mt-2 truncate text-sm text-zinc-200 group-hover:text-white">
        {series.name}
      </p>
      <p className="mt-0.5 flex h-4 items-center gap-1.5 text-xs text-zinc-500">
        <span>{series.releaseYear ?? ""}</span>
        {badge && (
          <span className="min-w-0 truncate rounded bg-zinc-800 px-1 text-[10px] text-zinc-400">
            {badge}
          </span>
        )}
      </p>
    </button>
  );
}
