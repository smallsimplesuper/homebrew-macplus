import { cn } from "@/lib/utils";

interface VersionBadgeProps {
  current: string | null;
  available: string | null;
  hasUpdate: boolean;
}

export function VersionBadge({ current, available, hasUpdate }: VersionBadgeProps) {
  if (!hasUpdate || !available) {
    return <span className="font-mono text-xs text-muted-foreground">{current ?? "—"}</span>;
  }

  return <span className={cn("font-mono text-xs font-semibold text-success")}>{available}</span>;
}
