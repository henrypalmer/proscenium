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
  /** The enabled providers whose catalogs are merged (Milestone 39). */
  enabledProviders: Provider[];
  /** Ids of `enabledProviders`, kept in sync — the merged read scope. */
  providerIds: string[];
  refreshing: boolean;
  stage: string | null;
  progress: number;
  summary: CatalogSummary | null;
  toast: ToastMessage | null;
  /** Active-provider health for the warning banner (spec §12); null = healthy. */
  providerStatus: ProviderStatus | null;
  /** Bumped after every successful refresh so views reload their data. */
  refreshTick: number;
  /** Resolve the enabled set, load cached counts, attach event listeners. */
  init: (providers: Provider[]) => Promise<void>;
  /** Replace the enabled-provider set. */
  setEnabled: (providerIds: string[]) => Promise<void>;
  /** Enable/disable one provider (keeping the rest). */
  toggleProvider: (providerId: string) => Promise<void>;
  refresh: () => Promise<void>;
  /** Post-refresh fan-out: reload counts + the providers' timestamps. */
  refreshSucceeded: () => Promise<void>;
  loadSummary: () => Promise<void>;
  handleProviderDeleted: (providerId: string) => Promise<void>;
  /** Re-probe the primary provider (banner Retry button). */
  recheckProviderStatus: () => Promise<void>;
  dismissProviderStatus: () => void;
  notify: (message: string, kind?: ToastMessage["kind"]) => void;
  dismissToast: () => void;
}

let listenersAttached = false;

function withIds(providers: Provider[]) {
  return { enabledProviders: providers, providerIds: providers.map((p) => p.id) };
}

export const useCatalogStore = create<CatalogState>((set, get) => ({
  enabledProviders: [],
  providerIds: [],
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
      // Spec §12: the startup probe only pushes an event when a provider is
      // unreachable or expired.
      await listen<ProviderStatus>("provider:status", (event) => {
        set({ providerStatus: event.payload });
      });
    }

    let enabled = await api.getEnabledProviders();
    // Older installs (or a deleted enabled provider) may have providers but no
    // enabled selection — default to the first profile (mirrors pre-M39).
    if (enabled.length === 0 && providers.length > 0) {
      await api.setEnabledProviders([providers[0].id]);
      enabled = await api.getEnabledProviders();
    }
    set(withIds(enabled));
    await get().loadSummary();
  },

  setEnabled: async (providerIds) => {
    await api.setEnabledProviders(providerIds);
    const enabled = await api.getEnabledProviders();
    // A changed set invalidates any banner from the previous primary.
    set({ ...withIds(enabled), providerStatus: null });
    await get().loadSummary();
  },

  toggleProvider: async (providerId) => {
    const current = get().providerIds;
    const next = current.includes(providerId)
      ? current.filter((id) => id !== providerId)
      : [...current, providerId];
    await get().setEnabled(next);
  },

  refresh: async () => {
    const ids = get().providerIds;
    if (ids.length === 0 || get().refreshing) return;
    set({ refreshing: true, stage: "Starting…", progress: 0 });
    try {
      // Refresh every enabled provider; in Tauri each emits its own
      // progress/complete events that the listeners above drive.
      for (const id of ids) await api.refreshCatalog(id);
      // The browser dev mock emits no Tauri events, so drive completion inline.
      if (!inTauri) await get().refreshSucceeded();
    } catch {
      // The refresh_complete event already surfaced the error as a toast.
    } finally {
      set({ refreshing: false, stage: null });
    }
  },

  // After a successful refresh, bump the reload tick, refresh the catalog counts,
  // and re-read the providers so their "Last refreshed" timestamps update.
  refreshSucceeded: async () => {
    set({ refreshTick: get().refreshTick + 1 });
    await get().loadSummary();
    await useProviderStore.getState().load();
    const enabled = await api.getEnabledProviders();
    set(withIds(enabled));
    get().notify("Catalog updated.");
  },

  loadSummary: async () => {
    const ids = get().providerIds;
    if (ids.length === 0) {
      set({ summary: null });
      return;
    }
    try {
      set({ summary: await api.getCatalogSummary(ids) });
    } catch {
      set({ summary: null });
    }
  },

  handleProviderDeleted: async (providerId) => {
    if (!get().providerIds.includes(providerId)) return;
    const enabled = await api.getEnabledProviders();
    set(withIds(enabled));
    await get().loadSummary();
  },

  recheckProviderStatus: async () => {
    const primary = get().providerIds[0];
    if (!primary) return;
    try {
      const status = await api.checkProviderStatus(primary);
      // Clear the banner when the provider recovers; otherwise refresh it.
      set({
        providerStatus: status.reachable && !status.expired ? null : status,
      });
      // A recovered, never-refreshed (or stale) provider should refill now.
      if (status.reachable) await get().refresh();
    } catch {
      // Leave the existing banner in place on a transient failure.
    }
  },

  dismissProviderStatus: () => set({ providerStatus: null }),

  notify: (message, kind = "info") => set({ toast: { message, kind } }),

  dismissToast: () => set({ toast: null }),
}));
