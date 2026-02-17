import { AlertTriangle, Beer, ExternalLink, ShieldAlert, X } from "lucide-react";
import { useEffect, useState } from "react";
import { checkSetupStatus, openSystemPreferences } from "@/lib/tauri-commands";
import { cn } from "@/lib/utils";
import { useAppFilterStore } from "@/stores/appFilterStore";

type BannerKind = "appManagement" | "automation" | "homebrew" | null;

const V2_KEY = "macplus-permission-banner-dismissed-v2";
const V1_KEY = "macplus-permission-banner-dismissed";

function loadDismissedKinds(): Set<string> {
  const v2 = localStorage.getItem(V2_KEY);
  if (v2) {
    try {
      return new Set(JSON.parse(v2));
    } catch {
      return new Set();
    }
  }
  // Migrate from v1 boolean key
  if (localStorage.getItem(V1_KEY) === "true") {
    const migrated = new Set(["homebrew", "automation"]);
    localStorage.setItem(V2_KEY, JSON.stringify([...migrated]));
    return migrated;
  }
  return new Set();
}

export function PermissionBanner() {
  const [dismissedKinds, setDismissedKinds] = useState<Set<string>>(loadDismissedKinds);
  const [bannerKind, setBannerKind] = useState<BannerKind>(null);
  const setFilterView = useAppFilterStore((s) => s.setFilterView);

  useEffect(() => {
    checkSetupStatus()
      .then((status) => {
        if (!status.permissions.appManagement) {
          setBannerKind("appManagement");
        } else if (!status.permissions.automation) {
          setBannerKind("automation");
        } else if (!status.homebrewInstalled) {
          setBannerKind("homebrew");
        }
      })
      .catch(() => {});
  }, []);

  if (!bannerKind || dismissedKinds.has(bannerKind)) return null;

  const dismiss = () => {
    const next = new Set(dismissedKinds);
    next.add(bannerKind);
    localStorage.setItem(V2_KEY, JSON.stringify([...next]));
    setDismissedKinds(next);
  };

  const openSetup = () => {
    setFilterView("settings");
    setTimeout(() => {
      window.dispatchEvent(new CustomEvent("navigate-settings-tab", { detail: "setup" }));
    }, 50);
  };

  const icon =
    bannerKind === "appManagement" ? (
      <ShieldAlert className="h-4 w-4 shrink-0 text-warning" />
    ) : bannerKind === "homebrew" ? (
      <Beer className="h-4 w-4 shrink-0 text-warning" />
    ) : (
      <AlertTriangle className="h-4 w-4 shrink-0 text-warning" />
    );

  const message =
    bannerKind === "appManagement"
      ? "App Management permission required to install and update apps in /Applications."
      : bannerKind === "automation"
        ? "Automation permission required to quit apps before updating."
        : "Homebrew is not installed. Some CLI tool updates may require it.";

  const action =
    bannerKind === "homebrew" ? (
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
        onClick={() =>
          openSystemPreferences(bannerKind === "appManagement" ? "app_management" : "automation")
        }
        className={cn(
          "flex items-center gap-1 rounded-md px-2.5 py-1",
          "bg-warning/10 text-xs font-medium text-warning",
          "transition-colors hover:bg-warning/20",
        )}
      >
        <ExternalLink className="h-3 w-3" />
        Open System Settings
      </button>
    );

  return (
    <div
      className={cn("flex items-center gap-3 border-b border-warning/20 bg-warning/5 px-4 py-2.5")}
    >
      {icon}
      <p className="flex-1 text-xs text-warning">{message}</p>
      {action}
      <button
        type="button"
        onClick={dismiss}
        className="rounded-md p-1 text-warning/60 transition-colors hover:text-warning"
      >
        <X className="h-3.5 w-3.5" />
      </button>
    </div>
  );
}
