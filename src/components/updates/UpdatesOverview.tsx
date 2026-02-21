import { useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-shell";
import {
  ArrowRight,
  CheckCircle,
  CheckCircle2,
  Download,
  EyeOff,
  FileText,
  Globe,
  Loader2,
  PackageMinus,
  RefreshCw,
} from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useMemo, useState } from "react";
import { ReleaseNotesContent } from "@/components/app-detail/ReleaseNotesSection";
import { AppIcon } from "@/components/app-list/AppIcon";
import { InfoPopover } from "@/components/shared/InfoPopover";
import { RelaunchButton, useCrawlingPercent } from "@/components/shared/InlineUpdateProgress";
import { useApps, useFullScan, useToggleIgnored } from "@/hooks/useApps";
import { useCheckAllUpdates } from "@/hooks/useAppUpdates";
import { useTauriEvent } from "@/hooks/useTauriEvent";
import { useExecuteBulkUpdate, useExecuteUpdate } from "@/hooks/useUpdateExecution";
import { formatDownloadProgress } from "@/lib/format-bytes";
import { getUpdateHistory } from "@/lib/tauri-commands";
import { isDelegatedUpdate } from "@/lib/update-utils";
import { cn } from "@/lib/utils";
import { useUIStore } from "@/stores/uiStore";
import { useUpdateProgressStore } from "@/stores/updateProgressStore";
import type { AppSummary } from "@/types/app";
import type { UpdateExecuteComplete } from "@/types/update";

// --- Categorization ---

type UpdateCategory = "desktop_apps" | "browser_extensions" | "homebrew_cli";

const BROWSER_PATTERNS = [
  /^com\.google\.Chrome\.app\./,
  /^com\.brave\.Browser\.app\./,
  /^com\.microsoft\.Edge\.app\./,
  /^org\.chromium\.Chromium\.app\./,
];

function categorizeApp(app: AppSummary): UpdateCategory {
  if (BROWSER_PATTERNS.some((p) => p.test(app.bundleId))) return "browser_extensions";
  if (app.installSource === "homebrew_formula" || app.bundleId.startsWith("homebrew.formula."))
    return "homebrew_cli";
  // CLI-only casks: installed via homebrew, have synthetic bundle ID, no .app path
  if (app.bundleId.startsWith("homebrew.cask.") && !app.appPath) return "homebrew_cli";
  return "desktop_apps";
}

function getBrowserName(bundleId: string): string | null {
  if (bundleId.startsWith("com.google.Chrome")) return "Chrome";
  if (bundleId.startsWith("com.brave.Browser")) return "Brave";
  if (bundleId.startsWith("com.microsoft.Edge")) return "Edge";
  if (bundleId.startsWith("org.chromium.Chromium")) return "Chromium";
  return null;
}

const CATEGORY_LABELS: Record<UpdateCategory, string> = {
  desktop_apps: "Desktop Apps",
  browser_extensions: "Browser Extensions",
  homebrew_cli: "Homebrew & CLI",
};

// --- Update Card ---

