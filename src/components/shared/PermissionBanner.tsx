import { AlertTriangle, Beer, Bell, ExternalLink, ShieldAlert, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { checkSetupStatus, openSystemPreferences } from "@/lib/tauri-commands";
import { cn } from "@/lib/utils";
import { useAppFilterStore } from "@/stores/appFilterStore";

type BannerKind = "appManagement" | "automation" | "notifications" | "homebrew";

interface BannerConfig {
  kind: BannerKind;
  icon: React.ReactNode;
  message: string;
  pane?: string;
  isSetupLink?: boolean;
}

const DISMISS_KEY = "macplus-permission-banner-dismissed-v3";

function loadDismissedKinds(): Set<string> {
  const raw = localStorage.getItem(DISMISS_KEY);
  if (raw) {
    try {
      return new Set(JSON.parse(raw));
    } catch {
      return new Set();
    }
  }
  return new Set();
}

function saveDismissedKinds(kinds: Set<string>) {
  localStorage.setItem(DISMISS_KEY, JSON.stringify([...kinds]));
}

export function PermissionBanner() {
  const [dismissedKinds, setDismissedKinds] = useState<Set<string>>(loadDismissedKinds);
  const [missingBanners, setMissingBanners] = useState<BannerConfig[]>([]);
  const setFilterView = useAppFilterStore((s) => s.setFilterView);

  const checkPermissions = useCallback(() => {
    checkSetupStatus()
      .then((status) => {
        const banners: BannerConfig[] = [];
        if (!status.permissions.appManagement) {
          banners.push({
            kind: "appManagement",
            icon: <ShieldAlert className="h-4 w-4 shrink-0 text-warning" />,
            message:
              "App Management permission required to install and update apps in /Applications.",
            pane: "app_management",
          });
        }
        if (!status.permissions.automation) {
          banners.push({
            kind: "automation",
            icon: <AlertTriangle className="h-4 w-4 shrink-0 text-warning" />,
            message: "Automation permission required to quit apps before updating.",
            pane: "automation",
          });
        }
        if (!status.permissions.notifications) {
          banners.push({
            kind: "notifications",
            icon: <Bell className="h-4 w-4 shrink-0 text-warning" />,
            message: "Notification permission required for background update alerts.",
            pane: "notifications",
          });
        }
        if (!status.homebrewInstalled) {
          banners.push({
            kind: "homebrew",
            icon: <Beer className="h-4 w-4 shrink-0 text-warning" />,
            message: "Homebrew is not installed. Some CLI tool updates may require it.",
            isSetupLink: true,
          });
        }
        setMissingBanners(banners);

        // Clear dismissed state for permissions that are now granted
        const currentMissing = new Set(banners.map((b) => b.kind));
        const next = new Set(dismissedKinds);
        let changed = false;
        for (const kind of dismissedKinds) {
          if (!currentMissing.has(kind as BannerKind)) {
            next.delete(kind);
            changed = true;
          }
        }
        if (changed) {
          saveDismissedKinds(next);
          setDismissedKinds(next);
        }
      })
      .catch(() => {});
  }, [dismissedKinds]);

  useEffect(() => {
    checkPermissions();
    // Re-check on window focus (user may have just granted permission)
    const onFocus = () => checkPermissions();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [checkPermissions]);

  const dismiss = (kind: BannerKind) => {
    const next = new Set(dismissedKinds);
    next.add(kind);
    saveDismissedKinds(next);
    setDismissedKinds(next);
  };

  const openSetup = () => {
    setFilterView("settings");
    setTimeout(() => {
      window.dispatchEvent(new CustomEvent("navigate-settings-tab", { detail: "setup" }));
    }, 50);
  };

  const visible = missingBanners.filter((b) => !dismissedKinds.has(b.kind));
  if (visible.length === 0) return null;

  return (
    <div className="divide-y divide-warning/10">
      {visible.map((banner) => (
        <div
          key={banner.kind}
          className={cn(
            "flex items-center gap-3 border-b border-warning/20 bg-warning/5 px-4 py-2",
          )}
        >
          {banner.icon}
          <p className="flex-1 text-xs text-warning">{banner.message}</p>
          {banner.isSetupLink ? (
            <button
              type="button"
              onClick={openSetup}
              className={cn(
                "flex items-center gap-1 rounded-md px-2.5 py-1",
                "bg-warning/10 text-xs font-medium text-warning",
                "transition-colors hover:bg-warning/20",
              )}
            >
              <ExternalLink className="h-3 w-3" />
              Open Setup
            </button>
          ) : (
            <button
              type="button"
              onClick={() => openSystemPreferences(banner.pane!)}
              className={cn(
                "flex items-center gap-1 rounded-md px-2.5 py-1",
                "bg-warning/10 text-xs font-medium text-warning",
                "transition-colors hover:bg-warning/20",
              )}
            >
              <ExternalLink className="h-3 w-3" />
              Open System Settings
            </button>
          )}
          <button
            type="button"
            onClick={() => dismiss(banner.kind)}
            className="rounded-md p-1 text-warning/60 transition-colors hover:text-warning"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
      ))}
    </div>
  );
}
