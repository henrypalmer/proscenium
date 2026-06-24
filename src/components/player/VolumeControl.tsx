import { mpv } from "../../lib/tauri";

interface VolumeControlProps {
  volume: number; // 0-100
  muted: boolean;
}

export default function VolumeControl({ volume, muted }: VolumeControlProps) {
  return (
    <div className="flex items-center gap-2">
      <button
        onClick={() => void mpv.setMute(!muted)}
        aria-label={muted ? "Unmute" : "Mute"}
        title={muted ? "Unmute (M)" : "Mute (M)"}
        className="rounded p-2 text-zinc-300 hover:bg-white/10 hover:text-white"
      >
        {muted || volume === 0 ? (
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" className="h-5 w-5">
            <path d="M11 5 6 9H2v6h4l5 4V5ZM22 9l-6 6M16 9l6 6" />
          </svg>
        ) : (
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" className="h-5 w-5">
            <path d="M11 5 6 9H2v6h4l5 4V5ZM15.5 8.5a5 5 0 0 1 0 7M19 5a9 9 0 0 1 0 14" />
          </svg>
        )}
      </button>
      <input
        type="range"
        min={0}
        max={100}
        value={muted ? 0 : Math.round(volume)}
        onChange={(e) => {
          const v = Number(e.target.value);
          void mpv.setVolume(v);
          if (muted && v > 0) void mpv.setMute(false);
        }}
        aria-label="Volume"
        className="h-1 w-24 cursor-pointer accent-zinc-200"
      />
    </div>
  );
}
