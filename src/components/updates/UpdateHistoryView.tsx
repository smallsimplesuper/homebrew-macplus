import { useQuery } from "@tanstack/react-query";
import { ArrowRight, CheckCircle2, Clock, ExternalLink, RefreshCw, XCircle } from "lucide-react";
import { AppIcon } from "@/components/app-list/AppIcon";
import { getUpdateHistory } from "@/lib/tauri-commands";
import type { UpdateHistoryEntry } from "@/types/update";

function formatRelativeTime(dateStr: string | null): string {
  if (!dateStr) return "";
  const date = new Date(`${dateStr}Z`); // SQLite stores UTC
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMin = Math.floor(diffMs / 60000);
  const diffHr = Math.floor(diffMin / 60);
  const diffDay = Math.floor(diffHr / 24);

  if (diffMin < 1) return "just now";
  if (diffMin < 60) return `${diffMin} min ago`;
  if (diffHr < 24) return `${diffHr}h ago`;
  if (diffDay === 1) return "yesterday";
  return `${diffDay}d ago`;
}

function groupByDate(entries: UpdateHistoryEntry[]): Map<string, UpdateHistoryEntry[]> {
  const groups = new Map<string, UpdateHistoryEntry[]>();
  const now = new Date();
  const today = now.toDateString();
  const yesterday = new Date(now.getTime() - 86400000).toDateString();

  for (const entry of entries) {
    const date = entry.startedAt ? new Date(`${entry.startedAt}Z`) : new Date();
    const dateStr = date.toDateString();

    let label: string;
    if (dateStr === today) label = "Today";
    else if (dateStr === yesterday) label = "Yesterday";
    else
      label = date.toLocaleDateString(undefined, {
        weekday: "long",
        month: "short",
        day: "numeric",
      });

    const group = groups.get(label) ?? [];
    group.push(entry);
    groups.set(label, group);
  }

  return groups;
}

function StatusBadge({ status }: { status: string }) {
  if (status === "completed") {
    return <CheckCircle2 className="h-3.5 w-3.5 text-success" />;
  }
  if (status === "delegated") {
    return <ExternalLink className="h-3.5 w-3.5 text-muted-foreground" />;
  }
  if (status === "failed") {
    return <XCircle className="h-3.5 w-3.5 text-destructive" />;
  }
  return <Clock className="h-3.5 w-3.5 text-muted-foreground" />;
}

export function UpdateHistoryView() {
  const {
    data: entries,
    isLoading,
    isError,
    refetch,
  } = useQuery({
    queryKey: ["update-history"],
    queryFn: () => getUpdateHistory(100),
    staleTime: 30_000,
    refetchOnMount: true,
    refetchInterval: 5000,
    retry: 2,
    retryDelay: 1000,
  });

  if (isLoading) {
    return (
      <div className="flex flex-col gap-4 p-4">
        <h1 className="text-title text-foreground">Update History</h1>
        <div className="flex items-center justify-center py-16">
          <RefreshCw className="h-5 w-5 animate-spin text-muted-foreground" />
        </div>
      </div>
    );
  }

  if (isError) {
    return (
      <div className="flex flex-col gap-4 p-4">
        <h1 className="text-title text-foreground">Update History</h1>
        <div className="flex flex-col items-center gap-3 py-16">
          <XCircle className="h-12 w-12 text-destructive/40" />
          <div className="text-center">
            <p className="text-sm font-medium text-foreground">Failed to load history</p>
            <p className="mt-1 text-xs text-muted-foreground">
              The database may be busy. Try again in a moment.
            </p>
          </div>
          <button
            type="button"
            onClick={() => refetch()}
            className="mt-2 flex items-center gap-1.5 rounded-md border border-border bg-background px-3 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-muted"
          >
            <RefreshCw className="h-3 w-3" />
            Retry
          </button>
        </div>
      </div>
    );
  }

  if (!entries || entries.length === 0) {
    return (
      <div className="flex flex-col gap-4 p-4">
        <h1 className="text-title text-foreground">Update History</h1>
        <div className="flex flex-col items-center gap-3 py-16">
          <Clock className="h-12 w-12 text-muted-foreground/40" />
          <div className="text-center">
            <p className="text-sm font-medium text-foreground">No update history yet</p>
            <p className="mt-1 text-xs text-muted-foreground">
              Updates you install will appear here.
            </p>
          </div>
        </div>
      </div>
    );
  }

  const grouped = groupByDate(entries);

  return (
    <div className="flex flex-col gap-4 p-4">
      <h1 className="text-title text-foreground">Update History</h1>

      {Array.from(grouped.entries()).map(([label, group]) => (
        <div key={label}>
          <h2 className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
            {label}
          </h2>
          <div className="flex flex-col gap-2">
            {group.map((entry) => (
              <div
                key={entry.id}
                className="grid min-h-[44px] items-center rounded-lg border border-border bg-card px-3 grid-cols-[28px_1fr_auto] gap-2.5"
              >
                <AppIcon
                  iconPath={entry.iconCachePath}
                  displayName={entry.displayName}
                  bundleId={entry.bundleId}
                  size={28}
                />
                <div className="flex min-w-0 items-center gap-1.5">
                  <span className="truncate text-sm font-medium leading-tight">
                    {entry.displayName}
                  </span>
                  <div className="flex shrink-0 items-center gap-1 text-footnote leading-tight">
                    <span className="text-muted-foreground">{entry.fromVersion}</span>
                    <ArrowRight className="size-2.5 shrink-0 text-muted-foreground/50" />
                    <span className="text-muted-foreground">{entry.toVersion}</span>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <span className="text-footnote text-muted-foreground">
                    {formatRelativeTime(entry.completedAt ?? entry.startedAt)}
                  </span>
                  <StatusBadge status={entry.status} />
                </div>
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
