import { useEffect, useRef, useState } from "react";
import { mpv } from "../../lib/tauri";
import type { TrackInfo } from "../../types";

interface TrackSelectorProps {
  kind: "audio" | "subtitle";
  tracks: TrackInfo[];
  activeId: number | null;
}

function trackLabel(track: TrackInfo): string {
  const parts = [track.title, track.lang, track.codec].filter(Boolean);
  return parts.length > 0 ? parts.join(" · ") : `Track ${track.id}`;
}

/** Dropdown for audio/subtitle track selection (spec §18). */
export default function TrackSelector({
  kind,
  tracks,
  activeId,
}: TrackSelectorProps) {
  const [openMenu, setOpenMenu] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!openMenu) return;
    const onDown = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpenMenu(false);
    };
    window.addEventListener("mousedown", onDown);
    return () => window.removeEventListener("mousedown", onDown);
  }, [openMenu]);

  const select = (id: number) => {
    if (kind === "audio") void mpv.setAudioTrack(id);
    else void mpv.setSubtitleTrack(id);
    setOpenMenu(false);
  };

  const disabled = tracks.length === 0;

  return (
    <div ref={ref} className="relative" data-testid={`track-selector-${kind}`}>
      <button
        onClick={() => setOpenMenu((v) => !v)}
        disabled={disabled}
        title={
          kind === "audio" ? "Audio track (A)" : "Subtitle track (S)"
        }
        className="rounded p-2 text-zinc-300 hover:bg-white/10 hover:text-white disabled:opacity-40"
      >
        {kind === "audio" ? (
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" className="h-5 w-5">
            <path d="M9 18V6l11-2v12M9 18a3 3 0 1 1-6 0 3 3 0 0 1 6 0Zm11-2a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z" />
          </svg>
        ) : (
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" className="h-5 w-5">
            <rect x="3" y="5" width="18" height="14" rx="2" />
            <path d="M7 15h4M13 15h4M7 11h2M11 11h6" />
          </svg>
        )}
      </button>
      {openMenu && (
        <div className="absolute bottom-full right-0 mb-2 min-w-44 rounded-md border border-zinc-700 bg-zinc-900 py-1 shadow-xl">
          {kind === "subtitle" && (
            <button
              onClick={() => select(-1)}
              className={`block w-full px-3 py-1.5 text-left text-sm hover:bg-zinc-800 ${
                activeId === null ? "text-white" : "text-zinc-400"
              }`}
            >
              Off
            </button>
          )}
          {tracks.map((track) => (
            <button
              key={track.id}
              onClick={() => select(track.id)}
              className={`block w-full px-3 py-1.5 text-left text-sm hover:bg-zinc-800 ${
                activeId === track.id ? "text-white" : "text-zinc-400"
              }`}
            >
              {trackLabel(track)}
              {activeId === track.id && " ✓"}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
