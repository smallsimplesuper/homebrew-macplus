import { toast } from "sonner";
import type { ScanComplete, UpdateCheckComplete, UpdateExecuteComplete } from "@/types/update";
import { useTauriEvent } from "./useTauriEvent";

export function useToastNotifications() {
  useTauriEvent<ScanComplete>("scan-complete", (payload) => {
    toast.success("Scan complete", {
      id: "scan-complete",
      description: `Found ${payload.appCount} apps in ${(payload.durationMs / 1000).toFixed(1)}s`,
    });
  });

  useTauriEvent<UpdateCheckComplete>("update-check-complete", (payload) => {
    const { updatesFound } = payload;
    if (updatesFound > 0) {
      toast.info(`${updatesFound} update${updatesFound === 1 ? "" : "s"} available`, {
        id: "update-check",
        description: "Click Updates in the sidebar to view them.",
      });
    } else {
      toast.success("All apps are up to date", {
        id: "update-check",
      });
    }
  });

  useTauriEvent<UpdateExecuteComplete>("update-execute-complete", (payload) => {
    if (payload.success && payload.delegated) {
      toast.success(`Opened ${payload.displayName} — update within the app`, {
        id: `update-${payload.displayName}`,
      });
    } else if (payload.success) {
      toast.success(`Updated ${payload.displayName}`, {
        id: `update-${payload.displayName}`,
      });
    } else {
      toast.error(`Failed to update ${payload.displayName}`, {
        id: `update-error-${payload.displayName}`,
        description: payload.message,
      });
    }
  });
}
