import S3Logo from "@/components/shared/S3Logo";
import { useConnectivity } from "@/hooks/useConnectivity";
import { useScanProgress } from "@/hooks/useScanProgress";
import { useUpdateCheckProgress } from "@/hooks/useUpdateCheckProgress";
import { cn } from "@/lib/utils";

interface StatusBarProps {
  appCount: number;
  updateCount?: number;
  ignoredCount?: number;
  className?: string;
}

export default function StatusBar({
  appCount,
  updateCount = 0,
  ignoredCount: _ignoredCount = 0,
  className,
}: StatusBarProps) {
  const { progress: scanProgress, isScanning } = useScanProgress();
  const { progress: checkProgress, isChecking, lastChecked } = useUpdateCheckProgress();
  const { status: connectivity } = useConnectivity();

  const formatLastChecked = (date: Date): string => {
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMinutes = Math.floor(diffMs / 60_000);

    if (diffMinutes < 1) return "just now";
    if (diffMinutes < 60) return `${diffMinutes}m ago`;

    const diffHours = Math.floor(diffMinutes / 60);
    if (diffHours < 24) return `${diffHours}h ago`;

    const diffDays = Math.floor(diffHours / 24);
    return `${diffDays}d ago`;
  };

  // Priority: scanning > checking > idle
  const renderContent = () => {
    if (isScanning && scanProgress) {
      return (
        <div className="flex flex-1 items-center gap-2 text-footnote tabular-nums text-muted-foreground transition-opacity duration-200">
          <div className="h-1.5 w-20 overflow-hidden rounded-full bg-muted">
            <div
              className="h-full rounded-full bg-primary shadow-[0_0_6px_var(--primary)] transition-all duration-300"
              style={{
                width:
                  scanProgress.total > 0
                    ? `${(scanProgress.current / scanProgress.total) * 100}%`
                    : "0%",
              }}
            />
          </div>
          <span>
            Scanning for apps
            {scanProgress.phase ? ` \u2014 ${scanProgress.phase}` : ""}
            {scanProgress.appName ? `: ${scanProgress.appName}` : ""}
            {scanProgress.total > 0 ? ` (${scanProgress.current}/${scanProgress.total})` : ""}
          </span>
        </div>
      );
    }

    if (isChecking && checkProgress) {
      return (
        <div className="flex flex-1 items-center gap-2 text-footnote tabular-nums text-muted-foreground transition-opacity duration-200">
          <div className="h-1.5 w-20 overflow-hidden rounded-full bg-muted">
            <div
              className="h-full rounded-full bg-primary shadow-[0_0_6px_var(--primary)] transition-all duration-300"
              style={{
                width:
                  checkProgress.total > 0
                    ? `${(checkProgress.checked / checkProgress.total) * 100}%`
                    : "0%",
              }}
            />
          </div>
          <span>
            Checking for updates
            {checkProgress.currentApp ? `: ${checkProgress.currentApp}` : ""}
            {checkProgress.total > 0 ? ` (${checkProgress.checked}/${checkProgress.total})` : ""}
          </span>
        </div>
      );
    }

    const connectivityColor =
      connectivity === "connected"
        ? "bg-green-500"
        : connectivity === "partial"
          ? "bg-orange-400"
          : "bg-red-500";

    const connectivityTooltip =
      connectivity === "connected"
        ? "Connected to all services"
        : connectivity === "partial"
          ? "Connected to some services"
          : "No services connected — check internet";

    return (
      <div className="flex flex-1 items-center gap-1 text-footnote tabular-nums text-muted-foreground">
        <span>{appCount} apps</span>
        {updateCount > 0 && (
          <>
            <span className="text-muted-foreground/40">&middot;</span>
            <span className="font-semibold text-primary">
              {updateCount} update{updateCount === 1 ? "" : "s"}
            </span>
          </>
        )}
        {lastChecked && (
          <>
            <span className="text-muted-foreground/40">&middot;</span>
            <span>Checked {formatLastChecked(lastChecked)}</span>
          </>
        )}
        <div className="ml-auto flex items-center gap-2">
          <span className="text-muted-foreground/40">v{__APP_VERSION__}</span>
          <div className="group relative">
            <div className={cn("h-2 w-2 rounded-full", connectivityColor)} />
            <div className="absolute bottom-full right-0 mb-1.5 hidden whitespace-nowrap rounded-md bg-popover px-2 py-1 text-xs text-popover-foreground shadow-md group-hover:block">
              {connectivityTooltip}
            </div>
          </div>
          <S3Logo className="opacity-40 transition-opacity hover:opacity-70" />
        </div>
      </div>
    );
  };

  return (
    <div className={cn("shrink-0", className)}>
      <div className="statusbar-glow" />
      <div className="flex h-7 items-center bg-[var(--statusbar-bg)] px-4 backdrop-blur-xl">
        {renderContent()}
      </div>
    </div>
  );
}
