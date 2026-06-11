import { useCatalogStore } from "../../store/catalogStore";

interface CatalogPlaceholderProps {
  section: "Live TV" | "Movies" | "TV Shows";
}

// Shows cached catalog counts until the browsing UI ships (Milestones 3/5).
export default function CatalogPlaceholder({
  section,
}: CatalogPlaceholderProps) {
  const summary = useCatalogStore((s) => s.summary);
  const refreshing = useCatalogStore((s) => s.refreshing);

  const count =
    summary === null
      ? null
      : section === "Live TV"
        ? summary.liveChannels
        : section === "Movies"
          ? summary.movies
          : summary.series;
  const noun =
    section === "Live TV" ? "channels" : section === "Movies" ? "movies" : "series";

  return (
    <div className="flex h-full flex-col items-center justify-center gap-2 text-center">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" className="h-10 w-10 text-zinc-700">
        <rect x="3" y="5" width="18" height="14" rx="2" />
        <path d="M10 9.5l4.5 2.5-4.5 2.5v-5Z" />
      </svg>
      {count !== null && count > 0 ? (
        <>
          <p className="text-sm font-medium text-zinc-300">
            {count.toLocaleString()} {noun} cached
          </p>
          <p className="max-w-xs text-xs text-zinc-600">
            Browsing for {section} arrives in the next milestone.
          </p>
        </>
      ) : (
        <>
          <p className="text-sm font-medium text-zinc-400">
            {refreshing ? "Refreshing catalog…" : "No catalog yet"}
          </p>
          <p className="max-w-xs text-xs text-zinc-600">
            {refreshing
              ? "Content stays browsable while the catalog updates."
              : `${section} content will appear here after the first catalog refresh.`}
          </p>
        </>
      )}
    </div>
  );
}
