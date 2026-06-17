import { create } from "zustand";
import * as api from "../lib/tauri";
import type { ListContentType, ListSummary } from "../types";

/**
 * Custom lists for the active provider (spec §5.11). Centralized so the Home
 * "My Lists" row, the "Add to list" picker, and List Detail all stay in sync.
 * Mutations re-fetch the summaries so covers and counts update.
 */
interface ListsState {
  providerId: string | null;
  lists: ListSummary[];
  loaded: boolean;
  load: (providerId: string) => Promise<void>;
  refresh: () => Promise<void>;
  create: (name: string) => Promise<ListSummary | null>;
  rename: (listId: string, name: string) => Promise<void>;
  remove: (listId: string) => Promise<void>;
  addItem: (
    listId: string,
    contentType: ListContentType,
    contentId: string,
  ) => Promise<void>;
  removeItem: (
    listId: string,
    contentType: ListContentType,
    contentId: string,
  ) => Promise<void>;
}

export const useListsStore = create<ListsState>((set, get) => ({
  providerId: null,
  lists: [],
  loaded: false,

  load: async (providerId) => {
    try {
      const lists = await api.getLists(providerId);
      set({ providerId, lists, loaded: true });
    } catch {
      set({ providerId, lists: [], loaded: true });
    }
  },

  refresh: async () => {
    const { providerId } = get();
    if (providerId) await get().load(providerId);
  },

  create: async (name) => {
    const { providerId } = get();
    if (!providerId) return null;
    const created = await api.createList(providerId, name);
    await get().refresh();
    return get().lists.find((l) => l.id === created.id) ?? null;
  },

  rename: async (listId, name) => {
    await api.renameList(listId, name);
    await get().refresh();
  },

  remove: async (listId) => {
    await api.deleteList(listId);
    await get().refresh();
  },

  addItem: async (listId, contentType, contentId) => {
    await api.addToList(listId, contentType, contentId);
    await get().refresh();
  },

  removeItem: async (listId, contentType, contentId) => {
    await api.removeFromList(listId, contentType, contentId);
    await get().refresh();
  },
}));
