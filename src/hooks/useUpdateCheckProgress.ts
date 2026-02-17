import { useCallback, useState } from "react";
import type { UpdateCheckComplete, UpdateCheckProgress } from "@/types/update";
import { useTauriEvent } from "./useTauriEvent";

export function useUpdateCheckProgress() {
  const [progress, setProgress] = useState<UpdateCheckProgress | null>(null);
  const [isChecking, setIsChecking] = useState(false);
  const [lastChecked, setLastChecked] = useState<Date | null>(null);

  useTauriEvent<UpdateCheckProgress>(
    "update-check-progress",
    useCallback((payload: UpdateCheckProgress) => {
      setIsChecking(true);
      setProgress(payload);
    }, []),
  );

  useTauriEvent<UpdateCheckComplete>(
    "update-check-complete",
    useCallback((_payload: UpdateCheckComplete) => {
      setIsChecking(false);
      setProgress(null);
      setLastChecked(new Date());
    }, []),
  );

  return { progress, isChecking, lastChecked };
}
