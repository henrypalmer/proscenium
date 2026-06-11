import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import * as api from "../lib/tauri";
import type {
  CatalogSummary,
  Provider,
  RefreshComplete,
  RefreshProgress,
} from "../types";

interface CatalogState {
  activeProvider: Provider | null;
  refreshing: boolean;
  stage: string | null;
  progress: number;
  summary: CatalogSummary | null;
  toast: string | null;
  /** Resolve the active provider, load cached counts, attach event listeners. */
  init: (providers: Provider[]) => Promise<void>;
  setActive: (providerId: string) => Promise<void>;
  refresh: () => Promise<void>;
  loadSummary: () => Promise<void>;
  handleProviderDeleted: (providerId: string) => Promise<void>;
  dismissToast: () => void;
}

let listenersAttached = false;

export const useCatalogStore = create<CatalogState>((set, get) => ({
  activeProvider: null,
  refreshing: false,
  stage: null,
  progress: 0,
  summary: null,
  toast: null,

  init: async (providers) => {
    if (!listenersAttached) {
      listenersAttached = true;
      await listen<RefreshProgress>("catalog:refresh_progress", (event) => {
        set({
          refreshing: true,
          stage: event.payload.stage,
          progress: event.payload.progress,
        });
      });
      await listen<RefreshComplete>("catalog:refresh_complete", (event) => {
        set({ refreshing: false, stage: null, progress: 0 });
        if (event.payload.success) {
          void get().loadSummary();
        } else {
          // Spec §5.2: non-blocking toast; the stale cache stays usable.
          set({
            toast: `Catalog refresh failed: ${event.payload.error ?? "unknown error"}`,
          });
        }
      });
    }

    let active = await api.getActiveProvider();
    // Older installs (or a deleted active provider) may have providers but
    // no active selection — default to the first profile.
    if (!active && providers.length > 0) {
      await api.setActiveProvider(providers[0].id);
      active = await api.getActiveProvider();
    }
    set({ activeProvider: active });
    await get().loadSummary();
  },

  setActive: async (providerId) => {
    await api.setActiveProvider(providerId);
    set({ activeProvider: await api.getActiveProvider() });
    await get().loadSummary();
  },

  refresh: async () => {
    const provider = get().activeProvider;
    if (!provider || get().refreshing) return;
    set({ refreshing: true, stage: "Starting…", progress: 0 });
    try {
      await api.refreshCatalog(provider.id);
    } catch {
      // The refresh_complete event already surfaced the error as a toast.
    } finally {
      set({ refreshing: false, stage: null });
    }
  },

  loadSummary: async () => {
    const provider = get().activeProvider;
    if (!provider) {
      set({ summary: null });
      return;
    }
    try {
      set({ summary: await api.getCatalogSummary(provider.id) });
    } catch {
      set({ summary: null });
    }
  },

  handleProviderDeleted: async (providerId) => {
    if (get().activeProvider?.id !== providerId) return;
    set({ activeProvider: null, summary: null });
    const active = await api.getActiveProvider();
    set({ activeProvider: active });
    if (active) await get().loadSummary();
  },

  dismissToast: () => set({ toast: null }),
}));
