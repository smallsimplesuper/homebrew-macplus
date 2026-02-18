import { useCallback, useEffect, useRef, useState } from "react";
import {
  getPermissionsPassive,
  openSystemPreferences,
  type PermissionsStatus,
  triggerAutomationPermission,
} from "@/lib/tauri-commands";
import { cn } from "@/lib/utils";

type PermKind = "appManagement" | "automation" | "notifications";

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

let _allGranted = false;

/** Returns whether all 3 required permissions are granted. Subscribable from other components. */
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
  const debounceRef = useRef<ReturnType<typeof setTimeout>>(null);

  const refresh = useCallback(() => {
    getPermissionsPassive()
      .then((status) => {
        setPerms(status);
        const allOk = status.appManagement && status.automation && status.notifications;
        if (_allGranted !== allOk) {
          _allGranted = allOk;
          window.dispatchEvent(new Event("permissions-changed"));
        }
      })
      .catch(() => {});
  }, []);

  // Initial check on mount
  useEffect(() => {
    refresh();
  }, [refresh]);

  // Re-check on visibilitychange with 1s debounce (user returns from System Settings)
  useEffect(() => {
    const handler = () => {
      if (document.visibilityState === "visible") {
        if (debounceRef.current) clearTimeout(debounceRef.current);
        debounceRef.current = setTimeout(refresh, 1000);
      }
    };
    document.addEventListener("visibilitychange", handler);
    return () => {
      document.removeEventListener("visibilitychange", handler);
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [refresh]);

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
        if (granted) {
          const allOk = perms.appManagement && perms.notifications;
          if (!_allGranted && allOk) {
            _allGranted = true;
            window.dispatchEvent(new Event("permissions-changed"));
          }
        }
      } finally {
        setTriggeringAutomation(false);
      }
    } else if (kind === "appManagement") {
      await openSystemPreferences("app_management");
    } else if (kind === "notifications") {
      await openSystemPreferences("notifications");
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
