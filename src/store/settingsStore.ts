import { create } from "zustand";
import * as api from "../lib/tauri";
import type { AppSettings } from "../types";

interface SettingsState {
  settings: AppSettings | null;
  loaded: boolean;
  load: () => Promise<void>;
  /** Persist one key and optimistically update local state. */
  update: <K extends keyof AppSettings>(
    key: K,
    value: AppSettings[K],
  ) => Promise<void>;
}

/** Map a camelCase settings field to its snake_case storage key (spec §15). */
function storageKey(key: keyof AppSettings): string {
  return key.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`);
}

function toStored(value: AppSettings[keyof AppSettings]): string {
  if (typeof value === "boolean") return value ? "true" : "false";
  return String(value ?? "");
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: null,
  loaded: false,

  load: async () => {
    try {
      const settings = await api.getSettings();
      set({ settings, loaded: true });
    } catch {
      set({ loaded: true });
    }
  },

  update: async (key, value) => {
    const current = get().settings;
    if (!current) return;
    // Optimistic: reflect the change immediately, then persist.
    set({ settings: { ...current, [key]: value } });
    await api.setSetting(storageKey(key), toStored(value));
  },
}));
