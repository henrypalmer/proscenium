import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import * as api from "../lib/tauri";
import { inTauri } from "../lib/tauri";
import { useCatalogStore } from "./catalogStore";
import { useProgressStore } from "./progressStore";
import type {
  MpvState,
  PlayableContentType,
  ProgressContentType,
  WatchProgress,
} from "../types";

// Spec §5.6: notice after 10s of buffering, §12: error state after 30s.
export const BUFFER_NOTICE_MS = 10_000;
export const BUFFER_ERROR_MS = 30_000;

// Spec §5.9: how often to persist position, and the minimum progress worth
// offering a resume prompt for (anything shorter just starts over silently).
const SAVE_INTERVAL_MS = 5_000;
const MIN_RESUME_SECONDS = 5;
const COMPLETION_THRESHOLD = 0.95;

export interface NowPlaying {
  title: string;
  providerId: string;
  contentType: PlayableContentType;
  contentId: string;
  streamUrl: string;
}

/** A pending resume choice (spec §5.9) shown before playback begins. */
export interface PendingResume {
  providerId: string;
  contentType: ProgressContentType;
  contentId: string;
  title: string;
  resumeSeconds: number;
}

interface OpenArgs {
  providerId: string;
  contentType: PlayableContentType;
  contentId: string;
  title: string;
}

interface PlayerStoreState {
  open: boolean;
  nowPlaying: NowPlaying | null;
  mpv: MpvState | null;
  /** Wall-clock ms when the current buffering stint began (null = not buffering). */
  bufferingSince: number | null;
  /** Fatal player error (stream failed or buffering exceeded the budget). */
  fatalError: string | null;
  /**
   * True once the current load has actually delivered frames. Until then
   * the overlay paints an opaque backdrop instead of revealing the (still
   * empty) native video surface.
   */
  everPlayed: boolean;
  /** Resume-vs-restart prompt awaiting the user (spec §5.9). */
  pendingResume: PendingResume | null;
  openContent: (args: OpenArgs) => Promise<void>;
  /**
   * Start playback immediately at `startAt` seconds, bypassing the resume
   * prompt — used by the detail-page "Resume from MM:SS" / "Start over" CTAs
   * (spec §5.9 / Milestone 26), where the user has already made the choice.
   */
  playDirect: (args: OpenArgs, startAt: number) => Promise<void>;
  /**
   * Play an arbitrary direct stream URL (a Stremio addon source, M41). No resume
   * prompt and no progress tracking — addon streams aren't provider content.
   */
  playUrl: (url: string, title: string, contentType: PlayableContentType) => Promise<void>;
  /** Proceed from a pending resume prompt. */
  resumePlayback: () => Promise<void>;
  startOver: () => Promise<void>;
  cancelResume: () => void;
  retry: () => Promise<void>;
  openExternal: () => Promise<void>;
  close: () => Promise<void>;
  applyMpvState: (state: MpvState) => void;
}

let listenerAttached = false;
let pollTimer: number | null = null;
/** Wall-clock ms of the last persisted position for the current item. */
let lastSaveAt = 0;

function startMockPolling(get: () => PlayerStoreState) {
  // The browser dev mock has no Tauri events; poll instead.
  if (inTauri || pollTimer !== null) return;
  pollTimer = window.setInterval(() => {
    if (!get().open) return;
    void api.mpv.getState().then((s) => get().applyMpvState(s));
  }, 400);
}

function localProgress(position: number, duration: number | null): WatchProgress {
  const positionSeconds = Math.max(0, Math.round(position));
  const durationSeconds = duration && duration > 0 ? Math.round(duration) : null;
  const completed =
    durationSeconds !== null &&
    positionSeconds / durationSeconds >= COMPLETION_THRESHOLD;
  return {
    positionSeconds,
    durationSeconds,
    completed,
    updatedAt: Math.floor(Date.now() / 1000),
  };
}

