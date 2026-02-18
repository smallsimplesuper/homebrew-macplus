import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-shell";
import { ArrowUpCircle, CheckCircle2, ExternalLink, Loader2, RefreshCw, X } from "lucide-react";
import { motion } from "motion/react";
import { useEffect, useState } from "react";
import { formatDownloadProgress } from "@/lib/format-bytes";
import {
  checkSelfUpdate,
  executeSelfUpdate,
  relaunchSelf,
  type SelfUpdateInfo,
  type SelfUpdateProgress,
} from "@/lib/tauri-commands";
import { cn } from "@/lib/utils";
import { useCrawlingPercent } from "./InlineUpdateProgress";

export function SelfUpdateBanner() {
  const [info, setInfo] = useState<SelfUpdateInfo | null>(null);
  const [dismissed, setDismissed] = useState(false);
  const [isUpdating, setIsUpdating] = useState(false);
  const [progress, setProgress] = useState<SelfUpdateProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [updateComplete, setUpdateComplete] = useState(false);
  const [isRelaunching, setIsRelaunching] = useState(false);

  useEffect(() => {
    checkSelfUpdate()
      .then((result) => {
        if (result) setInfo(result);
      })
      .catch(() => {});

    const unlistenAvailable = listen<SelfUpdateInfo>("self-update-available", (event) => {
      setInfo(event.payload);
      setDismissed(false);
    });

    const unlistenProgress = listen<SelfUpdateProgress>("self-update-progress", (event) => {
      setProgress(event.payload);
    });

    const unlistenComplete = listen<{ success: boolean }>("self-update-complete", (event) => {
      if (event.payload.success) {
        setUpdateComplete(true);
      }
    });

    return () => {
      unlistenAvailable.then((fn) => fn());
      unlistenProgress.then((fn) => fn());
      unlistenComplete.then((fn) => fn());
    };
  }, []);

  if (dismissed || !info) return null;

  const handleUpdate = () => {
    if (!info.downloadUrl) {
      const url =
        info.releaseNotesUrl ??
        "https://github.com/smallsimplesuper/homebrew-macplus/releases/latest";
      open(url).catch(console.error);
      return;
    }

    setIsUpdating(true);
    setError(null);
    setProgress(null);
    executeSelfUpdate(info.downloadUrl).catch((err) => {
      setIsUpdating(false);
      setError(String(err));
    });
  };

  const handleRetry = () => {
    setError(null);
    handleUpdate();
  };

  if (error) {
    return (
      <div className="flex items-center gap-3 border-b border-destructive/20 bg-destructive/5 px-4 py-2.5">
        <ArrowUpCircle className="h-4 w-4 shrink-0 text-destructive" />
        <p className="flex-1 truncate text-xs text-destructive">{error}</p>
        <button
          type="button"
          onClick={handleRetry}
          className={cn(
            "flex items-center gap-1 rounded-md px-2.5 py-1",
            "bg-destructive/10 text-xs font-medium text-destructive",
            "transition-colors hover:bg-destructive/20",
          )}
        >
          Retry
        </button>
        <button
          type="button"
          onClick={() => {
            setError(null);
            setDismissed(true);
          }}
          className="rounded-md p-1 text-destructive/60 transition-colors hover:text-destructive"
        >
          <X className="h-3.5 w-3.5" />
        </button>
      </div>
    );
  }

  if (updateComplete) {
    const handleRelaunch = () => {
      setIsRelaunching(true);
      relaunchSelf().catch(() => setIsRelaunching(false));
    };

    return (
      <div className="flex items-center gap-3 border-b border-green-500/20 bg-green-500/5 px-4 py-2.5">
        {isRelaunching ? (
          <Loader2 className="h-4 w-4 shrink-0 animate-spin text-green-600 dark:text-green-400" />
        ) : (
          <CheckCircle2 className="h-4 w-4 shrink-0 text-green-600 dark:text-green-400" />
        )}
        <p className="flex-1 text-xs text-green-700 dark:text-green-300">
          {isRelaunching ? "Restarting macPlus..." : "Update installed — restart to apply"}
        </p>
        {!isRelaunching && (
          <button
            type="button"
            onClick={handleRelaunch}
            className={cn(
              "flex items-center gap-1 rounded-md px-2.5 py-1",
              "bg-green-500/10 text-xs font-medium text-green-700 dark:text-green-300",
              "transition-colors hover:bg-green-500/20",
            )}
          >
            <RefreshCw className="h-3 w-3" />
            Restart Now
          </button>
        )}
      </div>
    );
  }

  if (isUpdating) {
    return <SelfUpdateProgressBar progress={progress} />;
  }

  return (
    <div className="flex items-center gap-3 border-b border-primary/20 bg-primary/5 px-4 py-2.5">
      <ArrowUpCircle className="h-4 w-4 shrink-0 text-primary" />
      <p className="flex-1 text-xs text-primary">
        macPlus {info.availableVersion} is available{" "}
        <span className="text-primary/60">(current: {info.currentVersion})</span>
      </p>
      <button
        type="button"
        onClick={handleUpdate}
        className={cn(
          "flex items-center gap-1 rounded-md px-2.5 py-1",
          "bg-primary/10 text-xs font-medium text-primary",
          "transition-colors hover:bg-primary/20",
        )}
      >
        {info.downloadUrl ? (
          <ArrowUpCircle className="h-3 w-3" />
        ) : (
          <ExternalLink className="h-3 w-3" />
        )}
        {info.downloadUrl ? "Update" : "Download"}
      </button>
      <button
        type="button"
        onClick={() => setDismissed(true)}
        className="rounded-md p-1 text-primary/60 transition-colors hover:text-primary"
      >
        <X className="h-3.5 w-3.5" />
      </button>
    </div>
  );
}

function SelfUpdateProgressBar({ progress }: { progress: SelfUpdateProgress | null }) {
  const percent = progress?.percent ?? 0;
  const phase = progress?.phase ?? "Preparing update...";
  const hasRealBytes = progress?.downloadedBytes != null && progress.downloadedBytes > 0;
  const displayPercent = useCrawlingPercent(percent, phase, hasRealBytes);

  const byteLabel =
    hasRealBytes && progress
      ? formatDownloadProgress(progress.downloadedBytes as number, progress.totalBytes)
      : null;

  return (
    <div className="flex items-center gap-3 border-b border-primary/20 bg-primary/5 px-4 py-2.5">
      <Loader2 className="h-4 w-4 shrink-0 animate-spin text-primary" />
      <div className="min-w-0 flex-1">
        <div className="mb-1 flex items-center justify-between">
          <span className="truncate text-xs text-primary">{phase}</span>
          <span className="ml-2 shrink-0 text-xs tabular-nums text-primary/60">
            {byteLabel ?? `${Math.round(displayPercent)}%`}
          </span>
        </div>
        <div className="h-1.5 w-full overflow-hidden rounded-full bg-primary/10">
          <motion.div
            className="h-full rounded-full bg-primary"
            initial={{ width: 0 }}
            animate={{ width: `${displayPercent}%` }}
            transition={{ duration: 0.3, ease: "easeOut" }}
          />
        </div>
      </div>
    </div>
  );
}
