import { useEffect } from "react";
import { formatTimestamp } from "../../lib/utils";
import type { Episode, Series } from "../../types";

interface ContinueWatchingSeriesDialogProps {
  series: Series;
  /** The last in-progress episode for the series (the card's item). */
  episode: Episode;
  resumeSeconds: number;
  onResume: () => void;
  onGoToSeries: () => void;
  onClose: () => void;
}

/**
 * Choice popup for a series card in the Home "Keep Watching" row (spec §5.10):
 * resume the last in-progress episode (via the standard §5.9 resume flow) or
 * navigate to the series detail page. Dismissible via click-away / Esc. Shown
 * only for series content — movie cards resume directly with no popup.
 */
export default function ContinueWatchingSeriesDialog({
  series,
  episode,
  resumeSeconds,
  onResume,
  onGoToSeries,
  onClose,
}: ContinueWatchingSeriesDialogProps) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      } else if (e.key === "Enter") {
        e.preventDefault();
        onResume();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose, onResume]);

  const epLabel = `S${episode.season}E${episode.episode}`;

  return (
    <div
      data-testid="continue-watching-series-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-6"
      onClick={onClose}
    >
      <div
        className="w-full max-w-sm rounded-xl border border-zinc-800 bg-zinc-900 p-6 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="truncate text-lg font-semibold text-white" title={series.name}>
          {series.name}
        </h2>
        <p className="mt-1 truncate text-sm text-zinc-400">
          {epLabel} · {episode.title}
        </p>
        <div className="mt-5 flex flex-col gap-2">
          <button
            autoFocus
            onClick={onResume}
            data-testid="series-dialog-resume"
            className="rounded-md bg-zinc-100 px-4 py-2 text-sm font-semibold text-zinc-900 hover:bg-white"
          >
            ▶ Resume {epLabel} ({formatTimestamp(resumeSeconds)})
          </button>
          <button
            onClick={onGoToSeries}
            data-testid="series-dialog-go-to-series"
            className="rounded-md border border-zinc-700 px-4 py-2 text-sm font-medium text-zinc-200 hover:bg-zinc-800"
          >
            Go to series
          </button>
        </div>
      </div>
    </div>
  );
}
