import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import * as api from "../lib/tauri";
import { inTauri } from "../lib/tauri";
import type { MpvState, TileState } from "../types";

export type MvLayout = "grid" | "focus";

/** A channel that can become a tile. */
export interface MvChannel {
  providerId: string;
  contentId: string;
  title: string;
  logoUrl?: string | null;
}

/** A live tile in the multi-view grid. id 0 is the primary (single) player. */
export interface MvTile extends MvChannel {
  id: number;
  state: MpvState | null;
}

interface MultiViewState {
  active: boolean;
  tiles: MvTile[];
  layout: MvLayout;
  /** Focus layout: the id of the large/primary tile. */
  focusId: number;
  /** The tile id that currently has audio (exactly one). */
  activeAudio: number;
  /** Effective cap = min(4, provider max_connections). */
  cap: number;
  pickerOpen: boolean;
  adding: boolean;
  error: string | null;

  enter: (channel: MvChannel, initialState: MpvState | null) => Promise<void>;
  exit: () => Promise<void>;
  addChannel: (channel: MvChannel) => Promise<void>;
  removeTile: (id: number) => Promise<void>;
  setActiveAudio: (id: number) => Promise<void>;
  setVolume: (volume: number) => Promise<void>;
  setLayout: (layout: MvLayout) => void;
  promote: (id: number) => void;
  openPicker: () => void;
  closePicker: () => void;
  applyTileState: (tileId: number, state: MpvState) => void;
}

let listenersAttached = false;
function attachListeners(get: () => MultiViewState) {
  if (listenersAttached || !inTauri) return;
  listenersAttached = true;
  // Secondary tiles emit per-tile state; the primary (tile 0) still emits the
  // single-player event, so listen to both and route by id.
  void listen<TileState>("mpv:tile_state", (e) =>
    get().applyTileState(e.payload.tileId, e.payload.state),
  );
  void listen<MpvState>("mpv:state_changed", (e) =>
    get().applyTileState(0, e.payload),
  );
}

export const useMultiViewStore = create<MultiViewState>((set, get) => ({
  active: false,
  tiles: [],
  layout: "grid",
  focusId: 0,
  activeAudio: 0,
  cap: 4,
  pickerOpen: false,
  adding: false,
  error: null,

  enter: async (channel, initialState) => {
    attachListeners(get);
    set({
      active: true,
      tiles: [{ id: 0, ...channel, state: initialState }],
      layout: "grid",
      focusId: 0,
      activeAudio: 0,
      cap: 4,
      pickerOpen: false,
      adding: false,
      error: null,
    });
    try {
      const budget = await api.mv.getBudget(channel.providerId);
      set({ cap: budget.cap });
    } catch {
      // Budget is best-effort; fall back to the default cap of 4.
    }
  },

  exit: async () => {
    try {
      await api.mv.close();
    } catch {
      // Closing is best-effort; tear the UI down regardless.
    }
    set({ active: false, tiles: [], pickerOpen: false, error: null });
  },

  addChannel: async (channel) => {
    const { tiles, cap } = get();
    if (tiles.length >= cap) {
      set({ error: `Connection limit reached — ${tiles.length} of ${cap} streams in use.` });
      return;
    }
    set({ adding: true, error: null, pickerOpen: false });
    try {
      // The initial rect is a placeholder; the grid re-renders with the new tile
      // and immediately reports accurate rects via mv_set_rects.
      const id = await api.mv.addTile(channel.providerId, channel.contentId, 0, 0, 1, 1);
      set((s) => ({
        tiles: [...s.tiles, { id, ...channel, state: null }],
        adding: false,
      }));
    } catch (e) {
      set({ adding: false, error: String(e) });
    }
  },

  removeTile: async (id) => {
    if (id === 0) return; // the primary tile leaves only by exiting multi-view
    try {
      await api.mv.removeTile(id);
    } catch {
      // best-effort
    }
    set((s) => ({
      tiles: s.tiles.filter((t) => t.id !== id),
      activeAudio: s.activeAudio === id ? 0 : s.activeAudio,
      focusId: s.focusId === id ? 0 : s.focusId,
    }));
  },

  setActiveAudio: async (id) => {
    set({ activeAudio: id });
    try {
      await api.mv.setActiveAudio(id);
    } catch {
      // best-effort
    }
  },

  setVolume: async (volume) => {
    const id = get().activeAudio;
    try {
      await api.mv.setVolume(id, volume);
    } catch {
      // best-effort
    }
    set((s) => ({
      tiles: s.tiles.map((t) =>
        t.id === id && t.state ? { ...t, state: { ...t.state, volume } } : t,
      ),
    }));
  },

  setLayout: (layout) => set({ layout }),
  promote: (id) => set({ focusId: id, layout: "focus" }),
  openPicker: () => set({ pickerOpen: true, error: null }),
  closePicker: () => set({ pickerOpen: false }),

  applyTileState: (tileId, state) =>
    set((s) =>
      s.active
        ? { tiles: s.tiles.map((t) => (t.id === tileId ? { ...t, state } : t)) }
        : {},
    ),
}));
