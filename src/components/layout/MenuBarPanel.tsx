import { RefreshCw } from "lucide-react";
import { motion } from "motion/react";
import type { ReactNode } from "react";
import { springs } from "@/lib/animations";
import { cn } from "@/lib/utils";

interface MenuBarPanelProps {
  children: ReactNode;
  updateCount?: number;
  onOpenMain?: () => void;
  onCheckNow?: () => void;
  isChecking?: boolean;
  className?: string;
}

export default function MenuBarPanel({
  children,
  updateCount = 0,
  onOpenMain,
  onCheckNow,
  isChecking = false,
  className,
}: MenuBarPanelProps) {
  return (
    <div className="flex h-screen w-[380px] flex-col items-center pt-2">
      {/* Upward-pointing caret */}
      <div className="h-0 w-0 border-x-8 border-b-8 border-x-transparent border-b-[var(--glass-border)]" />
      <div
        className={cn(
          "flex w-full flex-1 flex-col overflow-hidden rounded-xl",
          "bg-popover/95 backdrop-blur-2xl text-popover-foreground",
          "border border-[var(--glass-border)]",
          className,
        )}
      >
        {/* Header */}
        <div className="flex shrink-0 items-center justify-between border-b border-border/50 bg-background/40 backdrop-blur-xl px-4 py-2.5">
          <div className="flex items-center gap-2">
            <h1 className="text-sm font-semibold">macPlus</h1>
            {updateCount > 0 && (
              <span className="flex h-5 min-w-5 items-center justify-center rounded-full bg-primary px-1.5 text-caption font-semibold text-primary-foreground">
                {updateCount}
              </span>
            )}
          </div>

          <span className="text-xs text-muted-foreground">
            {updateCount > 0
              ? `${updateCount} update${updateCount === 1 ? "" : "s"} available`
              : "All apps up to date"}
          </span>
        </div>

        {/* Scrollable list area */}
        <div className="flex-1 overflow-y-auto">{children}</div>

        {/* Footer */}
        <div className="flex shrink-0 items-center justify-between border-t border-border/50 bg-background/30 backdrop-blur-xl px-3 py-2">
          <button
            onClick={onOpenMain}
            className="text-xs font-medium text-primary transition-colors hover:text-primary/80"
          >
            Open macPlus
          </button>

          <motion.button
            whileTap={{ scale: 0.95 }}
            transition={springs.micro}
            onClick={onCheckNow}
            disabled={isChecking}
            className={cn(
              "flex items-center gap-1.5 rounded-md bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground transition-colors",
              "hover:bg-primary/90",
              "disabled:opacity-50",
            )}
          >
            <RefreshCw className={cn("h-3 w-3", isChecking && "animate-spin")} />
            {isChecking ? "Checking..." : "Check Now"}
          </motion.button>
        </div>
      </div>
    </div>
  );
}
