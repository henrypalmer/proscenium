import { useEffect, useState } from "react";
import {
  BUFFER_ERROR_MS,
  BUFFER_NOTICE_MS,
  usePlayerStore,
} from "../../store/playerStore";

/**
 * Buffering and error presentation (spec §5.6/§12): spinner while loading,
 * a notice after 10s, and a full error state with Retry / Open in External
 * Player after 30s or on stream failure.
 */
export default function BufferingOverlay() {
  const bufferingSince = usePlayerStore((s) => s.bufferingSince);
  const fatalError = usePlayerStore((s) => s.fatalError);
  const retry = usePlayerStore((s) => s.retry);
  const openExternal = usePlayerStore((s) => s.openExternal);

  // Re-render on a timer so the 10s/30s thresholds trip without new events.
  const [, setTick] = useState(0);
  useEffect(() => {
    if (bufferingSince === null) return;
    const timer = window.setInterval(() => setTick((t) => t + 1), 1000);
    return () => window.clearInterval(timer);
  }, [bufferingSince]);

  const bufferingMs =
    bufferingSince === null ? 0 : Date.now() - bufferingSince;
  const timedOut = bufferingSince !== null && bufferingMs >= BUFFER_ERROR_MS;
  const showError = Boolean(fatalError) || timedOut;

  if (showError) {
    return (
      <div
        data-testid="player-error"
        className="absolute inset-0 z-20 flex flex-col items-center justify-center gap-3 bg-zinc-950/90 text-center"
      >
        <p className="text-sm font-medium text-red-300">
          {fatalError ?? "The stream could not be loaded."}
        </p>
        <div className="flex gap-2">
          <button
            onClick={() => void retry()}
            className="rounded-md bg-zinc-100 px-4 py-1.5 text-xs font-semibold text-zinc-900 hover:bg-white"
          >
            Retry
          </button>
          <button
            onClick={() => void openExternal()}
            className="rounded-md border border-zinc-600 px-4 py-1.5 text-xs font-medium text-zinc-200 hover:bg-zinc-800"
          >
            Open in External Player
          </button>
        </div>
      </div>
    );
  }

  if (bufferingSince === null) return null;

  return (
    <div
      data-testid="player-buffering"
      className="pointer-events-none absolute inset-0 z-20 flex flex-col items-center justify-center gap-3"
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="h-9 w-9 animate-spin text-zinc-200">
        <path d="M21 12a9 9 0 1 1-6.2-8.56" />
      </svg>
      {bufferingMs >= BUFFER_NOTICE_MS && (
        <p className="rounded-full bg-zinc-900/80 px-3 py-1 text-xs text-zinc-300">
          Stream is taking longer than expected to load.
        </p>
      )}
    </div>
  );
}
