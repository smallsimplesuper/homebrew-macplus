import { Command } from "cmdk";
import { EyeOff, MonitorSmartphone, RefreshCw, Search } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect } from "react";
import { useApps } from "@/hooks/useApps";
import { springs } from "@/lib/animations";
import { useAppFilterStore } from "@/stores/appFilterStore";
import { useUIStore } from "@/stores/uiStore";

export function CommandPalette() {
  const { commandPaletteOpen, setCommandPaletteOpen, selectApp } = useUIStore();
  const { data: apps } = useApps();
  const { setFilterView } = useAppFilterStore();

  useEffect(() => {
    const down = (e: KeyboardEvent) => {
      if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        setCommandPaletteOpen(!commandPaletteOpen);
      }
      if (e.key === "Escape") {
        setCommandPaletteOpen(false);
      }
    };
    document.addEventListener("keydown", down);
    return () => document.removeEventListener("keydown", down);
  }, [commandPaletteOpen, setCommandPaletteOpen]);

  return (
    <AnimatePresence>
      {commandPaletteOpen && (
        <>
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            className="fixed inset-0 bg-black/30 z-50"
            onClick={() => setCommandPaletteOpen(false)}
          />
          <motion.div
            initial={{ opacity: 0, scale: 0.96, y: -8 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.96, y: -8 }}
            transition={springs.snappy}
            className="fixed left-1/2 top-[20%] -translate-x-1/2 z-50 w-[90%] max-w-[560px]"
          >
            <Command className="rounded-xl border bg-popover shadow-lg overflow-hidden" loop>
              <div className="flex items-center border-b px-3 gap-2">
                <Search className="w-4 h-4 text-muted-foreground shrink-0" />
                <Command.Input
                  placeholder="Search apps, actions..."
                  className="flex h-11 w-full bg-transparent text-sm outline-none placeholder:text-muted-foreground"
                />
              </div>
              <Command.List className="max-h-[300px] overflow-y-auto p-1">
                <Command.Empty className="py-6 text-center text-sm text-muted-foreground">
                  No results found.
                </Command.Empty>

                <Command.Group
                  heading="Actions"
                  className="px-1 py-1.5 text-xs font-semibold text-muted-foreground"
                >
                  <Command.Item
                    className="flex items-center gap-2 px-2 py-1.5 rounded-md text-sm cursor-default aria-selected:bg-accent"
                    onSelect={() => {
                      setFilterView("updates");
                      setCommandPaletteOpen(false);
                    }}
                  >
                    <RefreshCw className="w-4 h-4" />
                    View Updates
                  </Command.Item>
                  <Command.Item
                    className="flex items-center gap-2 px-2 py-1.5 rounded-md text-sm cursor-default aria-selected:bg-accent"
                    onSelect={() => {
                      setFilterView("all");
                      setCommandPaletteOpen(false);
                    }}
                  >
                    <MonitorSmartphone className="w-4 h-4" />
                    View All Apps
                  </Command.Item>
                  <Command.Item
                    className="flex items-center gap-2 px-2 py-1.5 rounded-md text-sm cursor-default aria-selected:bg-accent"
                    onSelect={() => {
                      setFilterView("ignored");
                      setCommandPaletteOpen(false);
                    }}
                  >
                    <EyeOff className="w-4 h-4" />
                    View Ignored
                  </Command.Item>
                </Command.Group>

                {apps && apps.length > 0 && (
                  <Command.Group
                    heading="Apps"
                    className="px-1 py-1.5 text-xs font-semibold text-muted-foreground"
                  >
                    {apps.slice(0, 20).map((app) => (
                      <Command.Item
                        key={app.bundleId}
                        value={`${app.displayName} ${app.bundleId}`}
                        className="flex items-center gap-2 px-2 py-1.5 rounded-md text-sm cursor-default aria-selected:bg-accent"
                        onSelect={() => {
                          selectApp(app.bundleId);
                          setCommandPaletteOpen(false);
                        }}
                      >
                        <div className="w-5 h-5 rounded bg-primary/10 flex items-center justify-center text-caption font-medium text-primary shrink-0">
                          {app.displayName.charAt(0)}
                        </div>
                        <span className="truncate">{app.displayName}</span>
                        {app.hasUpdate && (
                          <span className="ml-auto text-caption bg-primary/10 text-primary px-1.5 py-0.5 rounded-full">
                            Update
                          </span>
                        )}
                      </Command.Item>
                    ))}
                  </Command.Group>
                )}
              </Command.List>
            </Command>
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}
