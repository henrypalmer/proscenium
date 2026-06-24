import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import * as api from "../lib/tauri";
import { inTauri } from "../lib/tauri";
import { useProviderStore } from "./providerStore";
import type {
  CatalogSummary,
  Provider,
  ProviderStatus,
  RefreshComplete,
  RefreshProgress,
} from "../types";

export interface ToastMessage {
  message: string;
  kind: "error" | "info";
}

interface CatalogState {
  activeProvider: Provider | null;
  refreshing: boolean;
  stage: string | null;
  progress: number;
  summary: CatalogSummary | null;
  toast: ToastMessage | null;
  /** Active-provider health for the warning banner (spec §12); null = healthy. */
  providerStatus: ProviderStatus | null;
  /** Bumped after every successful refresh so views reload their data. */
  refreshTick: number;
  /** Resolve the active provider, load cached counts, attach event listeners. */
  init: (providers: Provider[]) => Promise<void>;
  setActive: (providerId: string) => Promise<void>;
  refresh: () => Promise<void>;
  /** Post-refresh fan-out: reload counts + the active provider's timestamp. */
  refreshSucceeded: () => Promise<void>;
  loadSummary: () => Promise<void>;
  handleProviderDeleted: (providerId: string) => Promise<void>;
  /** Re-probe the active provider (banner Retry button). */
  recheckProviderStatus: () => Promise<void>;
  dismissProviderStatus: () => void;
  notify: (message: string, kind?: ToastMessage["kind"]) => void;
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
  providerStatus: null,
  refreshTick: 0,

  init: async (providers) => {
    // Tauri events don't exist in the browser dev mock.
    if (!listenersAttached && inTauri) {
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
          void get().refreshSucceeded();
        } else {
          // Spec §5.2: non-blocking toast; the stale cache stays usable.
          set({
            toast: {
              message: `Catalog refresh failed: ${event.payload.error ?? "unknown error"}`,
              kind: "error",
            },
          });
        }
      });
      // Spec §12: the startup probe only pushes an event when the provider is
      // unreachable or expired.
      await listen<ProviderStatus>("provider:status", (event) => {
        set({ providerStatus: event.payload });
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
    // A different provider invalidates any banner from the previous one.
    set({ activeProvider: await api.getActiveProvider(), providerStatus: null });
    await get().loadSummary();
  },

  refresh: async () => {
    const provider = get().activeProvider;
    if (!provider || get().refreshing) return;
    set({ refreshing: true, stage: "Starting…", progress: 0 });
    try {
      await api.refreshCatalog(provider.id);
      // The browser dev mock emits no Tauri events, so drive the completion
      // (timestamp + toast) inline; in Tauri the `refresh_complete` event does it.
      if (!inTauri) await get().refreshSucceeded();
    } catch {
      // The refresh_complete event already surfaced the error as a toast.
    } finally {
      set({ refreshing: false, stage: null });
    }
  },

  // Milestone 24: after a successful refresh, bump the reload tick, refresh the
  // catalog counts, and re-read the active provider so its "Last refreshed"
  // timestamp updates on the Settings card; finish with a confirmation toast.
  refreshSucceeded: async () => {
    set({ refreshTick: get().refreshTick + 1 });
    await get().loadSummary();
    const active = await api.getActiveProvider();
    if (active) set({ activeProvider: active });
    await useProviderStore.getState().load();
    get().notify("Catalog updated.");
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

  recheckProviderStatus: async () => {
    const provider = get().activeProvider;
    if (!provider) return;
    try {
      const status = await api.checkProviderStatus(provider.id);
      // Clear the banner when the provider recovers; otherwise refresh it.
      set({
        providerStatus: status.reachable && !status.expired ? null : status,
      });
      // A recovered, never-refreshed (or stale) provider should refill its
      // catalog now that it's reachable again.
      if (status.reachable) await get().refresh();
    } catch {
      // Leave the existing banner in place on a transient failure.
    }
  },

  dismissProviderStatus: () => set({ providerStatus: null }),

  notify: (message, kind = "info") => set({ toast: { message, kind } }),

  dismissToast: () => set({ toast: null }),
}));
