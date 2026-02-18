import { isPermissionGranted } from "@tauri-apps/plugin-notification";
import {
  Beer,
  Bell,
  CheckCircle2,
  ExternalLink,
  FolderOpen,
  Globe,
  Info,
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

function StatusIcon({ ok, optional }: { ok: boolean; optional?: boolean }) {
  if (ok) return <CheckCircle2 className="h-4 w-4 text-green-500" />;
  if (optional) return <XCircle className="h-4 w-4 text-muted-foreground" />;
  return <XCircle className="h-4 w-4 text-destructive" />;
}

function SetupRow({
  ok,
  optional,
  label,
  description,
  action,
}: {
  ok: boolean;
  optional?: boolean;
  label: string;
  description: string;
  action?: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between rounded-lg border border-border bg-background px-4 py-3">
      <div className="flex items-center gap-2">
        <StatusIcon ok={ok} optional={optional} />
        <div>
          <p className="text-sm font-medium text-foreground">{label}</p>
          <p className="text-xs text-muted-foreground">{description}</p>
        </div>
      </div>
      {action}
    </div>
  );
}

function ActionButton({
  onClick,
  icon,
  label,
  variant = "primary",
  disabled,
}: {
  onClick: () => void;
  icon: React.ReactNode;
  label: string;
  variant?: "primary" | "muted";
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={cn(
        "flex items-center gap-1 rounded-md px-2.5 py-1",
        "text-xs font-medium transition-colors",
        "disabled:cursor-not-allowed disabled:opacity-50",
        variant === "primary"
          ? "bg-primary/10 text-primary hover:bg-primary/20"
          : "bg-muted text-muted-foreground hover:bg-muted/80",
      )}
    >
      {icon}
      {label}
    </button>
  );
}

function SectionHeader({ icon, title }: { icon: React.ReactNode; title: string }) {
  return (
    <div className="mb-1 flex items-center gap-1.5">
      {icon}
      <h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
        {title}
      </h3>
    </div>
  );
}

export function SetupView() {
  const [status, setStatus] = useState<SetupStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(false);
  const [configuringAskpass, setConfiguringAskpass] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(false);
    try {
      const result = await checkSetupStatus();
      try {
        const notifGranted = await isPermissionGranted();
        result.permissions.notifications = notifGranted;
      } catch {
        // Fall back to plist-based check
      }
      setStatus(result);
    } catch {
      setError(true);
    } finally {
      setLoading(false);
    }
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
        <div className="flex items-center gap-2">
          <RefreshCw className="h-3.5 w-3.5 animate-spin text-muted-foreground" />
          <p className="text-sm text-muted-foreground">Checking setup...</p>
        </div>
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

      {/* Section 1 — Permissions */}
      <div>
        <SectionHeader
          icon={<ShieldCheck className="h-3.5 w-3.5 text-muted-foreground" />}
          title="Permissions"
        />
        <div className="space-y-1">
          <SetupRow
            ok={status.permissions.appManagement}
            label="App Management"
            description={
              status.permissions.appManagement
                ? "Granted"
                : "Required to install and update apps in /Applications"
            }
            action={
              !status.permissions.appManagement ? (
                <ActionButton
                  onClick={() => openSystemPreferences("app_management")}
                  icon={<ExternalLink className="h-3 w-3" />}
                  label="Grant"
                />
              ) : undefined
            }
          />
          <SetupRow
            ok={status.permissions.automation}
            label="Automation"
            description={
              status.permissions.automationState === "granted"
                ? "Granted"
                : status.permissions.automationState === "denied"
                  ? "Denied — open System Settings to grant"
                  : "Not yet requested — use the banner above to enable"
            }
            action={
              !status.permissions.automation ? (
                <ActionButton
                  onClick={() => openSystemPreferences("automation")}
                  icon={<ExternalLink className="h-3 w-3" />}
                  label="Grant"
                />
              ) : undefined
            }
          />
          <SetupRow
            ok={status.permissions.notifications}
            label="Notifications"
            description={
              status.permissions.notifications ? "Granted" : "Required for background update alerts"
            }
            action={
              !status.permissions.notifications ? (
                <ActionButton
                  onClick={() => openSystemPreferences("notifications")}
                  icon={<Bell className="h-3 w-3" />}
                  label="Grant"
                />
              ) : undefined
            }
          />
          <SetupRow
            ok={status.permissions.fullDiskAccess}
            optional
            label="Full Disk Access"
            description={
              status.permissions.fullDiskAccess
                ? "Granted"
                : "Optional — for scanning protected directories"
            }
            action={
              !status.permissions.fullDiskAccess ? (
                <ActionButton
                  onClick={() => openSystemPreferences("full_disk_access")}
                  icon={<ExternalLink className="h-3 w-3" />}
                  label="Grant"
                  variant="muted"
                />
              ) : undefined
            }
          />
        </div>
      </div>

      {/* Section 2 — Connectivity */}
      <div>
        <SectionHeader
          icon={<Globe className="h-3.5 w-3.5 text-muted-foreground" />}
          title="Connectivity"
        />
        <div className="space-y-1">
          <SetupRow
            ok={status.connectivity.github}
            label="GitHub API"
            description={
              status.connectivity.github ? "Reachable" : "Required for GitHub release checks"
            }
          />
          <SetupRow
            ok={status.connectivity.homebrew}
            label="Homebrew API"
            description={
              status.connectivity.homebrew
                ? "Reachable"
                : "Required for Homebrew cask version index"
            }
          />
          <SetupRow
            ok={status.connectivity.itunes}
            label="iTunes API"
            description={
              status.connectivity.itunes ? "Reachable" : "Required for Mac App Store update checks"
            }
          />
        </div>
      </div>

      {/* Section 3 — Tools */}
      <div>
        <SectionHeader
          icon={<Beer className="h-3.5 w-3.5 text-muted-foreground" />}
          title="Tools (Optional)"
        />
        <div className="space-y-1">
          <SetupRow
            ok={status.homebrewInstalled}
            optional
            label="Homebrew"
            description={
              status.homebrewInstalled
                ? `${status.homebrewVersion ?? "Installed"}${status.homebrewPath ? ` (${status.homebrewPath})` : ""}`
                : "Optional — enables CLI tool updates and provides a fallback for some apps"
            }
            action={
              !status.homebrewInstalled ? (
                <ActionButton
                  onClick={handleInstallHomebrew}
                  icon={<Terminal className="h-3 w-3" />}
                  label="Install Homebrew"
                  variant="muted"
                />
              ) : undefined
            }
          />
          <SetupRow
            ok={status.xcodeCltInstalled}
            optional
            label="Xcode Command Line Tools"
            description={
              status.xcodeCltInstalled
                ? "Installed"
                : "Optional — required for building Homebrew formulas from source"
            }
            action={
              !status.xcodeCltInstalled ? (
                <ActionButton
                  onClick={() => openTerminalWithCommand("xcode-select --install")}
                  icon={<Terminal className="h-3 w-3" />}
                  label="Install"
                  variant="muted"
                />
              ) : undefined
            }
          />
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
                    : "Allows macPlus to securely prompt for your password when apps need administrator access"}
                </p>
              </div>
            </div>
            {!status.askpassInstalled && (
              <ActionButton
                onClick={handleConfigureAskpass}
                disabled={configuringAskpass}
                icon={<Wrench className="h-3 w-3" />}
                label={configuringAskpass ? "Configuring..." : "Configure"}
              />
            )}
          </div>
        </div>
      </div>

      {/* Section 4 — App Info */}
      <div>
        <SectionHeader
          icon={<Info className="h-3.5 w-3.5 text-muted-foreground" />}
          title="App Info"
        />
        <div className="space-y-1">
          <div className="flex items-center justify-between rounded-lg border border-border bg-background px-4 py-3">
            <div className="flex items-center gap-2">
              <FolderOpen className="h-4 w-4 text-muted-foreground" />
              <div>
                <p className="text-sm font-medium text-foreground">Version</p>
                <p className="text-xs text-muted-foreground">v{__APP_VERSION__}</p>
              </div>
            </div>
          </div>
          <div className="flex items-center justify-between rounded-lg border border-border bg-background px-4 py-3">
            <div className="flex items-center gap-2">
              <KeyRound className="h-4 w-4 text-muted-foreground" />
              <div>
                <p className="text-sm font-medium text-foreground">Data Path</p>
                <p className="text-xs text-muted-foreground">
                  ~/Library/Application Support/com.macplus.app
                </p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
