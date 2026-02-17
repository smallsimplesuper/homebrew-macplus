import { create } from "zustand";

interface SelectionState {
  selectedIds: Set<string>;
  toggle: (bundleId: string) => void;
  selectAll: (bundleIds: string[]) => void;
  clearSelection: () => void;
  isSelected: (bundleId: string) => boolean;
}

export const useSelectionStore = create<SelectionState>((set, get) => ({
  selectedIds: new Set<string>(),
  toggle: (bundleId) =>
    set((s) => {
      const next = new Set(s.selectedIds);
      if (next.has(bundleId)) {
        next.delete(bundleId);
      } else {
        next.add(bundleId);
      }
      return { selectedIds: next };
    }),
  selectAll: (bundleIds) => set({ selectedIds: new Set(bundleIds) }),
  clearSelection: () => set({ selectedIds: new Set() }),
  isSelected: (bundleId) => get().selectedIds.has(bundleId),
}));