/** Persist the current position for a VOD item (no-op for live / position 0). */
async function persistProgress(
  np: NowPlaying,
  position: number,
  duration: number | null,
): Promise<void> {
  // Addon URL playback carries no provider/content id — nothing to persist.
  if (np.contentType === "live" || position <= 0 || !np.providerId) return;
  const contentType = np.contentType as ProgressContentType;
  try {
    await api.setWatchProgress(
      np.providerId,
      contentType,
      np.contentId,
      position,
      duration,
    );
    useProgressStore
      .getState()
      .setLocal(np.providerId, contentType, np.contentId, localProgress(position, duration));
  } catch {
    // Progress is best-effort; a failed write must not disrupt playback.
  }
}

export const usePlayerStore = create<PlayerStoreState>((set, get) => ({
  open: false,
  nowPlaying: null,
  mpv: null,
  bufferingSince: null,
  fatalError: null,
  everPlayed: false,
  pendingResume: null,

  openContent: async ({ providerId, contentType, contentId, title }) => {
    // Live TV is never tracked and never prompts (spec §5.9).
    if (contentType !== "live") {
      try {
        const progress = await api.getWatchProgress(
          providerId,
          contentType,
          contentId,
        );
        if (
          progress &&
          !progress.completed &&
          progress.positionSeconds >= MIN_RESUME_SECONDS
        ) {
          set({
            pendingResume: {
              providerId,
              contentType,
              contentId,
              title,
              resumeSeconds: progress.positionSeconds,
            },
          });
          return;
        }
      } catch {
        // Couldn't read progress — fall through and start from the beginning.
      }
    }
    await startPlayback(set, get, { providerId, contentType, contentId, title }, 0);
  },

  playDirect: async (args, startAt) => {
    await startPlayback(set, get, args, startAt);
  },

  playUrl: async (url, title, contentType) => {
    await startUrlPlayback(set, get, url, title, contentType);
  },

  resumePlayback: async () => {
    const pending = get().pendingResume;
    if (!pending) return;
    set({ pendingResume: null });
    await startPlayback(set, get, pending, pending.resumeSeconds);
  },

  startOver: async () => {
    const pending = get().pendingResume;
    if (!pending) return;
    set({ pendingResume: null });
    await startPlayback(set, get, pending, 0);
  },

  cancelResume: () => set({ pendingResume: null }),

  retry: async () => {
    const nowPlaying = get().nowPlaying;
    if (!nowPlaying?.streamUrl) return;
    set({ fatalError: null, bufferingSince: Date.now(), mpv: null, everPlayed: false });
    try {
      await api.mpv.loadUrl(nowPlaying.streamUrl);
    } catch (e) {
      set({ fatalError: String(e), bufferingSince: null });
    }
  },

  openExternal: async () => {
    const nowPlaying = get().nowPlaying;
    if (!nowPlaying?.streamUrl) return;
    try {
      await api.openInExternalPlayer(nowPlaying.streamUrl);
      await get().close();
    } catch (e) {
      useCatalogStore.getState().notify(String(e), "error");
    }
  },

  close: async () => {
    // Flush a final position before tearing down (spec §5.9).
    const { nowPlaying, mpv } = get();
    if (nowPlaying && mpv) {
      await persistProgress(nowPlaying, mpv.position, mpv.duration);
    }
    set({
      open: false,
      nowPlaying: null,
      mpv: null,
      bufferingSince: null,
      fatalError: null,
      everPlayed: false,
    });
    try {
      await api.mpv.stop();
    } catch {
      // Player may never have started; closing is still fine.
    }
  },

  applyMpvState: (state) => {
    if (!get().open) return;
    const previous = get();
    let bufferingSince = previous.bufferingSince;
    if (state.buffering) {
      bufferingSince = bufferingSince ?? Date.now();
    } else if (state.playing || state.error) {
      bufferingSince = null;
    }
    set({
      mpv: state,
      bufferingSince,
      fatalError: state.error ?? previous.fatalError,
      everPlayed:
        previous.everPlayed ||
        (state.playing && !state.buffering && state.position > 0),
    });

    // A newly-arrived stream error (spec §12, Milestone 22): upgrade the opaque
    // mpv message to a classified, user-facing reason (4xx/5xx/network/timeout)
    // and log a secret-redacted diagnostic line, in the background.
    if (state.error && !previous.fatalError && previous.nowPlaying?.providerId) {
      void refineStreamError(set, get, previous.nowPlaying, state.error);
    }

    // Throttled position persistence for VOD (spec §5.9).
    const np = previous.nowPlaying;
    if (np && np.contentType !== "live" && state.playing && state.position > 0) {
      const now = Date.now();
      if (now - lastSaveAt >= SAVE_INTERVAL_MS) {
        lastSaveAt = now;
        void persistProgress(np, state.position, state.duration);
      }
    }
  },
}));

