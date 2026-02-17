import { create } from "zustand";

export type SortField = "name" | "source" | "status";
export type SortDirection = "asc" | "desc";
export type FilterView = "all" | "updates" | "ignored" | "history" | "settings";

interface AppFilterState {
  search: string;
  sortField: SortField;
  sortDirection: SortDirection;
  filterView: FilterView;
  setSearch: (search: string) => void;
  setSortField: (field: SortField) => void;
  toggleSortDirection: () => void;
  setFilterView: (view: FilterView) => void;
}

export const useAppFilterStore = create<AppFilterState>((set) => ({
  search: "",
  sortField: "name",
  sortDirection: "asc",
  filterView: "updates",
  setSearch: (search) => set({ search }),
  setSortField: (sortField) => set({ sortField }),
  toggleSortDirection: () =>
    set((s) => ({ sortDirection: s.sortDirection === "asc" ? "desc" : "asc" })),
  setFilterView: (filterView) => set({ filterView }),
}));
