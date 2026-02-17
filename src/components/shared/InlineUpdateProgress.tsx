import { motion } from "framer-motion";
import { Loader2, RotateCcw } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { formatDownloadProgress } from "@/lib/format-bytes";
import { relaunchApp } from "@/lib/tauri-commands";
import { cn } from "@/lib/utils";
import { useUpdateProgressStore } from "@/stores/updateProgressStore";

interface InlineUpdateProgressProps {
  phase: string;
  percent: number;
  variant?: "compact" | "card";
  downloadedBytes?: number;
  totalBytes?: number | null;
}

function useCrawlingPercent(percent: number, phase?: string, hasRealBytes?: boolean) {
  const [displayPercent, setDisplayPercent] = useState(percent);
  const lastPercent = useRef(percent);

  useEffect(() => {
    // When backend percent changes, snap to it
    if (percent !== lastPercent.current) {
      lastPercent.current = percent;
      setDisplayPercent(percent);
    }
  }, [percent]);

  useEffect(() => {
    // When we have real byte data, don't crawl — the backend sends accurate progress
    if (hasRealBytes) return;

    // When percent is 100, snap immediately
    if (percent === 100) {
      setDisplayPercent(100);
      return;
    }

    // When percent is 0 or already at 100, don't crawl
    if (percent <= 0) return;

    // Don't crawl while waiting for user input (e.g., admin password dialog)
    const isWaiting =
      phase?.toLowerCase().includes("administrator") || phase?.toLowerCase().includes("waiting");
    if (isWaiting) return;

    const interval = setInterval(() => {
      setDisplayPercent((prev) => {
        const ceiling = Math.min(percent + 65, 92);
        if (prev >= ceiling) return prev;
        return prev + (ceiling - prev) * 0.015;
      });
    }, 600);

    return () => clearInterval(interval);
  }, [percent, phase, hasRealBytes]);

  return displayPercent;
}

export function InlineUpdateProgress({
  phase,
  percent,
  variant = "compact",
  downloadedBytes,
  totalBytes,
}: InlineUpdateProgressProps) {
  const hasRealBytes = downloadedBytes != null && downloadedBytes > 0;

  // When we have real byte data with a known total, compute percent from bytes
  const effectivePercent =
    hasRealBytes && totalBytes != null && totalBytes > 0
      ? (downloadedBytes / totalBytes) * 100
      : percent;

  const displayPercent = useCrawlingPercent(effectivePercent, phase, hasRealBytes);

  const byteLabel = hasRealBytes ? formatDownloadProgress(downloadedBytes, totalBytes) : null;

  if (variant === "card") {
    return (
      <div className="flex items-center gap-2.5">
        <Loader2 className="h-3.5 w-3.5 shrink-0 animate-spin text-primary" />
        <div className="min-w-0 flex-1">
          <div className="mb-1 flex items-center justify-between">
            <span className="truncate text-xs text-muted-foreground">{phase}</span>
            <span className="ml-2 shrink-0 text-footnote tabular-nums text-muted-foreground">
              {byteLabel ?? `${Math.round(displayPercent)}%`}
            </span>
          </div>
          <div className="h-1.5 w-full overflow-hidden rounded-full bg-muted">
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

  // Compact variant for AppRow
  return (
    <div className={cn("flex w-[100px] items-center gap-1.5")}>
      <Loader2 className="h-3 w-3 shrink-0 animate-spin text-primary" />
      <div className="min-w-0 flex-1">
        <div className="h-1 w-full overflow-hidden rounded-full bg-muted">
          <motion.div
            className="h-full rounded-full bg-primary"
            initial={{ width: 0 }}
            animate={{ width: `${displayPercent}%` }}
            transition={{ duration: 0.3, ease: "easeOut" }}
          />
        </div>
        <span className="mt-0.5 block truncate text-caption leading-tight text-muted-foreground">
          {byteLabel ?? phase}
        </span>
      </div>
    </div>
  );
}

interface RelaunchButtonProps {
  bundleId: string;
  appPath: string;
  variant?: "compact" | "card";
}

export function RelaunchButton({ bundleId, appPath, variant = "compact" }: RelaunchButtonProps) {
  const clearRelaunch = useUpdateProgressStore((s) => s.clearRelaunch);
  const [isRelaunching, setIsRelaunching] = useState(false);

  const handleRelaunch = async () => {
    setIsRelaunching(true);
    try {
      await relaunchApp(bundleId, appPath);
    } finally {
      clearRelaunch(bundleId);
    }
  };

  const size = variant === "card" ? "px-2.5 py-1.5 text-xs" : "px-2 py-1 text-footnote";

  return (
    <button
      type="button"
      onClick={handleRelaunch}
      disabled={isRelaunching}
      className={cn(
        "flex items-center gap-1.5 rounded-md font-medium transition-colors",
        "bg-amber-500/15 text-amber-600 hover:bg-amber-500/25",
        "dark:text-amber-400",
        "disabled:opacity-50",
        size,
      )}
    >
      <RotateCcw className={cn("h-3 w-3", isRelaunching && "animate-spin")} />
      Relaunch
    </button>
  );
}
