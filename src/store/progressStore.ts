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
  /** Bulk-load a whole section across the provider set into the cache. */
  loadSection: (
    providerIds: string[],
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

  loadSection: async (providerIds, contentType) => {
    if (providerIds.length === 0) return;
    try {
      // Backend keys each entry as "<providerId>:<contentId>" (Milestone 39).
      const map = await api.listWatchProgress(providerIds, contentType);
      set((state) => {
        const entries = { ...state.entries };
        for (const [pkey, progress] of Object.entries(map)) {
          const sep = pkey.indexOf(":");
          const providerId = pkey.slice(0, sep);
          const contentId = pkey.slice(sep + 1);
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
