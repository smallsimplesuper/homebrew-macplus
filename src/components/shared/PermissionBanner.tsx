import { isPermissionGranted, requestPermission } from "@tauri-apps/plugin-notification";
import { useCallback, useEffect, useState } from "react";
import { usePermissionRefresh } from "@/hooks/usePermissionRefresh";
import {
  getPermissionsPassive,
  openSystemPreferences,
  type PermissionsStatus,
  triggerAutomationPermission,
} from "@/lib/tauri-commands";
import { cn } from "@/lib/utils";

type PermKind = "appManagement" | "automation" | "notifications" | "fullDiskAccess";

interface PermItem {
  kind: PermKind;
  label: string;
  state: "granted" | "denied" | "unknown";
}

function dotColor(state: string) {
  if (state === "granted") return "bg-green-500";
  if (state === "denied") return "bg-red-500";
  return "bg-muted-foreground/40";
}

const STORAGE_KEY = "macplus-permissions-granted";

let _allGranted = false;

/** Returns whether all 4 required permissions are granted. Subscribable from other components. */
export function usePermissionsGranted() {
  const [granted, setGranted] = useState(_allGranted);
  useEffect(() => {
    const handler = () => setGranted(_allGranted);
    window.addEventListener("permissions-changed", handler);
    return () => window.removeEventListener("permissions-changed", handler);
  }, []);
  return granted;
}

export function PermissionBanner() {
  const [perms, setPerms] = useState<PermissionsStatus | null>(null);
  const [triggeringAutomation, setTriggeringAutomation] = useState(false);
  const [cachedGranted, setCachedGranted] = useState(() => {
    try {
      return localStorage.getItem(STORAGE_KEY) === "true";
    } catch {
      return false;
    }
  });
  const updateAllGranted = useCallback((status: PermissionsStatus) => {
    const allOk =
      status.appManagement && status.automation && status.notifications && status.fullDiskAccess;
    if (allOk) {
      try {
        localStorage.setItem(STORAGE_KEY, "true");
      } catch {}
      setCachedGranted(true);
    } else {
      try {
        localStorage.removeItem(STORAGE_KEY);
      } catch {}
      setCachedGranted(false);
    }
    if (_allGranted !== allOk) {
      _allGranted = allOk;
      window.dispatchEvent(new Event("permissions-changed"));
    }
  }, []);

  const refresh = useCallback(() => {
    getPermissionsPassive()
      .then(async (status) => {
        // Override plist-based notification check with the reliable native API
        try {
          const notifGranted = await isPermissionGranted();
          status.notifications = notifGranted;
        } catch {
          // Fall back to plist-based check if plugin fails
        }
        setPerms(status);
        updateAllGranted(status);
      })
      .catch(() => {});
  }, [updateAllGranted]);

  const { startPolling, stopPolling } = usePermissionRefresh(refresh);

  // Initial check on mount
  useEffect(() => {
    refresh();
  }, [refresh]);

  // Stop polling once all permissions are granted
  useEffect(() => {
    if (
      perms?.appManagement &&
      perms?.automation &&
      perms?.notifications &&
      perms?.fullDiskAccess
    ) {
      stopPolling();
    }
  }, [perms, stopPolling]);

  // If localStorage says all granted and we haven't loaded fresh data yet, hide banner
  if (!perms && cachedGranted) return null;
  if (!perms) return null;

  const items: PermItem[] = [
    {
      kind: "appManagement",
      label: "App Management",
      state: perms.appManagement ? "granted" : "denied",
    },
    {
      kind: "automation",
      label: "Automation",
      state: perms.automationState,
    },
    {
      kind: "notifications",
      label: "Notifications",
      state: perms.notifications ? "granted" : "denied",
    },
    {
      kind: "fullDiskAccess",
      label: "Drive Access",
      state: perms.fullDiskAccess ? "granted" : "denied",
    },
  ];

  const allGranted = items.every((i) => i.state === "granted");
  if (allGranted) return null;

  const handleEnable = async (kind: PermKind) => {
    if (kind === "automation") {
      setTriggeringAutomation(true);
      try {
        const granted = await triggerAutomationPermission();
        setPerms((prev) =>
          prev
            ? {
                ...prev,
                automation: granted,
                automationState: granted ? "granted" : "denied",
              }
            : prev,
        );
        if (granted && perms) {
          const allOk = perms.appManagement && perms.notifications && perms.fullDiskAccess;
          if (!_allGranted && allOk) {
            _allGranted = true;
            try {
              localStorage.setItem(STORAGE_KEY, "true");
            } catch {}
            setCachedGranted(true);
            window.dispatchEvent(new Event("permissions-changed"));
          }
        }
      } finally {
        setTriggeringAutomation(false);
      }
    } else if (kind === "appManagement") {
      await openSystemPreferences("app_management");
      startPolling();
    } else if (kind === "notifications") {
      try {
        const result = await requestPermission();
        if (result === "granted") {
          setPerms((prev) => (prev ? { ...prev, notifications: true } : prev));
          if (perms) {
            const allOk = perms.appManagement && perms.automation && perms.fullDiskAccess;
            if (!_allGranted && allOk) {
              _allGranted = true;
              try {
                localStorage.setItem(STORAGE_KEY, "true");
              } catch {}
              setCachedGranted(true);
              window.dispatchEvent(new Event("permissions-changed"));
            }
          }
          return;
        }
      } catch {
        // Plugin unavailable or already asked — fall back to System Settings
      }
      await openSystemPreferences("notifications");
      startPolling();
    } else if (kind === "fullDiskAccess") {
      await openSystemPreferences("full_disk_access");
      startPolling();
    }
  };

  return (
    <div className="flex items-center justify-center gap-4 border-b border-warning/20 bg-warning/5 px-4 py-2">
      {items.map((item) => (
        <div key={item.kind} className="flex items-center gap-2">
          <span className={cn("h-2 w-2 shrink-0 rounded-full", dotColor(item.state))} />
          <span className="text-xs text-foreground/80">{item.label}</span>
          {item.state !== "granted" && (
            <button
              type="button"
              onClick={() => handleEnable(item.kind)}
              disabled={item.kind === "automation" && triggeringAutomation}
              className={cn(
                "rounded-md px-2 py-0.5",
                "text-xs font-medium",
                "bg-primary/10 text-primary",
                "transition-colors hover:bg-primary/20",
                "disabled:opacity-50 disabled:cursor-not-allowed",
              )}
            >
              {item.kind === "automation" && triggeringAutomation ? "Requesting..." : "Enable"}
            </button>
          )}
        </div>
      ))}
    </div>
  );
}
