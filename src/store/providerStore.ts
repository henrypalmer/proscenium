import { create } from "zustand";
import * as api from "../lib/tauri";
import type { Provider, ProviderInput } from "../types";

interface ProviderState {
  providers: Provider[];
  loaded: boolean;
  error: string | null;
  load: () => Promise<void>;
  save: (input: ProviderInput) => Promise<Provider>;
  remove: (id: string) => Promise<void>;
}

export const useProviderStore = create<ProviderState>((set, get) => ({
  providers: [],
  loaded: false,
  error: null,

  load: async () => {
    try {
      const providers = await api.listProviders();
      set({ providers, loaded: true, error: null });
    } catch (e) {
      set({ loaded: true, error: String(e) });
    }
  },

  save: async (input) => {
    const saved = await api.upsertProvider(input);
    const providers = get()
      .providers.filter((p) => p.id !== saved.id)
      .concat(saved)
      .sort((a, b) => a.createdAt - b.createdAt);
    set({ providers });
    return saved;
  },

  remove: async (id) => {
    await api.deleteProvider(id);
    set({ providers: get().providers.filter((p) => p.id !== id) });
  },
}));