function UpdateCard({ app, onUpdate }: { app: AppSummary; onUpdate: (bundleId: string) => void }) {
  const progress = useUpdateProgressStore((s) => s.progress[app.bundleId]);
  const relaunch = useUpdateProgressStore((s) => s.relaunchNeeded[app.bundleId]);
  const toggleIgnored = useToggleIgnored();
  const setUninstallTarget = useUIStore((s) => s.setUninstallTarget);
  const [changelogOpen, setChangelogOpen] = useState(false);

  const hasChangelog = app.releaseNotes != null || app.releaseNotesUrl != null;

  // Smooth crawling percent — called unconditionally per hook rules
  const downloadedBytes = progress?.downloadedBytes ?? 0;
  const totalBytes = progress?.totalBytes ?? null;
  const hasRealBytes = downloadedBytes > 0;
  const effectivePercent =
    hasRealBytes && totalBytes != null && totalBytes > 0
      ? (downloadedBytes / totalBytes) * 100
      : (progress?.percent ?? 0);
  const displayPercent = useCrawlingPercent(effectivePercent, progress?.phase, hasRealBytes);
  const byteLabel = hasRealBytes ? formatDownloadProgress(downloadedBytes, totalBytes) : null;

  // Auto-close changelog when progress starts
  useEffect(() => {
    if (progress) setChangelogOpen(false);
  }, [progress]);

  return (
    <div className="group rounded-lg border border-border bg-card">
      <AnimatePresence mode="wait" initial={false}>
        {progress ? (
          <motion.div
            key="progress"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.15 }}
            className="px-3 py-1.5"
          >
            {/* Row 1: icon + name + version + spinner/percent */}
            <div className="flex items-center gap-2">
              <AppIcon
                iconPath={app.iconCachePath}
                appPath={app.appPath}
                displayName={app.displayName}
                bundleId={app.bundleId}
                size={22}
              />
              <span className="truncate text-xs font-medium leading-tight">{app.displayName}</span>
              <div className="flex shrink-0 items-center gap-1 text-caption leading-tight">
                <span className="text-muted-foreground">{app.installedVersion ?? "?"}</span>
                <ArrowRight className="size-2 shrink-0 text-muted-foreground/50" />
                <span className="text-muted-foreground">{app.availableVersion}</span>
              </div>
              <div className="ml-auto flex shrink-0 items-center gap-1.5">
                <Loader2 className="h-3 w-3 animate-spin text-primary" />
                <span className="text-caption tabular-nums text-muted-foreground">
                  {Math.round(displayPercent)}%
                </span>
              </div>
            </div>
            {/* Row 2: full-width progress bar + phase/bytes */}
            <div className="mt-1.5">
              <div className="h-[3px] w-full overflow-hidden rounded-full bg-muted">
                <motion.div
                  className="h-full rounded-full bg-primary"
                  initial={{ width: 0 }}
                  animate={{ width: `${displayPercent}%` }}
                  transition={{ duration: 0.3, ease: "easeOut" }}
                />
              </div>
              <div className="mt-0.5 flex items-center justify-between">
                <span className="truncate text-caption text-muted-foreground">
                  {progress.phase}
                </span>
                {byteLabel && (
                  <span className="shrink-0 text-caption tabular-nums text-muted-foreground">
                    {byteLabel}
                  </span>
                )}
              </div>
            </div>
          </motion.div>
        ) : (
          <motion.div
            key="normal"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.15 }}
            className={cn(
              "grid min-h-[44px] items-center px-3",
              "grid-cols-[28px_1fr_auto] gap-2.5",
              "transition-colors hover:bg-accent/30 rounded-lg",
            )}
          >
            {/* App icon */}
            <AppIcon
              iconPath={app.iconCachePath}
              appPath={app.appPath}
              displayName={app.displayName}
              bundleId={app.bundleId}
              size={28}
            />

            {/* Name + version inline */}
            <div className="flex min-w-0 items-center gap-1.5">
              <span className="truncate text-sm font-medium leading-tight">{app.displayName}</span>
              <div className="flex shrink-0 items-center gap-1 text-footnote leading-tight">
                <span className="text-muted-foreground">{app.installedVersion ?? "Unknown"}</span>
                <ArrowRight className="size-2.5 shrink-0 text-muted-foreground/50" />
                <span className="font-semibold text-success">{app.availableVersion}</span>
              </div>
            </div>

            {/* Right side: ignore + changelog + action button */}
            <div className="flex items-center gap-1.5">
              <button
                type="button"
                onClick={() => toggleIgnored.mutate({ bundleId: app.bundleId, ignored: true })}
                className={cn(
                  "flex shrink-0 items-center justify-center rounded-md",
                  "h-7 w-7 text-muted-foreground",
                  "opacity-0 transition-all group-hover:opacity-100",
                  "hover:bg-muted hover:text-foreground",
                )}
                title="Ignore this app"
              >
                <EyeOff className="h-3.5 w-3.5" />
              </button>
              {hasChangelog && (
                <button
                  type="button"
                  onClick={() => setChangelogOpen((v) => !v)}
                  className={cn(
                    "flex shrink-0 items-center justify-center rounded-md",
                    "h-7 w-7 text-muted-foreground",
                    "transition-colors hover:bg-muted hover:text-foreground",
                    changelogOpen && "bg-muted text-foreground",
                  )}
                  title="Release notes"
                >
                  <FileText className="h-3.5 w-3.5" />
                </button>
              )}

              <InfoPopover app={app} />

              <button
                type="button"
                onClick={() =>
                  setUninstallTarget({
                    bundleId: app.bundleId,
                    displayName: app.displayName,
                    appPath: app.appPath,
                    installSource: app.installSource,
                    iconCachePath: app.iconCachePath,
                    installedVersion: app.installedVersion,
                    homebrewCaskToken: app.homebrewCaskToken,
                    homebrewFormulaName: app.homebrewFormulaName,
                  })
                }
                className={cn(
                  "flex shrink-0 items-center justify-center rounded-md",
                  "h-7 w-7 text-muted-foreground",
                  "transition-colors hover:bg-destructive/10 hover:text-destructive",
                )}
                title="Uninstall"
              >
                <PackageMinus className="h-3.5 w-3.5" />
              </button>

              {relaunch ? (
                <RelaunchButton
                  bundleId={relaunch.bundleId}
                  appPath={relaunch.appPath}
                  variant="compact"
                />
              ) : isDelegatedUpdate(app) ? (
                <div className="flex items-center gap-1">
                  <button
                    type="button"
                    onClick={() => onUpdate(app.bundleId)}
                    className={cn(
                      "flex shrink-0 items-center gap-1.5 rounded-md",
                      "bg-primary px-2.5 py-1.5",
                      "text-xs font-medium text-primary-foreground",
                      "transition-colors hover:bg-primary/90",
                    )}
                  >
                    Open App
                  </button>
                  {app.releaseNotesUrl && (
                    <button
                      type="button"
                      onClick={() => open(app.releaseNotesUrl!)}
                      className={cn(
                        "flex shrink-0 items-center justify-center rounded-md",
                        "h-7 w-7 text-muted-foreground",
                        "transition-colors hover:bg-muted hover:text-foreground",
                      )}
                      title="Release notes"
                    >
                      <Globe className="h-3 w-3" />
                    </button>
                  )}
                </div>
              ) : (
                <button
                  type="button"
                  onClick={() => onUpdate(app.bundleId)}
                  className={cn(
                    "flex shrink-0 items-center gap-1.5 rounded-md",
                    "bg-primary px-2.5 py-1.5",
                    "text-xs font-medium text-primary-foreground",
                    "transition-colors hover:bg-primary/90",
                  )}
                >
                  <Download className="h-3 w-3" />
                  Update
                </button>
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* Expandable changelog */}
      <AnimatePresence>
        {changelogOpen && hasChangelog && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.2, ease: "easeInOut" }}
            className="overflow-hidden"
          >
            <div className="border-t border-border px-3 pb-3 pt-2">
              <ReleaseNotesContent
                releaseNotes={app.releaseNotes}
                releaseNotesUrl={app.releaseNotesUrl}
              />
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

