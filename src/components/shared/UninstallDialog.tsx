import { AnimatePresence, motion } from "framer-motion";
import { ChevronDown, ChevronRight, Loader2, ShieldAlert, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { AppIcon } from "@/components/app-list/AppIcon";
import { useUninstallApp } from "@/hooks/useApps";
import { springs } from "@/lib/animations";
import type { AssociatedFiles } from "@/lib/tauri-commands";
import { scanAssociatedFiles } from "@/lib/tauri-commands";
import { cn } from "@/lib/utils";
import { useUIStore } from "@/stores/uiStore";

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  const val = bytes / 1024 ** i;
  return `${val < 10 ? val.toFixed(1) : Math.round(val)} ${units[i]}`;
}

const KIND_LABELS: Record<string, string> = {
  application_support: "App Support",
  preferences: "Preferences",
  caches: "Caches",
  http_storages: "HTTP Storage",
  saved_state: "Saved State",
  containers: "Containers",
  group_containers: "Group Containers",
  logs: "Logs",
  webkit: "WebKit Data",
};

function getSourceLabel(source: string): string {
  switch (source) {
    case "homebrew":
      return "Homebrew Cask";
    case "homebrew_formula":
      return "Homebrew Formula";
    case "mas":
      return "Mac App Store";
    case "direct":
      return "Direct Install";
    default:
      return "Unknown Source";
  }
}

