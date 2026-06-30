import { Poster } from "../vod/PosterGrid";
import type { AvailabilityInfo, CanonicalItem } from "../../types";

interface Props {
  item: CanonicalItem;
  onActivate: (item: CanonicalItem) => void;
  /** When true, this poster carries the shared-element name so it morphs into
   * the detail view on open (and back on close). */
  morphActive?: boolean;
  /** Availability badge (M42), when the opt-in background pass has resolved it. */
  availability?: AvailabilityInfo;
}

/**
 * Canonical (Cinemeta) browse card: poster, title, year. No provider badge — a
 * canonical title is provider-agnostic until its sources are resolved (M40). An
 * optional availability badge (M42) shows when the opt-in pass found sources.
 */
export default function CanonicalCard({
  item,
  onActivate,
  morphActive,
  availability,
}: Props) {
  const badge =
    availability && availability.sourceCount > 0 ? (
      <span
        data-testid="availability-badge"
        className="absolute right-1.5 top-1.5 rounded bg-emerald-600/90 px-1.5 py-0.5 text-[10px] font-semibold text-white shadow"
      >
        {availability.bestQuality === "2160p"
          ? "4K"
          : availability.bestQuality === "1080p"
            ? "HD"
            : "✓"}
      </span>
    ) : null;
  return (
    <button
      onClick={() => onActivate(item)}
      data-testid="canonical-card"
      title={item.name}
      className="group relative block w-full text-left transition-transform duration-200 ease-out hover:z-10 hover:scale-[1.04] active:scale-[0.98] motion-reduce:transition-none motion-reduce:hover:scale-100"
    >
      <Poster
        url={item.posterUrl}
        title={item.name}
        overlay={badge}
        vtName={morphActive ? "vt-poster" : undefined}
      />
      <p className="mt-2 truncate text-sm text-zinc-200 group-hover:text-white">
        {item.name}
      </p>
      <p className="mt-0.5 flex h-4 items-center text-xs text-zinc-500">
        {item.releaseYear ?? ""}
      </p>
    </button>
  );
}
