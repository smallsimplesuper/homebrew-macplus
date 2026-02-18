import { Bug, Eye, EyeOff, FolderOpen, PackageMinus, Play, RefreshCw, X } from "lucide-react";
import { AnimatePresence, motion } from "motion/react";
import { useState } from "react";
import { AppIcon } from "@/components/app-list/AppIcon";
import { useAppDetail, useToggleIgnored } from "@/hooks/useApps";
import { useCheckSingleUpdate } from "@/hooks/useAppUpdates";
import { springs } from "@/lib/animations";
import type { UpdateCheckDiagnostic } from "@/lib/tauri-commands";
import { debugUpdateCheck, openApp, revealInFinder } from "@/lib/tauri-commands";
import { cn } from "@/lib/utils";
import { useUIStore } from "@/stores/uiStore";
import { AppInfoSection } from "./AppInfoSection";
import { ReleaseNotesSection } from "./ReleaseNotesSection";
import { VersionHistorySection } from "./VersionHistorySection";

const SOURCE_LABELS: Record<string, string> = {
  sparkle: "Sparkle",
  homebrew_cask: "Homebrew",
  homebrew_api: "Homebrew API",
  mas: "Mac App Store",
  github: "GitHub",
  electron: "Electron",
  keystone: "Keystone",
  microsoft_autoupdate: "Microsoft AutoUpdate",
  jetbrains_toolbox: "JetBrains Toolbox",
  adobe_cc: "Adobe CC",
};

function formatSourceType(source: string): string {
  return SOURCE_LABELS[source] ?? source;
}