export function UninstallDialog() {
  const target = useUIStore((s) => s.uninstallTarget);
  const setUninstallTarget = useUIStore((s) => s.setUninstallTarget);
  const uninstall = useUninstallApp();

  const [associatedFiles, setAssociatedFiles] = useState<AssociatedFiles | null>(null);
  const [loadingFiles, setLoadingFiles] = useState(false);
  const [cleanupChecked, setCleanupChecked] = useState(false);
  const [filesExpanded, setFilesExpanded] = useState(false);

  const isSystemApp =
    target?.appPath.startsWith("/System/Applications/") ||
    target?.appPath.startsWith("/System/Library/") ||
    false;
  const isSelf = target?.bundleId === "com.macplus.app";
  const isProtected = isSystemApp || isSelf;

  // Scan associated files when dialog opens
  useEffect(() => {
    if (!target) {
      setAssociatedFiles(null);
      setCleanupChecked(false);
      setFilesExpanded(false);
      return;
    }

    setLoadingFiles(true);
    scanAssociatedFiles(target.bundleId)
      .then(setAssociatedFiles)
      .catch(() => setAssociatedFiles(null))
      .finally(() => setLoadingFiles(false));
  }, [target]);

  const handleClose = () => {
    setUninstallTarget(null);
  };

  const handleUninstall = () => {
    if (!target || isProtected) return;
    uninstall.mutate(
      { bundleId: target.bundleId, cleanupAssociated: cleanupChecked },
      { onSuccess: () => setUninstallTarget(null) },
    );
  };

  // Group files by kind for display
  const groupedFiles = associatedFiles?.paths.reduce(
    (acc, file) => {
      const kind = file.kind;
      if (!acc[kind]) acc[kind] = [];
      acc[kind].push(file);
      return acc;
    },
    {} as Record<string, typeof associatedFiles.paths>,
  );

  return (
    <AnimatePresence>
      {target && (
        <>
          {/* Backdrop */}
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={springs.macEase}
            className="fixed inset-0 z-50 bg-black/30"
            onClick={handleClose}
          />

          {/* Dialog */}
          <motion.div
            initial={{ opacity: 0, scale: 0.96, y: -8 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.96, y: -8 }}
            transition={springs.snappy}
            className="fixed left-1/2 top-[20%] z-50 w-[90%] max-w-[400px] -translate-x-1/2 rounded-xl border border-border bg-popover shadow-lg"
          >
            <div className="p-5">
              {/* App identity */}
              <div className="flex flex-col items-center gap-2 text-center">
                <AppIcon
                  iconPath={target.iconCachePath}
                  appPath={target.appPath}
                  displayName={target.displayName}
                  bundleId={target.bundleId}
                  size={64}
                />
                <h3 className="text-base font-semibold text-foreground">{target.displayName}</h3>
                {target.installedVersion && (
                  <span className="text-xs text-muted-foreground">v{target.installedVersion}</span>
                )}
                <span className="inline-block rounded-full bg-muted px-2 py-0.5 text-[10px] font-medium text-muted-foreground">
                  {getSourceLabel(target.installSource)}
                </span>
              </div>

              {/* Warning banners */}
              {isProtected && (
                <div className="mt-4 flex items-center gap-2 rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2">
                  <ShieldAlert className="h-4 w-4 shrink-0 text-destructive" />
                  <span className="text-xs text-destructive">
                    {isSelf
                      ? "macPlus cannot uninstall itself."
                      : "This is a system app and cannot be uninstalled."}
                  </span>
                </div>
              )}

              {!isProtected && (
                <>
                  <p className="mt-4 text-center text-xs text-muted-foreground">
                    This will move <strong>{target.displayName}</strong> to the Trash.
                  </p>

                  {/* Associated files section */}
                  <div className="mt-4">
                    {loadingFiles ? (
                      <div className="flex items-center gap-2 text-xs text-muted-foreground">
                        <Loader2 className="h-3 w-3 animate-spin" />
                        Scanning for associated files...
                      </div>
                    ) : associatedFiles && associatedFiles.paths.length > 0 ? (
                      <div className="space-y-2">
                        <label className="flex items-center gap-2 cursor-pointer">
                          <input
                            type="checkbox"
                            checked={cleanupChecked}
                            onChange={(e) => setCleanupChecked(e.target.checked)}
                            className="h-3.5 w-3.5 rounded border-border accent-primary"
                          />
                          <span className="text-xs text-foreground">
                            Also remove associated data (
                            {formatBytes(associatedFiles.totalSizeBytes)})
                          </span>
                        </label>

                        {cleanupChecked && (
                          <div>
                            <button
                              type="button"
                              onClick={() => setFilesExpanded((v) => !v)}
                              className="flex items-center gap-1 text-[10px] text-muted-foreground hover:text-foreground transition-colors"
                            >
                              {filesExpanded ? (
                                <ChevronDown className="h-3 w-3" />
                              ) : (
                                <ChevronRight className="h-3 w-3" />
                              )}
                              {associatedFiles.paths.length} file
                              {associatedFiles.paths.length === 1 ? "" : "s"} found
                            </button>
                            <AnimatePresence>
                              {filesExpanded && groupedFiles && (
                                <motion.div
                                  initial={{ height: 0, opacity: 0 }}
                                  animate={{ height: "auto", opacity: 1 }}
                                  exit={{ height: 0, opacity: 0 }}
                                  transition={{ duration: 0.15 }}
                                  className="overflow-hidden"
                                >
                                  <div className="mt-1.5 max-h-[140px] overflow-y-auto rounded-md border border-border bg-muted/50 p-2 space-y-1.5">
                                    {Object.entries(groupedFiles).map(([kind, files]) => (
                                      <div key={kind}>
                                        <p className="text-[10px] font-semibold uppercase tracking-wider text-muted-foreground">
                                          {KIND_LABELS[kind] ?? kind}
                                        </p>
                                        {files.map((f) => (
                                          <div
                                            key={f.path}
                                            className="flex items-center justify-between text-[10px] text-muted-foreground"
                                          >
                                            <span className="truncate mr-2">
                                              {f.path.replace(/^.*\/Library\//, "~/Library/")}
                                            </span>
                                            <span className="shrink-0 tabular-nums">
                                              {formatBytes(f.sizeBytes)}
                                            </span>
                                          </div>
                                        ))}
                                      </div>
                                    ))}
                                  </div>
                                </motion.div>
                              )}
                            </AnimatePresence>
                          </div>
                        )}
                      </div>
                    ) : !loadingFiles && associatedFiles ? (
                      <p className="text-[10px] text-muted-foreground/60">
                        No associated data found.
                      </p>
                    ) : null}
                  </div>
                </>
              )}
            </div>

            {/* Buttons */}
            <div className="flex justify-end gap-2 border-t border-border px-5 py-3">
              <button
                type="button"
                onClick={handleClose}
                className={cn(
                  "rounded-lg border border-border bg-background px-4 py-1.5",
                  "text-xs font-medium text-foreground",
                  "transition-colors hover:bg-muted",
                )}
              >
                Cancel
              </button>
              {!isProtected && (
                <button
                  type="button"
                  onClick={handleUninstall}
                  disabled={uninstall.isPending}
                  className={cn(
                    "flex items-center gap-1.5 rounded-lg px-4 py-1.5",
                    "bg-destructive text-destructive-foreground",
                    "text-xs font-medium",
                    "transition-colors hover:bg-destructive/90",
                    "disabled:opacity-50 disabled:cursor-not-allowed",
                  )}
                >
                  {uninstall.isPending ? (
                    <Loader2 className="h-3 w-3 animate-spin" />
                  ) : (
                    <Trash2 className="h-3 w-3" />
                  )}
                  Move to Trash
                </button>
              )}
            </div>
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}
