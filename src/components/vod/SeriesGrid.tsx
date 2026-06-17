import { usePagedSeries } from "../../hooks/useCatalog";
import PosterGrid from "./PosterGrid";
import SeriesCard from "./SeriesCard";
import type { Series } from "../../types";

interface SeriesGridProps {
  providerId: string;
  categoryId: string | null;
  version: number;
  onActivate: (series: Series) => void;
  onContextMenu?: (series: Series, x: number, y: number) => void;
  /** Id of the card whose poster should morph into the detail view, if any. */
  morphId?: string | null;
}

/** Virtualized grid of `SeriesCard` items (spec §18). */
export default function SeriesGrid({
  providerId,
  categoryId,
  version,
  onActivate,
  onContextMenu,
  morphId,
}: SeriesGridProps) {
  const { total, getItem, ensureRange } = usePagedSeries(
    providerId,
    categoryId,
    version,
  );
  return (
    <PosterGrid<Series>
      total={total}
      getItem={getItem}
      ensureRange={ensureRange}
      resetKey={`${categoryId ?? "all"}:${version}`}
      emptyNoun="series"
      filtered={categoryId !== null}
      renderCard={(series) => (
        <SeriesCard
          series={series}
          onActivate={onActivate}
          onContextMenu={onContextMenu}
          morphActive={morphId === series.id}
        />
      )}
    />
  );
}
