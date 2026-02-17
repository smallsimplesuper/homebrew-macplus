import { AlertTriangle, ArrowRight, Check, Download, ExternalLink } from "lucide-react";
import { memo } from "react";
import { InlineUpdateProgress, RelaunchButton } from "@/components/shared/InlineUpdateProgress";
import { useExecuteUpdate } from "@/hooks/useUpdateExecution";
import { isDelegatedUpdate } from "@/lib/update-utils";
import { cn } from "@/lib/utils";
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
          "grid min-h-[44px] cursor-pointer items-center rounded-lg border border-border bg-card px-3 transition-colors duration-150",
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
        <div className="flex items-center justify-end" onClick={(e) => e.stopPropagation()}>
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
              {isDelegatedUpdate(app) ? (
                <>
                  <ExternalLink className="size-3" />
                  Open Updater
                </>
              ) : (
                <>
                  <Download className="size-3" />
                  Update
                </>
              )}
            </button>
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
