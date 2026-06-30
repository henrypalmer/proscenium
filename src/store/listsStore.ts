import { create } from "zustand";
import * as api from "../lib/tauri";
import type { ListContentType, ListSummary } from "../types";

/**
 * Custom lists (spec §5.11). Global since Milestone 39 (not provider-scoped):
 * a list can mix items from several providers, so membership mutations carry the
 * item's `providerId`. Centralized so the Home "My Lists" row, the "Add to list"
 * picker, and List Detail stay in sync; mutations re-fetch the summaries.
 */
interface ListsState {
  lists: ListSummary[];
  loaded: boolean;
  load: () => Promise<void>;
  create: (name: string) => Promise<ListSummary | null>;
  rename: (listId: string, name: string) => Promise<void>;
  remove: (listId: string) => Promise<void>;
  addItem: (
    listId: string,
    providerId: string,
    contentType: ListContentType,
    contentId: string,
  ) => Promise<void>;
  removeItem: (
    listId: string,
    providerId: string,
    contentType: ListContentType,
    contentId: string,
  ) => Promise<void>;
}

export const useListsStore = create<ListsState>((set, get) => ({
  lists: [],
  loaded: false,

  load: async () => {
    try {
      const lists = await api.getLists();
      set({ lists, loaded: true });
    } catch {
      set({ lists: [], loaded: true });
    }
  },

  create: async (name) => {
    const created = await api.createList(name);
    await get().load();
    return get().lists.find((l) => l.id === created.id) ?? null;
  },

  rename: async (listId, name) => {
    await api.renameList(listId, name);
    await get().load();
  },

  remove: async (listId) => {
    await api.deleteList(listId);
    await get().load();
  },

  addItem: async (listId, providerId, contentType, contentId) => {
    await api.addToList(listId, providerId, contentType, contentId);
    await get().load();
  },

  removeItem: async (listId, providerId, contentType, contentId) => {
    await api.removeFromList(listId, providerId, contentType, contentId);
    await get().load();
  },
}));
