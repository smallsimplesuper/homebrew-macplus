import { create } from "zustand";

export interface UninstallTarget {
  bundleId: string;
  displayName: string;
  appPath: string;
  installSource: string;
  iconCachePath: string | null;
  installedVersion: string | null;
  homebrewCaskToken: string | null;
  homebrewFormulaName: string | null;
}

interface UIState {
  selectedAppId: string | null;
  detailOpen: boolean;
  commandPaletteOpen: boolean;
  uninstallTarget: UninstallTarget | null;
  selectApp: (bundleId: string | null) => void;
  setDetailOpen: (open: boolean) => void;
  setCommandPaletteOpen: (open: boolean) => void;
  setUninstallTarget: (target: UninstallTarget | null) => void;
}

export const useUIStore = create<UIState>((set) => ({
  selectedAppId: null,
  detailOpen: false,
  commandPaletteOpen: false,
  uninstallTarget: null,
  selectApp: (bundleId) => set({ selectedAppId: bundleId, detailOpen: !!bundleId }),
  setDetailOpen: (detailOpen) =>
    set(detailOpen ? { detailOpen } : { detailOpen, selectedAppId: null }),
  setCommandPaletteOpen: (commandPaletteOpen) => set({ commandPaletteOpen }),
  setUninstallTarget: (uninstallTarget) => set({ uninstallTarget }),
}));
