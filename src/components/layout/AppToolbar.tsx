import { getCurrentWindow } from "@tauri-apps/api/window";
import { ArrowDownCircle, Clock, EyeOff, LayoutGrid, Search, Settings, X } from "lucide-react";
import { motion } from "motion/react";
import MacPlusLogo from "@/components/shared/MacPlusLogo";
import { springs } from "@/lib/animations";
import { cn } from "@/lib/utils";
import { type FilterView, useAppFilterStore } from "@/stores/appFilterStore";

interface NavItem {
  icon: typeof ArrowDownCircle;
  view: FilterView;
  label: string;
}

const NAV_ITEMS: NavItem[] = [
  { icon: ArrowDownCircle, view: "updates", label: "Updates" },
  { icon: LayoutGrid, view: "all", label: "All Apps" },
  { icon: EyeOff, view: "ignored", label: "Ignored" },
];

interface AppToolbarProps {
  updateCount?: number;
  className?: string;
}

export default function AppToolbar({ updateCount = 0, className }: AppToolbarProps) {
  const { search, setSearch, filterView, setFilterView } = useAppFilterStore();

  const activeNavView = NAV_ITEMS.find((n) => n.view === filterView);

  return (
    <div
      className={cn(
        "flex h-[44px] shrink-0 items-center bg-background/80 backdrop-blur-xl",
        className,
      )}
    >
      {/* Brand */}
      <div
        className="flex items-center gap-1.5 pl-4 self-stretch select-none"
        data-tauri-drag-region
      >
        <MacPlusLogo size={18} />
        <span className="text-[13px] font-semibold tracking-tight text-foreground">macPlus</span>
      </div>

      {/* Nav icons */}
      <div className="ml-3 flex items-center gap-0.5">
        {NAV_ITEMS.map((item) => {
          const isActive = activeNavView?.view === item.view;
          const Icon = item.icon;
          return (
            <motion.button
              key={item.view}
              whileTap={{ scale: 0.9 }}
              transition={springs.micro}
              onClick={() => setFilterView(item.view)}
              className={cn(
                "relative flex h-7 w-7 items-center justify-center rounded-md transition-colors",
                isActive
                  ? "text-foreground"
                  : "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
              )}
              aria-label={item.label}
            >
              <Icon className="h-3.5 w-3.5" />
              {/* Update badge */}
              {item.view === "updates" && updateCount > 0 && (
                <span className="absolute -right-0.5 -top-0.5 flex h-3.5 min-w-3.5 items-center justify-center rounded-full bg-primary px-0.5 text-[8px] font-bold leading-none text-primary-foreground">
                  {updateCount > 99 ? "99+" : updateCount}
                </span>
              )}
              {/* Active indicator dot */}
              {isActive && (
                <motion.div
                  layoutId="toolbar-active"
                  className="absolute -bottom-0.5 h-0.5 w-3 rounded-full bg-primary"
                  transition={springs.snappy}
                />
              )}
            </motion.button>
          );
        })}
      </div>

      {/* Spacer — primary drag target */}
      <div className="flex-1 self-stretch" data-tauri-drag-region />

      {/* Utility buttons */}
      <div className="flex shrink-0 items-center gap-0.5 pr-1.5">
        {/* Search */}
        <motion.button
          whileTap={{ scale: 0.9 }}
          transition={springs.micro}
          onClick={() => {
            if (filterView !== "all" && filterView !== "ignored") {
              setFilterView("all");
            }
            setSearch(search.trim() ? "" : " ");
          }}
          className="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
          aria-label="Search"
        >
          <Search className="h-3.5 w-3.5" />
        </motion.button>

        {/* Divider */}
        <div className="mx-1 h-4 w-px bg-border/50" />

        {/* History */}
        <motion.button
          whileTap={{ scale: 0.9 }}
          whileHover={{ scale: 1.05 }}
          transition={springs.micro}
          onClick={() => setFilterView(filterView === "history" ? "all" : "history")}
          className={cn(
            "flex h-7 w-7 items-center justify-center rounded-md transition-colors",
            filterView === "history"
              ? "bg-accent text-accent-foreground"
              : "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
          )}
          aria-label="History"
        >
          <Clock className="h-3.5 w-3.5" />
        </motion.button>

        {/* Settings */}
        <motion.button
          whileTap={{ scale: 0.9 }}
          whileHover={{ scale: 1.05 }}
          transition={springs.micro}
          onClick={() => setFilterView(filterView === "settings" ? "all" : "settings")}
          className={cn(
            "flex h-7 w-7 items-center justify-center rounded-md transition-colors",
            filterView === "settings"
              ? "bg-accent text-accent-foreground"
              : "text-muted-foreground hover:bg-accent hover:text-accent-foreground",
          )}
          aria-label="Settings"
        >
          <Settings className="h-3.5 w-3.5" />
        </motion.button>

        {/* Close (hides to tray) */}
        <motion.button
          whileTap={{ scale: 0.9 }}
          transition={springs.micro}
          onClick={() => {
            getCurrentWindow().hide().catch(console.error);
          }}
          className="flex h-7 w-7 items-center justify-center rounded-md text-muted-foreground/60 transition-colors hover:bg-destructive/10 hover:text-muted-foreground"
          aria-label="Close"
        >
          <X className="h-3.5 w-3.5" />
        </motion.button>
      </div>
    </div>
  );
}
