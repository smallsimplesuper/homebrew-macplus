import { useCallback } from "react";
import { useUpdateProgressStore } from "@/stores/updateProgressStore";
import type { UpdateExecuteComplete, UpdateExecuteProgress } from "@/types/update";
import { useTauriEvent } from "./useTauriEvent";

export function useUpdateProgressListener() {
  const setProgress = useUpdateProgressStore((s) => s.setProgress);
  const clearProgress = useUpdateProgressStore((s) => s.clearProgress);
  const setRelaunchNeeded = useUpdateProgressStore((s) => s.setRelaunchNeeded);

  const handleProgress = useCallback(
    (payload: UpdateExecuteProgress) => {
      setProgress(
        payload.bundleId,
        payload.phase,
        payload.percent,
        payload.downloadedBytes ?? undefined,
        payload.totalBytes ?? undefined,
      );
    },
    [setProgress],
  );

  const handleComplete = useCallback(
    (payload: UpdateExecuteComplete) => {
      if (payload.delegated) {
        // Delegated updates: clear progress after brief delay, no relaunch needed.
        // The update stays in the app list until the next check verifies the version changed.
        setTimeout(() => clearProgress(payload.bundleId), 1500);
      } else if (payload.needsRelaunch && payload.appPath) {
        // Move from progress to relaunchNeeded instead of clearing
        clearProgress(payload.bundleId);
        setRelaunchNeeded(payload.bundleId, payload.appPath);
      } else {
        // Brief delay so the user sees 100% before it disappears
        setTimeout(() => clearProgress(payload.bundleId), 1500);
      }
    },
    [clearProgress, setRelaunchNeeded],
  );

  useTauriEvent<UpdateExecuteProgress>("update-execute-progress", handleProgress);
  useTauriEvent<UpdateExecuteComplete>("update-execute-complete", handleComplete);
}