/**
 * Replace an opaque mpv error with a classified, user-facing reason (spec §12,
 * Milestone 22). Runs in the background after the error first appears; only
 * applies if the same item is still open and still errored.
 */
async function refineStreamError(
  set: (partial: Partial<PlayerStoreState>) => void,
  get: () => PlayerStoreState,
  np: NowPlaying,
  rawError: string,
): Promise<void> {
  try {
    const message = await api.diagnosePlaybackFailure(
      np.providerId,
      np.contentType,
      np.contentId,
      rawError,
    );
    const cur = get();
    if (cur.open && cur.nowPlaying?.contentId === np.contentId && cur.fatalError) {
      set({ fatalError: message });
    }
  } catch {
    // Diagnosis is best-effort; keep the raw error if it fails.
  }
}

/** Shared playback launch used by openContent and the resume prompt. */
async function startPlayback(
  set: (partial: Partial<PlayerStoreState>) => void,
  get: () => PlayerStoreState,
  args: OpenArgs,
  startAt: number,
): Promise<void> {
  if (!listenerAttached && inTauri) {
    listenerAttached = true;
    await listen<MpvState>("mpv:state_changed", (event) => {
      get().applyMpvState(event.payload);
    });
  }
  startMockPolling(get);
  lastSaveAt = Date.now();
  try {
    const streamUrl = await api.resolveStreamUrl(
      args.providerId,
      args.contentType,
      args.contentId,
    );
    set({
      open: true,
      nowPlaying: {
        title: args.title,
        providerId: args.providerId,
        contentType: args.contentType,
        contentId: args.contentId,
        streamUrl,
      },
      mpv: null,
      fatalError: null,
      bufferingSince: Date.now(),
      everPlayed: false,
    });
    // Record a live channel as recently-watched (spec §13, Milestone 29).
    // Best-effort and local; never blocks playback.
    if (args.contentType === "live") {
      void api.recordRecentChannel(args.providerId, args.contentId).catch(() => {});
    }
    await api.mpv.loadUrl(streamUrl, startAt > 0 ? startAt : undefined);
  } catch (e) {
    set({
      open: true,
      nowPlaying: {
        title: args.title,
        providerId: args.providerId,
        contentType: args.contentType,
        contentId: args.contentId,
        streamUrl: "",
      },
      fatalError: String(e),
      bufferingSince: null,
      everPlayed: false,
    });
  }
}

/** Playback launch for an arbitrary direct URL (a Stremio addon source, M41).
 * Skips `resolveStreamUrl` (the URL is already direct) and tracks no progress. */
async function startUrlPlayback(
  set: (partial: Partial<PlayerStoreState>) => void,
  get: () => PlayerStoreState,
  url: string,
  title: string,
  contentType: PlayableContentType,
): Promise<void> {
  if (!listenerAttached && inTauri) {
    listenerAttached = true;
    await listen<MpvState>("mpv:state_changed", (event) => {
      get().applyMpvState(event.payload);
    });
  }
  startMockPolling(get);
  lastSaveAt = Date.now();
  const nowPlaying: NowPlaying = {
    title,
    providerId: "",
    contentType,
    contentId: "",
    streamUrl: url,
  };
  try {
    set({
      open: true,
      nowPlaying,
      mpv: null,
      fatalError: null,
      bufferingSince: Date.now(),
      everPlayed: false,
    });
    await api.mpv.loadUrl(url);
  } catch (e) {
    set({
      open: true,
      nowPlaying: { ...nowPlaying, streamUrl: "" },
      fatalError: String(e),
      bufferingSince: null,
      everPlayed: false,
    });
  }
}
