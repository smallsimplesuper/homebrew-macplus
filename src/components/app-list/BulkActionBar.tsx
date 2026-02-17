import { AnimatePresence, motion } from "framer-motion";
import { springs } from "@/lib/animations";

interface BulkActionBarProps {
  selectedCount: number;
  onUpdateSelected: () => void;
  onUpdateAll: () => void;
  onClearSelection: () => void;
}

export function BulkActionBar({
  selectedCount,
  onUpdateSelected,
  onUpdateAll,
  onClearSelection,
}: BulkActionBarProps) {
  return (
    <AnimatePresence>
      {selectedCount > 0 && (
        <motion.div
          initial={{ y: 20, opacity: 0 }}
          animate={{ y: 0, opacity: 1 }}
          exit={{ y: 20, opacity: 0 }}
          transition={springs.snappy}
          className="flex items-center gap-3 rounded-xl border bg-background/95 px-4 py-2.5 shadow-lg backdrop-blur-sm"
        >
          <span className="text-sm font-medium text-muted-foreground">
            {selectedCount} selected
          </span>

          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={onUpdateSelected}
              className="rounded-md bg-primary px-3 py-1.5 text-xs font-semibold text-primary-foreground hover:bg-primary/90 transition-colors"
            >
              Update Selected
            </button>

            <button
              type="button"
              onClick={onUpdateAll}
              className="rounded-md bg-primary px-3 py-1.5 text-xs font-semibold text-primary-foreground hover:bg-primary/90 transition-colors"
            >
              Update All
            </button>

            <button
              type="button"
              onClick={onClearSelection}
              className="px-2 py-1.5 text-xs text-muted-foreground hover:text-foreground transition-colors"
            >
              Clear
            </button>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
