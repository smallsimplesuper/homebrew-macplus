import { useVirtualizer } from "@tanstack/react-virtual";
import { ArrowUpDown, ChevronDown, PackageOpen, Search } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useApps, useToggleIgnored } from "@/hooks/useApps";
import { useExecuteBulkUpdate } from "@/hooks/useUpdateExecution";
import { cn } from "@/lib/utils";
import { type SortField, useAppFilterStore } from "@/stores/appFilterStore";
import { useSelectionStore } from "@/stores/selectionStore";
import type { AppSummary } from "@/types/app";
import { AppRow } from "./AppRow";
import { AppListSkeleton } from "./AppRowSkeleton";
import { BulkActionBar } from "./BulkActionBar";

const sortOptions: { value: SortField; label: string }[] = [
  { value: "name", label: "Name" },
  { value: "source", label: "Source" },
  { value: "status", label: "Status" },
];

function filterApps(apps: AppSummary[], search: string, filterView: string): AppSummary[] {
  let filtered = apps;

  if (filterView === "updates") {
    filtered = filtered.filter((a) => a.hasUpdate);
  } else if (filterView === "ignored") {
    filtered = filtered.filter((a) => a.isIgnored);
  }

  if (search.trim()) {
    const q = search.toLowerCase().trim();
    filtered = filtered.filter(
      (a) =>
        a.displayName.toLowerCase().includes(q) ||
        a.bundleId.toLowerCase().includes(q) ||
        a.installSource.toLowerCase().includes(q),
    );
  }

  return filtered;
}

function sortApps(apps: AppSummary[], field: string, direction: string): AppSummary[] {
  const sorted = [...apps];
  const dir = direction === "asc" ? 1 : -1;

  sorted.sort((a, b) => {
    switch (field) {
      case "name":
        return dir * a.displayName.localeCompare(b.displayName);
      case "source":
        return dir * a.installSource.localeCompare(b.installSource);
      case "status": {
        const aVal = a.hasUpdate ? 0 : a.isIgnored ? 2 : 1;
        const bVal = b.hasUpdate ? 0 : b.isIgnored ? 2 : 1;
        return dir * (aVal - bVal);
      }
      default:
        return 0;
    }
  });

  return sorted;
}

