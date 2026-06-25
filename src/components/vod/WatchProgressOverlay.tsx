import type { WatchProgress } from "../../types";

interface WatchProgressOverlayProps {
  progress: WatchProgress | undefined;
  /** Render the corner watched checkmark (off for episode rows, §5.9). */
  showCheck?: boolean;
}

/**
 * Watched-state overlay for a movie/episode thumbnail (spec §5.9): a thin
 * progress bar pinned to the bottom edge for in-progress items, or a corner
 * checkmark once completed. Renders inside a `relative` parent. Nothing is
 * drawn for unwatched items.
 */
export default function WatchProgressOverlay({
  progress,
  showCheck = true,
}: WatchProgressOverlayProps) {
  if (!progress) return null;

  if (progress.completed) {
    if (!showCheck) return null;
    return (
      <div
        data-testid="watched-check"
        title="Watched — you've finished this"
        aria-label="Watched"
        className="absolute right-1.5 top-1.5 flex h-5 w-5 items-center justify-center rounded-full bg-emerald-500 text-[11px] font-bold text-white shadow"
      >
        ✓
      </div>
    );
  }

  // In-progress: proportional bar only when the runtime is known.
  if (progress.durationSeconds && progress.durationSeconds > 0) {
    const fraction = Math.min(
      1,
      progress.positionSeconds / progress.durationSeconds,
    );
    return (
      <div
        data-testid="progress-bar"
        className="absolute inset-x-0 bottom-0 h-1 bg-black/50"
      >
        <div
          className="h-full bg-emerald-500"
          style={{ width: `${fraction * 100}%` }}
        />
      </div>
    );
  }

  return null;
}
