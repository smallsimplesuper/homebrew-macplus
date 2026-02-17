import { AlertTriangle, Beer, ExternalLink, X } from "lucide-react";
import { useEffect, useState } from "react";
import { checkSetupStatus, openSystemPreferences } from "@/lib/tauri-commands";
import { cn } from "@/lib/utils";
import { useAppFilterStore } from "@/stores/appFilterStore";

type BannerKind = "homebrew" | "automation" | null;

const DISMISSED_KEY = "macplus-permission-banner-dismissed";

export function PermissionBanner() {
  const [dismissed, setDismissed] = useState(() => localStorage.getItem(DISMISSED_KEY) === "true");
  const [bannerKind, setBannerKind] = useState<BannerKind>(null);
  const setFilterView = useAppFilterStore((s) => s.setFilterView);

  useEffect(() => {
    checkSetupStatus()
      .then((status) => {
        if (!status.homebrewInstalled) {
          setBannerKind("homebrew");
        } else if (!status.permissions.automation) {
          setBannerKind("automation");
        }
      })
      .catch(() => {});
  }, []);

  if (dismissed || !bannerKind) return null;

  const openSetup = () => {
    setFilterView("settings");
    // Small delay so SettingsView mounts before we navigate to its tab
    setTimeout(() => {
      window.dispatchEvent(new CustomEvent("navigate-settings-tab", { detail: "setup" }));
    }, 50);
  };

  return (
    <div
      className={cn("flex items-center gap-3 border-b border-warning/20 bg-warning/5 px-4 py-2.5")}
    >
      {bannerKind === "homebrew" ? (
        <Beer className="h-4 w-4 shrink-0 text-warning" />
      ) : (
        <AlertTriangle className="h-4 w-4 shrink-0 text-warning" />
      )}
      <p className="flex-1 text-xs text-warning">
        {bannerKind === "homebrew"
          ? "Homebrew is not installed. Some CLI tool updates may require it."
          : "Automation permission required to quit apps before updating."}
      </p>
      {bannerKind === "homebrew" ? (
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
          onClick={() => openSystemPreferences("automation")}
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
        onClick={() => {
          localStorage.setItem(DISMISSED_KEY, "true");
          setDismissed(true);
        }}
        className="rounded-md p-1 text-warning/60 transition-colors hover:text-warning"
      >
        <X className="h-3.5 w-3.5" />
      </button>
    </div>
  );
}
