import { create } from "zustand";

/** Open/closed state of the global search overlay, shared by the Header
 * trigger and the Cmd/Ctrl+F shortcut (spec §5.5). */
interface SearchUiState {
  open: boolean;
  setOpen: (open: boolean) => void;
}

export const useSearchStore = create<SearchUiState>((set) => ({
  open: false,
  setOpen: (open) => set({ open }),
}));