// --- Recently Updated ---

function formatRelativeTime(dateStr: string | null): string {
  if (!dateStr) return "";
  const date = new Date(`${dateStr}Z`);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMin = Math.floor(diffMs / 60000);
  const diffHr = Math.floor(diffMin / 60);

  if (diffMin < 1) return "just now";
  if (diffMin < 60) return `${diffMin} min ago`;
  if (diffHr < 24) return `${diffHr}h ago`;
  return `${Math.floor(diffHr / 24)}d ago`;
}

function RecentlyUpdated() {
  const queryClient = useQueryClient();

  const { data: entries } = useQuery({
    queryKey: ["update-history-recent"],
    queryFn: () => getUpdateHistory(20),
    refetchInterval: 60 * 1000,
  });

  useTauriEvent<UpdateExecuteComplete>("update-execute-complete", () => {
    setTimeout(() => {
      queryClient.invalidateQueries({ queryKey: ["update-history-recent"] });
    }, 2000);
  });

  const [clearedAt, setClearedAt] = useState<string | null>(
    () => localStorage.getItem("macplus-recently-cleared-at") || null,
  );

  const handleClear = () => {
    const now = new Date().toISOString();
    localStorage.setItem("macplus-recently-cleared-at", now);
    setClearedAt(now);
  };

  const recent = useMemo(() => {
    if (!entries) return [];
    const sevenDaysAgo = Date.now() - 7 * 24 * 60 * 60 * 1000;
    const clearedAtMs = clearedAt ? new Date(clearedAt).getTime() : 0;
    const seen = new Set<string>();
    return entries
      .filter((e) => e.status === "completed")
      .filter((e) => {
        const ts = e.startedAt ? new Date(`${e.startedAt}Z`).getTime() : 0;
        return ts > sevenDaysAgo && ts > clearedAtMs;
      })
      .filter((e) => e.fromVersion !== e.toVersion)
      .filter((e) => {
        if (seen.has(e.bundleId)) return false;
        seen.add(e.bundleId);
        return true;
      });
  }, [entries, clearedAt]);

  if (recent.length === 0) return null;

  return (
    <div>
      <div className="flex items-center justify-between mb-2">
        <h2 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          Recently Updated
        </h2>
        <button
          type="button"
          onClick={handleClear}
          className="text-caption text-muted-foreground/50 hover:text-muted-foreground"
        >
          Clear
        </button>
      </div>
      <div className="flex flex-col gap-2">
        {recent.map((entry) => (
          <div
            key={entry.id}
            className="grid min-h-[44px] items-center rounded-lg border border-border/50 bg-card px-3 grid-cols-[28px_1fr_auto] gap-2.5 opacity-60"
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
            <div className="flex items-center gap-1.5">
              <span className="text-caption text-muted-foreground/50">
                {formatRelativeTime(entry.completedAt ?? entry.startedAt)}
              </span>
              <CheckCircle2 className="h-3.5 w-3.5 text-success/60" />
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

// --- Main Component ---

export function UpdatesOverview() {
  const { data: apps, isLoading } = useApps();
  const checkAll = useCheckAllUpdates();
  const fullScan = useFullScan();
  const executeUpdate = useExecuteUpdate();
  const executeBulk = useExecuteBulkUpdate();
  const hasAnyProgress = useUpdateProgressStore((s) => Object.keys(s.progress).length > 0);

  const updatableApps = apps?.filter((app) => app.hasUpdate && !app.isIgnored) ?? [];
  const updateCount = updatableApps.length;

  const categorized = useMemo(() => {
    const groups: Record<UpdateCategory, AppSummary[]> = {
      desktop_apps: [],
      browser_extensions: [],
      homebrew_cli: [],
    };

    for (const app of updatableApps) {
      const cat = categorizeApp(app);
      groups[cat].push(app);
    }

    // Sort each category A-Z
    for (const cat of Object.keys(groups) as UpdateCategory[]) {
      groups[cat].sort((a, b) => a.displayName.localeCompare(b.displayName));
    }

    return groups;
  }, [updatableApps]);

  // Determine if we need category headers (more than one category has items)
  const nonEmptyCategories = (Object.keys(categorized) as UpdateCategory[]).filter(
    (cat) => categorized[cat].length > 0,
  );
  const showCategories = nonEmptyCategories.length > 1;

  const handleCheckNow = () => {
    fullScan.mutate(undefined, {
      onSuccess: () => checkAll.mutate(),
    });
  };

  const handleUpdateAll = () => {
    const ids = updatableApps.map((app) => app.bundleId);
    if (ids.length > 0) {
      executeBulk.mutate(ids);
    }
  };

  const handleUpdateSingle = (bundleId: string) => {
    executeUpdate.mutate(bundleId);
  };

  if (isLoading) {
    return (
      <div className="flex items-center justify-center py-16">
        <RefreshCw className="h-5 w-5 animate-spin text-muted-foreground" />
      </div>
    );
  }

  const renderCategoryLabel = (cat: UpdateCategory): string => {
    if (cat === "browser_extensions") {
      // Extract browser name from first app in category
      const firstApp = categorized[cat][0];
      const browser = firstApp ? getBrowserName(firstApp.bundleId) : null;
      return browser
        ? `Browser Extensions \u2014 ${browser} (${categorized[cat].length})`
        : `Browser Extensions (${categorized[cat].length})`;
    }
    return `${CATEGORY_LABELS[cat]} (${categorized[cat].length})`;
  };

  return (
    <div className="flex flex-col gap-4 p-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h1 className="text-title text-foreground">
          {updateCount > 0
            ? `${updateCount} Update${updateCount === 1 ? "" : "s"} Available`
            : "Updates"}
        </h1>
        <div className="flex items-center gap-2">
          <button
            type="button"
            onClick={handleCheckNow}
            disabled={fullScan.isPending || checkAll.isPending}
            className={cn(
              "flex items-center gap-1.5 rounded-lg",
              "border border-border bg-background px-3 py-1.5",
              "text-xs font-medium text-foreground",
              "transition-colors hover:bg-muted",
              "disabled:opacity-50 disabled:cursor-not-allowed",
            )}
          >
            <RefreshCw
              className={cn(
                "h-3.5 w-3.5",
                (fullScan.isPending || checkAll.isPending) && "animate-spin",
              )}
            />
            Check Now
          </button>
          {updateCount > 0 && (
            <button
              type="button"
              onClick={handleUpdateAll}
              disabled={executeBulk.isPending || hasAnyProgress}
              className={cn(
                "flex items-center gap-1.5 rounded-lg",
                "bg-primary px-3 py-1.5",
                "text-xs font-medium text-primary-foreground",
                "transition-colors hover:bg-primary/90",
                "disabled:opacity-50 disabled:cursor-not-allowed",
              )}
            >
              <Download className="h-3.5 w-3.5" />
              Update All
            </button>
          )}
        </div>
      </div>

      {/* Update list or empty state */}
      {updateCount === 0 ? (
        <div className="flex flex-col items-center gap-3 py-16">
          <CheckCircle className="h-12 w-12 text-success/60" />
          <div className="text-center">
            <p className="text-sm font-medium text-foreground">All apps are up to date!</p>
            <p className="mt-1 text-xs text-muted-foreground">
              macPlus will notify you when updates become available.
            </p>
          </div>
        </div>
      ) : showCategories ? (
        <div className="flex flex-col gap-4">
          {nonEmptyCategories.map((cat) => (
            <div key={cat}>
              <h2 className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                {renderCategoryLabel(cat)}
              </h2>
              <div className="flex flex-col gap-2">
                {categorized[cat].map((app) => (
                  <UpdateCard key={app.bundleId} app={app} onUpdate={handleUpdateSingle} />
                ))}
              </div>
            </div>
          ))}
        </div>
      ) : (
        <div className="flex flex-col gap-2">
          {[...updatableApps]
            .sort((a, b) => a.displayName.localeCompare(b.displayName))
            .map((app) => (
              <UpdateCard key={app.bundleId} app={app} onUpdate={handleUpdateSingle} />
            ))}
        </div>
      )}

      {/* Recently Updated */}
      <RecentlyUpdated />
    </div>
  );
}