export function AppDetailSheet() {
  const selectedAppId = useUIStore((s) => s.selectedAppId);
  const detailOpen = useUIStore((s) => s.detailOpen);
  const setDetailOpen = useUIStore((s) => s.setDetailOpen);

  const { data: detail, isLoading } = useAppDetail(selectedAppId);
  const toggleIgnored = useToggleIgnored();
  const checkUpdate = useCheckSingleUpdate();
  const [debugResult, setDebugResult] = useState<UpdateCheckDiagnostic | null>(null);
  const [debugLoading, setDebugLoading] = useState(false);

  const handleClose = () => setDetailOpen(false);

  const handleOpen = () => {
    if (detail?.appPath) {
      openApp(detail.appPath);
    }
  };

  const handleReveal = () => {
    if (detail?.appPath) {
      revealInFinder(detail.appPath);
    }
  };

  const handleCheckUpdate = () => {
    if (detail?.bundleId) {
      checkUpdate.mutate(detail.bundleId);
    }
  };

  const handleDebug = async () => {
    if (detail?.bundleId) {
      setDebugLoading(true);
      try {
        const result = await debugUpdateCheck(detail.bundleId);
        setDebugResult(result);
      } catch (e) {
        console.error("Debug check failed:", e);
      } finally {
        setDebugLoading(false);
      }
    }
  };

  const handleToggleIgnore = () => {
    if (detail) {
      toggleIgnored.mutate({
        bundleId: detail.bundleId,
        ignored: !detail.isIgnored,
      });
    }
  };

  return (
    <AnimatePresence mode="wait">
      {detailOpen && (
        <motion.div
          key="app-detail-sheet"
          initial={{ x: "100%" }}
          animate={{ x: 0 }}
          exit={{ x: "100%" }}
          transition={springs.default}
          className={cn(
            "fixed right-0 top-0 z-40 h-full w-full max-w-[420px]",
            "border-l border-border bg-card/95 backdrop-blur-xl",
            "flex flex-col shadow-xl",
          )}
        >
          {/* Header */}
          <div className="flex items-center justify-between border-b border-border px-4 py-3">
            <h2 className="text-sm font-semibold text-foreground">App Details</h2>
            <button
              type="button"
              onClick={handleClose}
              className={cn(
                "rounded-md p-1.5 text-muted-foreground",
                "transition-colors hover:bg-muted hover:text-foreground",
              )}
            >
              <X className="h-4 w-4" />
            </button>
          </div>

          {/* Content */}
          <div className="flex-1 overflow-y-auto px-4 py-4">
            {isLoading && (
              <div className="flex items-center justify-center py-12">
                <RefreshCw className="h-5 w-5 animate-spin text-muted-foreground" />
              </div>
            )}

            {!isLoading && !detail && (
              <p className="py-12 text-center text-sm text-muted-foreground">App not found</p>
            )}

            {detail && (
              <div className="space-y-6">
                {/* App identity */}
                <div className="flex flex-col items-center gap-3 text-center">
                  <AppIcon
                    iconPath={detail.iconCachePath}
                    appPath={detail.appPath}
                    displayName={detail.displayName}
                    bundleId={detail.bundleId}
                    size={64}
                  />
                  <div>
                    <h3 className="text-base font-semibold text-foreground">
                      {detail.displayName}
                    </h3>
                    <p className="text-xs text-muted-foreground">{detail.bundleId}</p>
                  </div>
                  <p className="text-xs text-muted-foreground truncate max-w-full">
                    {detail.appPath}
                  </p>
                  {detail.installedVersion && (
                    <span className="inline-block rounded-full bg-muted px-2.5 py-0.5 text-xs font-medium text-foreground">
                      v{detail.installedVersion}
                    </span>
                  )}
                  {detail.isIgnored && (
                    <span className="inline-block rounded-full bg-warning/10 px-2.5 py-0.5 text-xs font-medium text-warning">
                      Ignored
                    </span>
                  )}
                </div>

                {/* Available update banner */}
                {detail.availableUpdate && (
                  <div className="rounded-lg border border-primary/20 bg-primary/5 p-3">
                    <p className="text-xs font-medium text-primary">
                      Update available: v{detail.availableUpdate.availableVersion}
                      {detail.availableUpdate.sourceType && (
                        <span className="ml-1.5 text-primary/70">
                          via {formatSourceType(detail.availableUpdate.sourceType)}
                        </span>
                      )}
                    </p>
                  </div>
                )}

                {/* Action buttons */}
                <div className="grid grid-cols-2 gap-2">
                  <button
                    type="button"
                    onClick={handleOpen}
                    className={cn(
                      "flex items-center justify-center gap-2 rounded-lg",
                      "border border-border bg-background px-3 py-2",
                      "text-xs font-medium text-foreground",
                      "transition-colors hover:bg-muted",
                    )}
                  >
                    <Play className="h-3.5 w-3.5" />
                    Open
                  </button>
                  <button
                    type="button"
                    onClick={handleReveal}
                    className={cn(
                      "flex items-center justify-center gap-2 rounded-lg",
                      "border border-border bg-background px-3 py-2",
                      "text-xs font-medium text-foreground",
                      "transition-colors hover:bg-muted",
                    )}
                  >
                    <FolderOpen className="h-3.5 w-3.5" />
                    Reveal in Finder
                  </button>
                  <button
                    type="button"
                    onClick={handleCheckUpdate}
                    disabled={checkUpdate.isPending}
                    className={cn(
                      "flex items-center justify-center gap-2 rounded-lg",
                      "border border-border bg-background px-3 py-2",
                      "text-xs font-medium text-foreground",
                      "transition-colors hover:bg-muted",
                      "disabled:opacity-50 disabled:cursor-not-allowed",
                    )}
                  >
                    <RefreshCw
                      className={cn("h-3.5 w-3.5", checkUpdate.isPending && "animate-spin")}
                    />
                    Check for Update
                  </button>
                  <button
                    type="button"
                    onClick={handleToggleIgnore}
                    disabled={toggleIgnored.isPending}
                    className={cn(
                      "flex items-center justify-center gap-2 rounded-lg",
                      "border border-border bg-background px-3 py-2",
                      "text-xs font-medium text-foreground",
                      "transition-colors hover:bg-muted",
                      "disabled:opacity-50 disabled:cursor-not-allowed",
                    )}
                  >
                    {detail.isIgnored ? (
                      <>
                        <Eye className="h-3.5 w-3.5" />
                        Unignore
                      </>
                    ) : (
                      <>
                        <EyeOff className="h-3.5 w-3.5" />
                        Ignore
                      </>
                    )}
                  </button>
                  <button
                    type="button"
                    onClick={() =>
                      useUIStore.getState().setUninstallTarget({
                        bundleId: detail.bundleId,
                        displayName: detail.displayName,
                        appPath: detail.appPath,
                        installSource: detail.installSource,
                        iconCachePath: detail.iconCachePath,
                        installedVersion: detail.installedVersion,
                        homebrewCaskToken: detail.homebrewCaskToken ?? null,
                        homebrewFormulaName: detail.homebrewFormulaName ?? null,
                      })
                    }
                    className={cn(
                      "flex items-center justify-center gap-2 rounded-lg",
                      "border border-destructive/30 bg-background px-3 py-2",
                      "text-xs font-medium text-destructive",
                      "transition-colors hover:bg-destructive/10",
                    )}
                  >
                    <PackageMinus className="h-3.5 w-3.5" />
                    Uninstall
                  </button>
                  <button
                    type="button"
                    onClick={handleDebug}
                    disabled={debugLoading}
                    className={cn(
                      "flex items-center justify-center gap-2 rounded-lg",
                      "border border-border bg-background px-3 py-2",
                      "text-xs font-medium text-foreground",
                      "transition-colors hover:bg-muted",
                      "disabled:opacity-50 disabled:cursor-not-allowed",
                    )}
                  >
                    <Bug className={cn("h-3.5 w-3.5", debugLoading && "animate-spin")} />
                    Debug Update Check
                  </button>
                </div>

                {/* Debug output */}
                {debugResult && (
                  <div className="rounded-lg border border-border bg-muted/50 p-3 space-y-2">
                    <p className="text-xs font-semibold text-foreground">Debug Results</p>
                    <div className="space-y-1 text-caption">
                      <p className="text-muted-foreground">
                        Version: {debugResult.installedVersion ?? "unknown"} | Source:{" "}
                        {debugResult.installSource}
                      </p>
                      {debugResult.homebrewCaskToken && (
                        <p className="text-muted-foreground">
                          Cask token: {debugResult.homebrewCaskToken}
                        </p>
                      )}
                    </div>
                    <div className="space-y-0.5">
                      {debugResult.checkersTried.map((c) => (
                        <div
                          key={c.source}
                          className={cn(
                            "flex items-center justify-between text-caption px-2 py-0.5 rounded",
                            c.result.startsWith("found") && "bg-success/10 text-success",
                            c.result === "not_found" && "text-muted-foreground",
                            c.result === "skipped" && "text-muted-foreground/50",
                            c.result.startsWith("error") && "bg-destructive/10 text-destructive",
                          )}
                        >
                          <span>{c.source}</span>
                          <span className="font-mono">{c.result}</span>
                        </div>
                      ))}
                    </div>
                  </div>
                )}

                {/* Info section */}
                <AppInfoSection detail={detail} />

                {/* Release notes */}
                <ReleaseNotesSection
                  releaseNotesUrl={detail.availableUpdate?.releaseNotesUrl ?? null}
                  releaseNotes={detail.availableUpdate?.releaseNotes ?? null}
                />

                {/* Version history */}
                <VersionHistorySection />
              </div>
            )}
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
