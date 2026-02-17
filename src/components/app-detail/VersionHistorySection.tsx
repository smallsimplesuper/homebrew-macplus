import { Clock } from "lucide-react";

export function VersionHistorySection() {
  return (
    <div className="space-y-1">
      <h4 className="text-caption-uppercase tracking-wider text-muted-foreground">
        Version History
      </h4>
      <div className="flex flex-col items-center gap-2 rounded-lg border border-border bg-background p-6">
        <Clock className="h-5 w-5 text-muted-foreground/50" />
        <p className="text-xs text-muted-foreground">No update history yet</p>
      </div>
    </div>
  );
}
