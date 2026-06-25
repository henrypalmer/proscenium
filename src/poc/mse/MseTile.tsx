import { useEffect, useRef, useState } from "react";
import Hls from "hls.js";
import mpegts from "mpegts.js";

/** POC (Spike D): one MSE-backed `<video>` tile. Plays a stream URL via
 * mpegts.js (MPEG-TS) or hls.js (HLS) entirely inside the WebView — no native
 * window, so a multi-view grid is just N of these in CSS. */

export type TileKind = "ts" | "hls";

export interface TileDiag {
  state: "idle" | "loading" | "playing" | "error";
  error: string | null;
  width: number;
  height: number;
  /** Decoded frames per second, where the engine reports it. */
  fps: number | null;
  engine: string;
}

interface MseTileProps {
  src: string;
  kind: TileKind;
  label: string;
  /** This tile owns audio (others are muted). */
  active: boolean;
  onActivate: () => void;
  onClose: () => void;
  onDiag?: (diag: TileDiag) => void;
}

export default function MseTile({
  src,
  kind,
  label,
  active,
  onActivate,
  onClose,
  onDiag,
}: MseTileProps) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const [diag, setDiag] = useState<TileDiag>({
    state: "idle",
    error: null,
    width: 0,
    height: 0,
    fps: null,
    engine: kind === "hls" ? "hls.js" : "mpegts.js",
  });

  // Keep the latest diag callback without re-running the player effect.
  const diagCb = useRef(onDiag);
  diagCb.current = onDiag;
  const update = (patch: Partial<TileDiag>) => setDiag((d) => ({ ...d, ...patch }));
  // Report diagnostics to the parent *after* commit (not inside a setState
  // updater, which would update the parent while rendering this child).
  useEffect(() => {
    diagCb.current?.(diag);
  }, [diag]);

  // Build/tear down the engine when the source changes.
  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    let mpegtsPlayer: mpegts.Player | null = null;
    let hls: Hls | null = null;
    update({ state: "loading", error: null, engine: kind === "hls" ? "hls.js" : "mpegts.js" });

    try {
      if (kind === "hls") {
        if (video.canPlayType("application/vnd.apple.mpegurl")) {
          // Native HLS (Safari / WKWebView) — no JS engine needed.
          video.src = src;
          update({ engine: "native HLS" });
        } else if (Hls.isSupported()) {
          hls = new Hls({ enableWorker: true, lowLatencyMode: true });
          hls.loadSource(src);
          hls.attachMedia(video);
          hls.on(Hls.Events.ERROR, (_e, data) => {
            if (data.fatal) update({ state: "error", error: `${data.type}: ${data.details}` });
          });
        } else {
          update({ state: "error", error: "HLS not supported in this WebView" });
        }
      } else {
        if (mpegts.getFeatureList().mseLivePlayback) {
          mpegtsPlayer = mpegts.createPlayer(
            { type: "mse", isLive: true, url: src },
            { enableStashBuffer: false, liveBufferLatencyChasing: true },
          );
          mpegtsPlayer.attachMediaElement(video);
          mpegtsPlayer.on(mpegts.Events.ERROR, (type, detail) => {
            update({ state: "error", error: `${type}: ${detail}` });
          });
          mpegtsPlayer.load();
        } else {
          update({ state: "error", error: "MSE live playback unsupported in this WebView" });
        }
      }
      void video.play().catch(() => {
        /* autoplay may defer until muted/visible; not fatal */
      });
    } catch (e) {
      update({ state: "error", error: String(e) });
    }

    return () => {
      try {
        mpegtsPlayer?.destroy();
      } catch {
        /* ignore */
      }
      try {
        hls?.destroy();
      } catch {
        /* ignore */
      }
      video.removeAttribute("src");
      video.load();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [src, kind]);

  // Audio focus: only the active tile is unmuted.
  useEffect(() => {
    if (videoRef.current) videoRef.current.muted = !active;
  }, [active]);

  // Track playback diagnostics off the media element.
  const onPlaying = () => {
    const v = videoRef.current;
    update({
      state: "playing",
      width: v?.videoWidth ?? 0,
      height: v?.videoHeight ?? 0,
    });
  };

  return (
    <button
      type="button"
      onClick={onActivate}
      data-testid="mse-tile"
      className={`group relative aspect-video w-full overflow-hidden rounded-lg bg-black text-left ring-2 transition ${
        active ? "ring-emerald-500" : "ring-zinc-800 hover:ring-zinc-600"
      }`}
    >
      <video
        ref={videoRef}
        muted={!active}
        playsInline
        autoPlay
        onPlaying={onPlaying}
        onWaiting={() => update({ state: "loading" })}
        className="h-full w-full bg-black object-contain"
      />

      {/* Header bar: label + audio state + close. */}
      <div className="absolute inset-x-0 top-0 flex items-center gap-2 bg-gradient-to-b from-black/80 to-transparent px-2.5 py-1.5 text-xs">
        <span className="min-w-0 flex-1 truncate font-medium text-zinc-100">{label}</span>
        <span className={active ? "text-emerald-400" : "text-zinc-500"}>
          {active ? "🔊" : "🔇"}
        </span>
        <span
          role="button"
          aria-label="Close tile"
          onClick={(e) => {
            e.stopPropagation();
            onClose();
          }}
          className="rounded px-1 text-zinc-300 hover:bg-white/15 hover:text-white"
        >
          ✕
        </span>
      </div>

      {/* Diagnostics footer. */}
      <div className="absolute inset-x-0 bottom-0 flex items-center gap-2 bg-gradient-to-t from-black/80 to-transparent px-2.5 py-1 text-[11px] tabular-nums text-zinc-400">
        <span
          className={
            diag.state === "playing"
              ? "text-emerald-400"
              : diag.state === "error"
                ? "text-red-400"
                : "text-amber-400"
          }
        >
          ●
        </span>
        <span>{diag.engine}</span>
        {diag.width > 0 && (
          <span>
            {diag.width}×{diag.height}
          </span>
        )}
        {diag.error && <span className="truncate text-red-400">{diag.error}</span>}
      </div>
    </button>
  );
}
