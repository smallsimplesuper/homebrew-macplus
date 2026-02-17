import { AppWindow, RefreshCw } from "lucide-react";
import { cn } from "@/lib/utils";

interface MenuBarFooterProps {
  onOpenMain: () => void;
  onCheckNow: () => void;
  isChecking?: boolean;
}

export function MenuBarFooter({ onOpenMain, onCheckNow, isChecking = false }: MenuBarFooterProps) {
  return (
    <div className="flex items-center justify-between border-t border-border/50 bg-background/30 backdrop-blur-xl px-3 py-2">
      <button
        type="button"
        onClick={onOpenMain}
        className={cn(
          "flex items-center gap-1.5 rounded-md px-2 py-1",
          "text-xs font-medium text-muted-foreground",
          "transition-colors hover:bg-muted hover:text-foreground",
        )}
      >
        <AppWindow className="h-3.5 w-3.5" />
        Open macPlus
      </button>
      <button
        type="button"
        onClick={onCheckNow}
        disabled={isChecking}
        className={cn(
          "flex items-center gap-1.5 rounded-md px-2 py-1",
          "text-xs font-medium text-muted-foreground",
          "transition-colors hover:bg-muted hover:text-foreground",
          "disabled:cursor-not-allowed disabled:opacity-50",
        )}
      >
        <RefreshCw className={cn("h-3.5 w-3.5", isChecking && "animate-spin")} />
        Check Now
      </button>
    </div>
  );
}
