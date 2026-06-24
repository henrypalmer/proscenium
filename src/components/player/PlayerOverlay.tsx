import { useCallback, useEffect, useRef, useState } from "react";
import { inTauri, mpv } from "../../lib/tauri";
import { usePlayerStore } from "../../store/playerStore";
import BufferingOverlay from "./BufferingOverlay";
import PlayerControls from "./PlayerControls";
import type { TrackInfo } from "../../types";

// Longer grace before the control bar auto-hides (Milestone 22): the old 3s was
// easy to miss when reaching for volume / track / fullscreen targets.
const CONTROLS_HIDE_MS = 4500;

async function setFullscreen(on: boolean): Promise<void> {
  if (inTauri) {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().setFullscreen(on);
  } else if (on) {
    await document.documentElement.requestFullscreen().catch(() => {});
  } else if (document.fullscreenElement) {
    await document.exitFullscreen().catch(() => {});
  }
}

async function isFullscreen(): Promise<boolean> {
  if (inTauri) {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    return getCurrentWindow().isFullscreen();
  }
  return document.fullscreenElement !== null;
}

/** Next id in the track cycle; -1 represents "off" (subtitles only). */
function nextTrackId(
  tracks: TrackInfo[],
  activeId: number | null,
  includeOff: boolean,
): number | null {
  const ids = tracks.map((t) => t.id);
  if (includeOff) ids.push(-1);
  if (ids.length === 0) return null;
  const current = activeId ?? -1;
  const idx = ids.indexOf(current);
  return ids[(idx + 1) % ids.length];
}

/**
 * Full-screen player container (spec §18). In the Tauri shell the video is
 * rendered by mpv into a native window *behind* the WebView; everything
 * here paints on a transparent background so the video shows through,
 * with controls composited on top.
 */
export default function PlayerOverlay() {
  const open = usePlayerStore((s) => s.open);
  const nowPlaying = usePlayerStore((s) => s.nowPlaying);
  const state = usePlayerStore((s) => s.mpv);
  const everPlayed = usePlayerStore((s) => s.everPlayed);
  const fatalError = usePlayerStore((s) => s.fatalError);
  const close = usePlayerStore((s) => s.close);

  // Until the stream has delivered frames (and whenever it has failed) the
  // player area keeps a soft opaque backdrop instead of exposing the empty
  // native video surface.
  const showBackdrop = !everPlayed || Boolean(fatalError);

  const [controlsVisible, setControlsVisible] = useState(true);
  const hideTimer = useRef<number | null>(null);

  const pokeControls = useCallback(() => {
    setControlsVisible(true);
    if (hideTimer.current !== null) window.clearTimeout(hideTimer.current);
    hideTimer.current = window.setTimeout(
      () => setControlsVisible(false),
      CONTROLS_HIDE_MS,
    );
  }, []);

  // The page must stop painting an opaque background while the player is
  // open or the native video behind the WebView stays hidden.
  useEffect(() => {
    document.body.classList.toggle("player-open", open);
    if (open) pokeControls();
    return () => document.body.classList.remove("player-open");
  }, [open, pokeControls]);

  const handleClose = useCallback(async () => {
    if (await isFullscreen()) await setFullscreen(false);
    await close();
  }, [close]);

  const toggleFullscreen = useCallback(async () => {
    await setFullscreen(!(await isFullscreen()));
  }, []);

  // Keyboard shortcuts (spec §5.6).
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      const s = usePlayerStore.getState().mpv;
      switch (e.key) {
        case " ":
          e.preventDefault();
          if (s) void (s.paused ? mpv.play() : mpv.pause());
          break;
        case "ArrowRight":
          e.preventDefault();
          if (s?.duration != null) void mpv.seek(s.position + 10);
          break;
        case "ArrowLeft":
          e.preventDefault();
          if (s?.duration != null) void mpv.seek(Math.max(0, s.position - 10));
          break;
        case "ArrowUp":
          e.preventDefault();
          if (s) void mpv.setVolume(Math.min(100, s.volume + 5));
          break;
        case "ArrowDown":
          e.preventDefault();
          if (s) void mpv.setVolume(Math.max(0, s.volume - 5));
          break;
        case "m":
        case "M":
          if (s) void mpv.setMute(!s.muted);
          break;
        case "f":
        case "F":
          void toggleFullscreen();
          break;
        case "Escape":
          void handleClose();
          break;
        case "a":
        case "A": {
          const next = s && nextTrackId(s.audioTracks, s.activeAudioTrack, false);
          if (next != null) void mpv.setAudioTrack(next);
          break;
        }
        case "s":
        case "S": {
          const next =
            s && nextTrackId(s.subtitleTracks, s.activeSubtitleTrack, true);
          if (next != null) void mpv.setSubtitleTrack(next);
          break;
        }
      }
      pokeControls();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, handleClose, toggleFullscreen, pokeControls]);

  if (!open || !nowPlaying) return null;

  return (
    <div
      data-testid="player-overlay"
      data-backdrop={showBackdrop ? "on" : "off"}
      onMouseMove={pokeControls}
      onDoubleClick={() => void toggleFullscreen()}
      className={`fixed inset-0 z-40 transition-colors duration-500 ${
        showBackdrop ? "bg-zinc-900" : "bg-transparent"
      } ${controlsVisible ? "" : "cursor-none"}`}
    >
      {/* Browser dev mode has no native video behind the page. */}
      {!inTauri && (
        <div className="absolute inset-0 -z-10 flex items-center justify-center bg-gradient-to-br from-zinc-900 via-black to-zinc-900">
          <span className="select-none text-2xl font-semibold tracking-widest text-zinc-700">
            VIDEO (dev mock)
          </span>
        </div>
      )}

      <BufferingOverlay />

      {controlsVisible && state && (
        <PlayerControls
          state={state}
          title={nowPlaying.title}
          isLive={nowPlaying.contentType === "live"}
          onToggleFullscreen={() => void toggleFullscreen()}
          onClose={() => void handleClose()}
        />
      )}
    </div>
  );
}
