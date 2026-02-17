import { ArrowUpDown, ChevronDown, Search } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { cn } from "@/lib/utils";
import { type SortField, useAppFilterStore } from "@/stores/appFilterStore";

const sortOptions: { value: SortField; label: string }[] = [
  { value: "name", label: "Name" },
  { value: "source", label: "Source" },
  { value: "status", label: "Status" },
];

export function AppListHeader() {
  const { search, setSearch, sortField, setSortField, sortDirection, toggleSortDirection } =
    useAppFilterStore();

  const [sortOpen, setSortOpen] = useState(false);
  const sortRef = useRef<HTMLDivElement>(null);

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

  return (
    <div className="flex items-center gap-2 px-3 pb-2 pt-3">
      {/* Search */}
      <div className="relative flex-1">
        <Search className="pointer-events-none absolute left-2.5 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
        <input
          type="text"
          placeholder="Search apps..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="h-8 w-full rounded-md border border-input bg-background pl-8 pr-3 text-sm outline-none ring-ring placeholder:text-muted-foreground focus-visible:ring-1"
        />
      </div>

      {/* Sort popover */}
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
          className={cn("size-3.5 transition-transform", sortDirection === "desc" && "rotate-180")}
        />
      </button>
    </div>
  );
}
