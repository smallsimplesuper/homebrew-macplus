import { create } from "zustand";

interface UIState {
  selectedAppId: string | null;
  detailOpen: boolean;
  commandPaletteOpen: boolean;
  selectApp: (bundleId: string | null) => void;
  setDetailOpen: (open: boolean) => void;
  setCommandPaletteOpen: (open: boolean) => void;
}

export const useUIStore = create<UIState>((set) => ({
  selectedAppId: null,
  detailOpen: false,
  commandPaletteOpen: false,
  selectApp: (bundleId) => set({ selectedAppId: bundleId, detailOpen: !!bundleId }),
  setDetailOpen: (detailOpen) => set({ detailOpen, selectedAppId: detailOpen ? undefined : null }),
  setCommandPaletteOpen: (commandPaletteOpen) => set({ commandPaletteOpen }),
}));
