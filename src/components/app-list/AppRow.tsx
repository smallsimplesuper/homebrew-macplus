import { open } from "@tauri-apps/plugin-shell";
import { AlertTriangle, ArrowRight, Check, Download, Eye, EyeOff, Globe } from "lucide-react";
import { memo } from "react";
import { InfoPopover } from "@/components/shared/InfoPopover";
import { InlineUpdateProgress, RelaunchButton } from "@/components/shared/InlineUpdateProgress";
import { useToggleIgnored } from "@/hooks/useApps";
import { useExecuteUpdate } from "@/hooks/useUpdateExecution";
import { isDelegatedUpdate } from "@/lib/update-utils";
import { cn } from "@/lib/utils";
import { useAppFilterStore } from "@/stores/appFilterStore";
import { useUIStore } from "@/stores/uiStore";
import { useUpdateProgressStore } from "@/stores/updateProgressStore";
import type { AppSummary } from "@/types/app";
import { AppIcon } from "./AppIcon";

interface AppRowProps {
  app: AppSummary;
  isSelected: boolean;
  onClick: () => void;
}

export const AppRow = memo(
  function AppRow({ app, isSelected }: AppRowProps) {
    const selectApp = useUIStore((s) => s.selectApp);
    const executeUpdate = useExecuteUpdate();
    const toggleIgnored = useToggleIgnored();
    const filterView = useAppFilterStore((s) => s.filterView);
    const progress = useUpdateProgressStore((s) => s.progress[app.bundleId]);
    const relaunch = useUpdateProgressStore((s) => s.relaunchNeeded[app.bundleId]);

    const handleRowClick = () => {
      selectApp(app.bundleId);
    };

    const handleUpdate = (e: React.MouseEvent) => {
      e.stopPropagation();
      executeUpdate.mutate(app.bundleId);
    };

    return (
      <div
        onClick={handleRowClick}
        className={cn(
          "group grid min-h-[44px] cursor-pointer items-center rounded-lg border border-border bg-card px-3 transition-colors duration-150",
          "grid-cols-[28px_1fr_auto]",
          "gap-2.5",
          isSelected ? "bg-accent/60" : "hover:bg-accent/30",
        )}
      >
        {/* Icon */}
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
            <span className="text-muted-foreground">{app.installedVersion ?? "—"}</span>
            {app.hasUpdate && (
              <>
                <ArrowRight className="size-2.5 shrink-0 text-muted-foreground/50" />
                <span className="font-semibold text-success">{app.availableVersion}</span>
              </>
            )}
            {app.updateNotes && (
              <span
                className="flex items-center gap-0.5 text-caption text-amber-500 ml-0.5"
                title={app.updateNotes}
              >
                <AlertTriangle className="size-2.5 shrink-0" />
              </span>
            )}
          </div>
        </div>

        {/* Action button */}
        <div className="flex items-center justify-end gap-1" onClick={(e) => e.stopPropagation()}>
          {filterView === "ignored" ? (
            <button
              type="button"
              onClick={() => toggleIgnored.mutate({ bundleId: app.bundleId, ignored: false })}
              className={cn(
                "flex shrink-0 items-center justify-center rounded-md",
                "h-7 w-7 text-muted-foreground",
                "transition-colors hover:bg-muted hover:text-foreground",
              )}
              title="Unignore this app"
            >
              <Eye className="h-3.5 w-3.5" />
            </button>
          ) : (
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
          )}
          <InfoPopover app={app} />
          {relaunch ? (
            <RelaunchButton
              bundleId={relaunch.bundleId}
              appPath={relaunch.appPath}
              variant="compact"
            />
          ) : progress ? (
            <InlineUpdateProgress
              phase={progress.phase}
              percent={progress.percent}
              variant="compact"
              downloadedBytes={progress.downloadedBytes}
              totalBytes={progress.totalBytes}
            />
          ) : app.hasUpdate ? (
            isDelegatedUpdate(app) ? (
              <div className="flex items-center gap-1">
                <button
                  type="button"
                  onClick={handleUpdate}
                  disabled={executeUpdate.isPending}
                  className={cn(
                    "flex items-center gap-1.5 rounded-md px-3 py-1.5",
                    "bg-primary text-primary-foreground",
                    "text-xs font-medium",
                    "transition-colors hover:bg-primary/90",
                    "disabled:opacity-50",
                  )}
                >
                  Open App
                </button>
                {app.releaseNotesUrl && (
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      open(app.releaseNotesUrl!);
                    }}
                    className={cn(
                      "flex shrink-0 items-center justify-center rounded-md",
                      "h-7 w-7 text-muted-foreground",
                      "transition-colors hover:bg-muted hover:text-foreground",
                    )}
                    title="Release notes"
                  >
                    <Globe className="size-3" />
                  </button>
                )}
              </div>
            ) : (
              <button
                type="button"
                onClick={handleUpdate}
                disabled={executeUpdate.isPending}
                className={cn(
                  "flex items-center gap-1.5 rounded-md px-3 py-1.5",
                  "bg-primary text-primary-foreground",
                  "text-xs font-medium",
                  "transition-colors hover:bg-primary/90",
                  "disabled:opacity-50",
                )}
              >
                <Download className="size-3" />
                Update
              </button>
            )
          ) : (
            <span className="flex items-center gap-1 text-footnote text-muted-foreground/60">
              <Check className="size-3" />
              Up to date
            </span>
          )}
        </div>
      </div>
    );
  },
  (prev, next) =>
    prev.isSelected === next.isSelected &&
    prev.app.bundleId === next.app.bundleId &&
    prev.app.hasUpdate === next.app.hasUpdate &&
    prev.app.availableVersion === next.app.availableVersion &&
    prev.app.installedVersion === next.app.installedVersion &&
    prev.app.isIgnored === next.app.isIgnored &&
    prev.app.updateNotes === next.app.updateNotes,
);
