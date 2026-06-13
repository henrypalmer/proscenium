import { create } from "zustand";
import * as api from "../lib/tauri";
import type { ProgressContentType, WatchProgress } from "../types";

/**
 * Watch-progress cache (spec §5.9). Sections (movies / a series' episodes) are
 * bulk-loaded so cards and rows render their progress bar / watched checkmark
 * without a query per item. The player updates a single entry as it plays and
 * re-syncs the affected section when it closes.
 */

const key = (
  providerId: string,
  contentType: ProgressContentType,
  contentId: string,
) => `${providerId}|${contentType}|${contentId}`;

interface ProgressStoreState {
  /** Keyed by `${providerId}|${contentType}|${contentId}`. */
  entries: Record<string, WatchProgress>;
  /** Bulk-load a whole section into the cache. */
  loadSection: (
    providerId: string,
    contentType: ProgressContentType,
  ) => Promise<void>;
  /** Re-fetch one item from the backend (after playback). */
  syncOne: (
    providerId: string,
    contentType: ProgressContentType,
    contentId: string,
  ) => Promise<void>;
  /** Update one entry locally for instant UI (null clears it). */
  setLocal: (
    providerId: string,
    contentType: ProgressContentType,
    contentId: string,
    progress: WatchProgress | null,
  ) => void;
}

export const useProgressStore = create<ProgressStoreState>((set) => ({
  entries: {},

  loadSection: async (providerId, contentType) => {
    try {
      const map = await api.listWatchProgress(providerId, contentType);
      set((state) => {
        const entries = { ...state.entries };
        for (const [contentId, progress] of Object.entries(map)) {
          entries[key(providerId, contentType, contentId)] = progress;
        }
        return { entries };
      });
    } catch {
      // Progress is non-essential chrome; a failure just leaves cards bare.
    }
  },

  syncOne: async (providerId, contentType, contentId) => {
    try {
      const progress = await api.getWatchProgress(
        providerId,
        contentType,
        contentId,
      );
      set((state) => {
        const entries = { ...state.entries };
        const k = key(providerId, contentType, contentId);
        if (progress) entries[k] = progress;
        else delete entries[k];
        return { entries };
      });
    } catch {
      // ignore — see loadSection.
    }
  },

  setLocal: (providerId, contentType, contentId, progress) =>
    set((state) => {
      const entries = { ...state.entries };
      const k = key(providerId, contentType, contentId);
      if (progress) entries[k] = progress;
      else delete entries[k];
      return { entries };
    }),
}));

/** Selector hook: the cached progress for one item (or undefined). */
export function useWatchProgress(
  providerId: string,
  contentType: ProgressContentType,
  contentId: string,
): WatchProgress | undefined {
  return useProgressStore((s) => s.entries[key(providerId, contentType, contentId)]);
}
