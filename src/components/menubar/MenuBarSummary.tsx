import { ArrowDownCircle, CheckCircle } from "lucide-react";
import { cn } from "@/lib/utils";

interface MenuBarSummaryProps {
  updateCount: number;
}

export function MenuBarSummary({ updateCount }: MenuBarSummaryProps) {
  const hasUpdates = updateCount > 0;

  return (
    <div className="flex items-center gap-2 px-3 py-1.5">
      {hasUpdates ? (
        <ArrowDownCircle className="h-4 w-4 shrink-0 text-primary" />
      ) : (
        <CheckCircle className="h-4 w-4 shrink-0 text-success" />
      )}
      <span
        className={cn(
          "text-xs font-medium",
          hasUpdates ? "text-foreground" : "text-muted-foreground",
        )}
      >
        {hasUpdates
          ? `${updateCount} update${updateCount === 1 ? "" : "s"} available`
          : "All up to date"}
      </span>
    </div>
  );
}