export function AppListView() {
  const parentRef = useRef<HTMLDivElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const { data: apps = [], isLoading } = useApps();
  const {
    search,
    setSearch,
    sortField,
    setSortField,
    sortDirection,
    toggleSortDirection,
    filterView,
  } = useAppFilterStore();
  const { selectedIds, toggle, clearSelection } = useSelectionStore();
  const executeBulk = useExecuteBulkUpdate();
  const toggleIgnored = useToggleIgnored();
  const [sortOpen, setSortOpen] = useState(false);
  const sortRef = useRef<HTMLDivElement>(null);

  // Auto-focus search input when search is non-empty (e.g. triggered from toolbar)
  useEffect(() => {
    if (search && searchInputRef.current) {
      searchInputRef.current.focus();
    }
  }, [search]);

  // Close sort dropdown on outside click
  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      if (sortRef.current && !sortRef.current.contains(e.target as Node)) {
        setSortOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, []);

  const currentSort = sortOptions.find((o) => o.value === sortField);

  const processedApps = useMemo(() => {
    const filtered = filterApps(apps, search, filterView);
    return sortApps(filtered, sortField, sortDirection);
  }, [apps, search, filterView, sortField, sortDirection]);

  const virtualizer = useVirtualizer({
    count: processedApps.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 52,
    overscan: 8,
  });

  const handleUpdateSelected = useCallback(() => {
    const updatable = processedApps
      .filter((a) => selectedIds.has(a.bundleId) && a.hasUpdate)
      .map((a) => a.bundleId);
    if (updatable.length > 0) {
      executeBulk.mutate(updatable);
    }
  }, [processedApps, selectedIds, executeBulk]);

  const handleUpdateAll = useCallback(() => {
    const updatable = processedApps
      .filter((a) => a.hasUpdate && !a.isIgnored)
      .map((a) => a.bundleId);
    if (updatable.length > 0) {
      executeBulk.mutate(updatable);
    }
  }, [processedApps, executeBulk]);

  const handleIgnoreSelected = useCallback(() => {
    for (const id of selectedIds) {
      toggleIgnored.mutate({ bundleId: id, ignored: true });
    }
    clearSelection();
  }, [selectedIds, toggleIgnored, clearSelection]);

  const handleUnignoreSelected = useCallback(() => {
    for (const id of selectedIds) {
      toggleIgnored.mutate({ bundleId: id, ignored: false });
    }
    clearSelection();
  }, [selectedIds, toggleIgnored, clearSelection]);

  const selectedCount = selectedIds.size;

  return (
    <div className="flex h-full flex-col">
      {/* Inline header */}
      <div className="flex items-center justify-between p-4 pb-2">
        <h1 className="text-title text-foreground">
          {filterView === "ignored" ? "Ignored Apps" : "All Apps"}
        </h1>
        <div className="flex items-center gap-2">
          {/* Inline search input */}
          <div className="relative">
            <Search className="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
            <input
              ref={searchInputRef}
              type="text"
              placeholder="Search apps..."
              aria-label="Search applications"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="h-8 w-[200px] rounded-lg border border-input bg-background pl-8 pr-3 text-sm outline-none ring-ring placeholder:text-muted-foreground focus-visible:ring-1"
            />
          </div>
          {/* Sort dropdown */}
          <div ref={sortRef} className="relative">
            <button
              type="button"
              onClick={() => setSortOpen(!sortOpen)}
              className={cn(
                "flex h-8 items-center gap-1.5 rounded-md border border-input bg-background px-2.5",
                "text-xs text-muted-foreground hover:bg-accent hover:text-accent-foreground",
              )}
            >
              {currentSort?.label ?? "Sort"}
              <ChevronDown className="size-3" />
            </button>
            {sortOpen && (
              <div className="absolute right-0 top-full z-50 mt-1 min-w-[120px] rounded-md border border-border bg-popover p-1 shadow-md">
                {sortOptions.map((opt) => (
                  <button
                    key={opt.value}
                    type="button"
                    onClick={() => {
                      setSortField(opt.value);
                      setSortOpen(false);
                    }}
                    className={cn(
                      "flex w-full items-center rounded-sm px-2 py-1.5 text-xs",
                      sortField === opt.value
                        ? "bg-accent text-accent-foreground font-medium"
                        : "text-popover-foreground hover:bg-accent/50",
                    )}
                  >
                    {opt.label}
                  </button>
                ))}
              </div>
            )}
          </div>
          {/* Sort direction toggle */}
          <button
            type="button"
            onClick={toggleSortDirection}
            className="flex h-8 w-8 items-center justify-center rounded-md border border-input bg-background text-muted-foreground hover:bg-accent hover:text-accent-foreground"
            title={`Sort ${sortDirection === "asc" ? "ascending" : "descending"}`}
          >
            <ArrowUpDown
              className={cn(
                "size-3.5 transition-transform",
                sortDirection === "desc" && "rotate-180",
              )}
            />
          </button>
        </div>
      </div>

      <div ref={parentRef} className="flex-1 overflow-y-auto px-3">
        {isLoading ? (
          <AppListSkeleton />
        ) : processedApps.length === 0 ? (
          <div className="flex h-40 flex-col items-center justify-center gap-2 text-muted-foreground">
            <PackageOpen className="size-10 opacity-40" />
            <span className="text-sm">No apps found</span>
          </div>
        ) : (
          <div
            style={{
              height: virtualizer.getTotalSize(),
              width: "100%",
              position: "relative",
            }}
          >
            {virtualizer.getVirtualItems().map((virtualRow) => {
              const app = processedApps[virtualRow.index];
              return (
                <div
                  key={app.bundleId}
                  style={{
                    position: "absolute",
                    top: 0,
                    left: 0,
                    width: "100%",
                    height: virtualRow.size,
                    transform: `translateY(${virtualRow.start}px)`,
                    paddingTop: 4,
                    paddingBottom: 4,
                  }}
                >
                  <AppRow
                    app={app}
                    isSelected={selectedIds.has(app.bundleId)}
                    onClick={() => toggle(app.bundleId)}
                  />
                </div>
              );
            })}
          </div>
        )}
      </div>

      {selectedCount > 0 && (
        <div className="flex justify-center pb-3 pt-1">
          <BulkActionBar
            selectedCount={selectedCount}
            filterView={filterView}
            onUpdateSelected={handleUpdateSelected}
            onUpdateAll={handleUpdateAll}
            onIgnoreSelected={handleIgnoreSelected}
            onUnignoreSelected={handleUnignoreSelected}
            onClearSelection={clearSelection}
          />
        </div>
      )}
    </div>
  );
}
