import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import * as api from "../lib/tauri";
import { inTauri } from "../lib/tauri";
import { useCatalogStore } from "./catalogStore";
import type { MpvState, PlayableContentType } from "../types";

// Spec §5.6: notice after 10s of buffering, §12: error state after 30s.
export const BUFFER_NOTICE_MS = 10_000;
export const BUFFER_ERROR_MS = 30_000;

export interface NowPlaying {
  title: string;
  contentType: PlayableContentType;
  streamUrl: string;
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
  openContent: (args: {
    providerId: string;
    contentType: PlayableContentType;
    contentId: string;
    title: string;
  }) => Promise<void>;
  retry: () => Promise<void>;
  openExternal: () => Promise<void>;
  close: () => Promise<void>;
  applyMpvState: (state: MpvState) => void;
}

let listenerAttached = false;
let pollTimer: number | null = null;

function startMockPolling(get: () => PlayerStoreState) {
  // The browser dev mock has no Tauri events; poll instead.
  if (inTauri || pollTimer !== null) return;
  pollTimer = window.setInterval(() => {
    if (!get().open) return;
    void api.mpv.getState().then((s) => get().applyMpvState(s));
  }, 400);
}

export const usePlayerStore = create<PlayerStoreState>((set, get) => ({
  open: false,
  nowPlaying: null,
  mpv: null,
  bufferingSince: null,
  fatalError: null,
  everPlayed: false,

  openContent: async ({ providerId, contentType, contentId, title }) => {
    if (!listenerAttached && inTauri) {
      listenerAttached = true;
      await listen<MpvState>("mpv:state_changed", (event) => {
        get().applyMpvState(event.payload);
      });
    }
    startMockPolling(get);
    try {
      const streamUrl = await api.resolveStreamUrl(
        providerId,
        contentType,
        contentId,
      );
      set({
        open: true,
        nowPlaying: { title, contentType, streamUrl },
        mpv: null,
        fatalError: null,
        bufferingSince: Date.now(),
        everPlayed: false,
      });
      await api.mpv.loadUrl(streamUrl);
    } catch (e) {
      set({
        open: true,
        nowPlaying: { title, contentType, streamUrl: "" },
        fatalError: String(e),
        bufferingSince: null,
        everPlayed: false,
      });
    }
  },

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
  },
}));
