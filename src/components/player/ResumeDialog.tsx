import { useEffect } from "react";
import { usePlayerStore } from "../../store/playerStore";
import { formatTimestamp } from "../../lib/utils";

/**
 * Pre-playback prompt for movies/episodes with saved progress (spec §5.9):
 * resume from the saved position or start from the beginning. Shown only when
 * `pendingResume` is set; the player itself opens after a choice is made.
 */
export default function ResumeDialog() {
  const pending = usePlayerStore((s) => s.pendingResume);
  const resumePlayback = usePlayerStore((s) => s.resumePlayback);
  const startOver = usePlayerStore((s) => s.startOver);
  const cancelResume = usePlayerStore((s) => s.cancelResume);

  useEffect(() => {
    if (!pending) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        cancelResume();
      } else if (e.key === "Enter") {
        e.preventDefault();
        void resumePlayback();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [pending, cancelResume, resumePlayback]);

  if (!pending) return null;

  return (
    <div
      data-testid="resume-dialog"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-6"
      onClick={cancelResume}
    >
      <div
        className="w-full max-w-sm rounded-xl border border-zinc-800 bg-zinc-900 p-6 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-lg font-semibold text-white">Resume playback?</h2>
        <p className="mt-1 truncate text-sm text-zinc-400" title={pending.title}>
          {pending.title}
        </p>
        <div className="mt-5 flex flex-col gap-2">
          <button
            autoFocus
            onClick={() => void resumePlayback()}
            data-testid="resume-continue"
            className="rounded-md bg-zinc-100 px-4 py-2 text-sm font-semibold text-zinc-900 hover:bg-white"
          >
            ▶ Resume from {formatTimestamp(pending.resumeSeconds)}
          </button>
          <button
            onClick={() => void startOver()}
            data-testid="resume-restart"
            className="rounded-md border border-zinc-700 px-4 py-2 text-sm font-medium text-zinc-200 hover:bg-zinc-800"
          >
            Start from beginning
          </button>
        </div>
      </div>
    </div>
  );
}
