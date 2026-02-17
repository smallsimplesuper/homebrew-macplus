import {
  Beer,
  CheckCircle2,
  ExternalLink,
  KeyRound,
  RefreshCw,
  ShieldCheck,
  Terminal,
  Wrench,
  XCircle,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import {
  checkSetupStatus,
  ensureAskpassHelper,
  openSystemPreferences,
  openTerminalWithCommand,
  type SetupStatus,
} from "@/lib/tauri-commands";
import { cn } from "@/lib/utils";

export function SetupView() {
  const [status, setStatus] = useState<SetupStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);
  const [configuringAskpass, setConfiguringAskpass] = useState(false);

  const refresh = useCallback(() => {
    setLoading(true);
    setError(false);
    checkSetupStatus()
      .then(setStatus)
      .catch(() => setError(true))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const handleInstallHomebrew = () => {
    openTerminalWithCommand(
      '/bin/bash -c \\"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\\"',
    );
  };

  const handleConfigureAskpass = async () => {
    setConfiguringAskpass(true);
    try {
      await ensureAskpassHelper();
      refresh();
    } catch {
      // ignore
    } finally {
      setConfiguringAskpass(false);
    }
  };

  if (loading && !status) {
    return (
      <div className="rounded-lg border border-border bg-background p-6">
        <p className="text-sm text-muted-foreground">Checking setup...</p>
      </div>
    );
  }

  if (error && !status) {
    return (
      <div className="rounded-lg border border-border bg-background p-6">
        <p className="text-sm text-muted-foreground">Failed to check setup status.</p>
        <button
          type="button"
          onClick={refresh}
          className={cn(
            "mt-2 flex items-center gap-1.5 rounded-md px-3 py-1.5",
            "text-xs font-medium text-foreground",
            "border border-border bg-background",
            "transition-colors hover:bg-muted",
          )}
        >
          <RefreshCw className="h-3 w-3" />
          Retry
        </button>
      </div>
    );
  }

  if (!status) return null;

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold text-foreground">Setup</h2>
        <button
          type="button"
          onClick={refresh}
          disabled={loading}
          className={cn(
            "flex items-center gap-1.5 rounded-md px-2.5 py-1",
            "text-xs font-medium text-muted-foreground",
            "transition-colors hover:text-foreground",
            "disabled:cursor-not-allowed disabled:opacity-50",
          )}
        >
          <RefreshCw className={cn("h-3.5 w-3.5", loading && "animate-spin")} />
          Refresh Status
        </button>
      </div>

      {/* Section 1 — Homebrew */}
      <div>
        <div className="mb-1 flex items-center gap-1.5">
          <Beer className="h-3.5 w-3.5 text-muted-foreground" />
          <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
            Homebrew (Optional)
          </h3>
        </div>
        <div className="space-y-1">
          <div className="flex items-center justify-between rounded-lg border border-border bg-background px-4 py-3">
            <div className="flex items-center gap-2">
              {status.homebrewInstalled ? (
                <CheckCircle2 className="h-4 w-4 text-green-500" />
              ) : (
                <XCircle className="h-4 w-4 text-destructive" />
              )}
              <div>
                <p className="text-sm font-medium text-foreground">Homebrew</p>
                {status.homebrewInstalled ? (
                  <p className="text-xs text-muted-foreground">
                    {status.homebrewVersion ?? "Installed"}{" "}
                    {status.homebrewPath && (
                      <span className="text-muted-foreground/60">({status.homebrewPath})</span>
                    )}
                  </p>
                ) : (
                  <p className="text-xs text-muted-foreground">
                    Optional — enables CLI tool updates and provides a fallback for some apps
                  </p>
                )}
              </div>
            </div>
            {!status.homebrewInstalled && (
              <button
                type="button"
                onClick={handleInstallHomebrew}
                className={cn(
                  "flex items-center gap-1 rounded-md px-2.5 py-1",
                  "bg-primary/10 text-xs font-medium text-primary",
                  "transition-colors hover:bg-primary/20",
                )}
              >
                <Terminal className="h-3 w-3" />
                Install Homebrew
              </button>
            )}
          </div>

          {/* Xcode CLT */}
          <div className="flex items-center justify-between rounded-lg border border-border bg-background px-4 py-3">
            <div className="flex items-center gap-2">
              {status.xcodeCltInstalled ? (
                <CheckCircle2 className="h-4 w-4 text-green-500" />
              ) : (
                <XCircle className="h-4 w-4 text-muted-foreground" />
              )}
              <div>
                <p className="text-sm font-medium text-foreground">Xcode Command Line Tools</p>
                <p className="text-xs text-muted-foreground">
                  {status.xcodeCltInstalled
                    ? "Installed"
                    : "Required for building Homebrew formulas from source"}
                </p>
              </div>
            </div>
            {!status.xcodeCltInstalled && (
              <button
                type="button"
                onClick={() => openTerminalWithCommand("xcode-select --install")}
                className={cn(
                  "flex items-center gap-1 rounded-md px-2.5 py-1",
                  "bg-muted text-xs font-medium text-muted-foreground",
                  "transition-colors hover:bg-muted/80",
                )}
              >
                <Terminal className="h-3 w-3" />
                Install
              </button>
            )}
          </div>
        </div>
      </div>

      {/* Section 2 — Permissions */}
      <div>
        <div className="mb-1 flex items-center gap-1.5">
          <ShieldCheck className="h-3.5 w-3.5 text-muted-foreground" />
          <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
            Permissions
          </h3>
        </div>
        <div className="space-y-1">
          {/* Automation */}
          <div className="flex items-center justify-between rounded-lg border border-border bg-background px-4 py-3">
            <div className="flex items-center gap-2">
              {status.permissions.automation ? (
                <CheckCircle2 className="h-4 w-4 text-green-500" />
              ) : (
                <XCircle className="h-4 w-4 text-destructive" />
              )}
              <div>
                <p className="text-sm font-medium text-foreground">Automation</p>
                <p className="text-xs text-muted-foreground">
                  Required to quit apps before updating
                </p>
              </div>
            </div>
            {!status.permissions.automation && (
              <button
                type="button"
                onClick={() => openSystemPreferences("automation")}
                className={cn(
                  "flex items-center gap-1 rounded-md px-2.5 py-1",
                  "bg-primary/10 text-xs font-medium text-primary",
                  "transition-colors hover:bg-primary/20",
                )}
              >
                <ExternalLink className="h-3 w-3" />
                Grant
              </button>
            )}
          </div>

          {/* App Management */}
          <div className="flex items-center justify-between rounded-lg border border-border bg-background px-4 py-3">
            <div className="flex items-center gap-2">
              {status.permissions.appManagement ? (
                <CheckCircle2 className="h-4 w-4 text-green-500" />
              ) : (
                <XCircle className="h-4 w-4 text-destructive" />
              )}
              <div>
                <p className="text-sm font-medium text-foreground">App Management</p>
                <p className="text-xs text-muted-foreground">
                  Required to install and update apps in /Applications
                </p>
              </div>
            </div>
            {!status.permissions.appManagement && (
              <button
                type="button"
                onClick={() => openSystemPreferences("app_management")}
                className={cn(
                  "flex items-center gap-1 rounded-md px-2.5 py-1",
                  "bg-primary/10 text-xs font-medium text-primary",
                  "transition-colors hover:bg-primary/20",
                )}
              >
                <ExternalLink className="h-3 w-3" />
                Grant
              </button>
            )}
          </div>

          {/* Full Disk Access */}
          <div className="flex items-center justify-between rounded-lg border border-border bg-background px-4 py-3">
            <div className="flex items-center gap-2">
              {status.permissions.fullDiskAccess ? (
                <CheckCircle2 className="h-4 w-4 text-green-500" />
              ) : (
                <XCircle className="h-4 w-4 text-muted-foreground" />
              )}
              <div>
                <p className="text-sm font-medium text-foreground">Full Disk Access</p>
                <p className="text-xs text-muted-foreground">
                  Optional, for scanning protected directories
                </p>
              </div>
            </div>
            {!status.permissions.fullDiskAccess && (
              <button
                type="button"
                onClick={() => openSystemPreferences("full_disk_access")}
                className={cn(
                  "flex items-center gap-1 rounded-md px-2.5 py-1",
                  "bg-muted text-xs font-medium text-muted-foreground",
                  "transition-colors hover:bg-muted/80",
                )}
              >
                <ExternalLink className="h-3 w-3" />
                Grant
              </button>
            )}
          </div>
        </div>
      </div>

      {/* Section 3 — Admin Helper */}
      <div>
        <div className="mb-1 flex items-center gap-1.5">
          <KeyRound className="h-3.5 w-3.5 text-muted-foreground" />
          <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
            Admin Helper
          </h3>
        </div>
        <div className="space-y-1">
          <div className="flex items-center justify-between rounded-lg border border-border bg-background px-4 py-3">
            <div className="flex items-center gap-2">
              {status.askpassInstalled ? (
                <CheckCircle2 className="h-4 w-4 text-green-500" />
              ) : (
                <Wrench className="h-4 w-4 text-yellow-500" />
              )}
              <div>
                <p className="text-sm font-medium text-foreground">Password Prompt Helper</p>
                <p className="text-xs text-muted-foreground">
                  {status.askpassInstalled
                    ? "Ready"
                    : "Allows macPlus to securely prompt for your password when apps need administrator access to install (e.g., driver packages)"}
                </p>
              </div>
            </div>
            {!status.askpassInstalled && (
              <button
                type="button"
                onClick={handleConfigureAskpass}
                disabled={configuringAskpass}
                className={cn(
                  "flex items-center gap-1 rounded-md px-2.5 py-1",
                  "bg-primary/10 text-xs font-medium text-primary",
                  "transition-colors hover:bg-primary/20",
                  "disabled:cursor-not-allowed disabled:opacity-50",
                )}
              >
                <Wrench className="h-3 w-3" />
                {configuringAskpass ? "Configuring..." : "Configure"}
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
