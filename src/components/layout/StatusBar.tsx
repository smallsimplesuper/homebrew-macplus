import { useState } from "react";
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

function ConnectivityPopover({
  details,
  status,
}: {
  details: { github: boolean; homebrew: boolean; itunes: boolean } | undefined;
  status: string;
}) {
  const [open, setOpen] = useState(false);

  const connectivityColor =
    status === "connected" ? "bg-green-500" : status === "partial" ? "bg-orange-400" : "bg-red-500";

  const services = details
    ? [
        { name: "GitHub API", ok: details.github },
        { name: "Homebrew API", ok: details.homebrew },
        { name: "iTunes API", ok: details.itunes },
      ]
    : [];

  return (
    <div className="relative">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        onBlur={() => setTimeout(() => setOpen(false), 150)}
        className="flex items-center"
      >
        <div className={cn("h-2 w-2 rounded-full", connectivityColor)} />
      </button>
      {open && (
        <div className="absolute bottom-full right-0 mb-2 w-52 rounded-lg border border-border bg-popover p-2 shadow-lg">
          <p className="mb-1.5 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
            Service Status
          </p>
          <div className="flex flex-col gap-1">
            {services.map((svc) => (
              <div key={svc.name} className="flex items-center justify-between">
                <div className="flex items-center gap-1.5">
                  <div
                    className={cn(
                      "h-1.5 w-1.5 rounded-full",
                      svc.ok ? "bg-green-500" : "bg-red-500",
                    )}
                  />
                  <span className="text-xs text-foreground/80">{svc.name}</span>
                </div>
                <span className="text-[10px] text-muted-foreground">
                  {svc.ok ? "Connected" : "Offline"}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

export default function StatusBar({
  appCount,
  updateCount = 0,
  ignoredCount: _ignoredCount = 0,
  className,
}: StatusBarProps) {
  const { progress: scanProgress, isScanning } = useScanProgress();
  const { progress: checkProgress, isChecking, lastChecked } = useUpdateCheckProgress();
  const { status: connectivity, details: connectivityDetails } = useConnectivity();

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

  // Right-side fixed elements (version + connectivity + logo)
  const rightSection = (
    <div className="ml-auto flex shrink-0 items-center gap-2">
      <span className="text-muted-foreground/40">v{__APP_VERSION__}</span>
      <ConnectivityPopover details={connectivityDetails} status={connectivity} />
      <S3Logo className="opacity-40 transition-opacity hover:opacity-70" />
    </div>
  );

  // Priority: scanning > checking > idle
  const renderContent = () => {
    if (isScanning && scanProgress) {
      const percent =
        scanProgress.total > 0 ? (scanProgress.current / scanProgress.total) * 100 : 0;

      return (
        <div className="flex flex-1 items-center gap-2">
          <div className="flex flex-1 flex-col gap-0.5">
            <div className="h-1.5 w-full overflow-hidden rounded-full bg-muted">
              <div
                className="h-full rounded-full bg-primary shadow-[0_0_6px_var(--primary)] transition-all duration-300"
                style={{ width: `${percent}%` }}
              />
            </div>
            <span className="text-[10px] tabular-nums text-muted-foreground">
              Scanning
              {scanProgress.phase ? ` \u2014 ${scanProgress.phase}` : ""}
              {scanProgress.appName ? `: ${scanProgress.appName}` : ""}
              {scanProgress.total > 0 ? ` (${scanProgress.current}/${scanProgress.total})` : ""}
            </span>
          </div>
          {rightSection}
        </div>
      );
    }

    if (isChecking && checkProgress) {
      const percent =
        checkProgress.total > 0 ? (checkProgress.checked / checkProgress.total) * 100 : 0;

      return (
        <div className="flex flex-1 items-center gap-2">
          <div className="flex flex-1 flex-col gap-0.5">
            <div className="h-1.5 w-full overflow-hidden rounded-full bg-muted">
              <div
                className="h-full rounded-full bg-primary shadow-[0_0_6px_var(--primary)] transition-all duration-300"
                style={{ width: `${percent}%` }}
              />
            </div>
            <span className="text-[10px] tabular-nums text-muted-foreground">
              Checking updates
              {checkProgress.currentApp ? ` \u2014 ${checkProgress.currentApp}` : ""}
              {checkProgress.total > 0 ? ` (${checkProgress.checked}/${checkProgress.total})` : ""}
            </span>
          </div>
          {rightSection}
        </div>
      );
    }

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
        {rightSection}
      </div>
    );
  };

  return (
    <div className={cn("shrink-0", className)}>
      <div className="statusbar-glow" />
      <div className="flex h-7 items-center bg-[var(--statusbar-bg)] px-4 backdrop-blur-xl text-footnote">
        {renderContent()}
      </div>
    </div>
  );
}
