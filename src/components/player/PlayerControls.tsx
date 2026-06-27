import { mpv } from "../../lib/tauri";
import VolumeControl from "./VolumeControl";
import TrackSelector from "./TrackSelector";
import type { MpvState } from "../../types";

function formatTime(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  return h > 0
    ? `${h}:${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}`
    : `${m}:${String(sec).padStart(2, "0")}`;
}

interface PlayerControlsProps {
  state: MpvState;
  title: string;
  isLive: boolean;
  onToggleFullscreen: () => void;
  onClose: () => void;
  /** Enter multi-view with this channel as the first tile (live only). */
  onMultiView?: () => void;
}

export default function PlayerControls({
  state,
  title,
  isLive,
  onToggleFullscreen,
  onClose,
  onMultiView,
}: PlayerControlsProps) {
  // Three distinct bar modes (Milestone 22): a seekable VOD, a genuine live
  // stream, or a VOD whose duration isn't known yet (loading or failed) — the
  // last must NOT masquerade as "● LIVE / 0:00".
  const seekable = !isLive && state.duration !== null;

  return (
    <div
      data-testid="player-controls"
      className="pointer-events-auto absolute inset-x-0 bottom-0 z-30 bg-gradient-to-t from-black/90 via-black/60 to-transparent px-6 pb-5 pt-12"
    >
      {seekable ? (
        <input
          type="range"
          min={0}
          max={state.duration ?? 0}
          step={0.5}
          value={Math.min(state.position, state.duration ?? 0)}
          onChange={(e) => void mpv.seek(Number(e.target.value))}
          aria-label="Seek"
          data-testid="seek-bar"
          className="mb-3 h-1.5 w-full cursor-pointer accent-zinc-100"
        />
      ) : isLive ? (
        <div
          data-testid="live-badge"
          className="mb-3 flex items-center gap-2 text-[11px] font-semibold uppercase tracking-wide text-red-400"
        >
          <span className="h-2 w-2 animate-pulse rounded-full bg-red-500" />
          Live
        </div>
      ) : (
        // VOD still resolving a duration (or failed) — neutral placeholder track.
        <div
          data-testid="seek-placeholder"
          aria-hidden="true"
          className="mb-3 h-1.5 w-full rounded-full bg-white/15"
        />
      )}

      <div className="flex items-center gap-3">
        <button
          onClick={() => void (state.paused ? mpv.play() : mpv.pause())}
          aria-label={state.paused ? "Play" : "Pause"}
          title={state.paused ? "Play (Space)" : "Pause (Space)"}
          data-testid="play-pause"
          className="rounded p-2 text-white hover:bg-white/10"
        >
          {state.paused ? (
            <svg viewBox="0 0 24 24" fill="currentColor" className="h-6 w-6">
              <path d="M8 5v14l11-7L8 5Z" />
            </svg>
          ) : (
            <svg viewBox="0 0 24 24" fill="currentColor" className="h-6 w-6">
              <path d="M7 5h4v14H7V5Zm6 0h4v14h-4V5Z" />
            </svg>
          )}
        </button>

        <span className="text-xs tabular-nums text-zinc-300">
          {seekable
            ? `${formatTime(state.position)} / ${formatTime(state.duration ?? 0)}`
            : ""}
          {/* Live and still-loading VOD show no counter: a running session-elapsed
              timer next to "● LIVE" was ambiguous (spec §13, QA §2). The "Live"
              badge alone conveys the live state. */}
        </span>

        <span className="min-w-0 flex-1 truncate text-center text-sm text-zinc-200">
          {title}
        </span>

        <VolumeControl volume={state.volume} muted={state.muted} />
        <TrackSelector
          kind="audio"
          tracks={state.audioTracks}
          activeId={state.activeAudioTrack}
        />
        <TrackSelector
          kind="subtitle"
          tracks={state.subtitleTracks}
          activeId={state.activeSubtitleTrack}
        />

        {onMultiView && (
          <button
            onClick={onMultiView}
            aria-label="Multi-view"
            title="Multi-view — watch several channels at once"
            data-testid="multiview-enter"
            className="rounded p-2 text-zinc-300 hover:bg-white/10 hover:text-white"
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" className="h-5 w-5">
              <rect x="3" y="3" width="8" height="8" rx="1" />
              <rect x="13" y="3" width="8" height="8" rx="1" />
              <rect x="3" y="13" width="8" height="8" rx="1" />
              <rect x="13" y="13" width="8" height="8" rx="1" />
            </svg>
          </button>
        )}
        <button
          onClick={onToggleFullscreen}
          aria-label="Toggle full screen"
          title="Full screen (F)"
          className="rounded p-2 text-zinc-300 hover:bg-white/10 hover:text-white"
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" className="h-5 w-5">
            <path d="M8 3H5a2 2 0 0 0-2 2v3m18 0V5a2 2 0 0 0-2-2h-3m0 18h3a2 2 0 0 0 2-2v-3M3 16v3a2 2 0 0 0 2 2h3" />
          </svg>
        </button>
        <button
          onClick={onClose}
          aria-label="Close player"
          title="Close (Esc)"
          data-testid="player-close"
          className="rounded p-2 text-zinc-300 hover:bg-white/10 hover:text-white"
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" className="h-5 w-5">
            <path d="M6 6l12 12M18 6L6 18" />
          </svg>
        </button>
      </div>
    </div>
  );
}
