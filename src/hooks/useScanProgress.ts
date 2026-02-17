import { useCallback, useState } from "react";
import type { ScanComplete, ScanProgress } from "@/types/update";
import { useTauriEvent } from "./useTauriEvent";

export function useScanProgress() {
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [isScanning, setIsScanning] = useState(false);

  useTauriEvent<ScanProgress>(
    "scan-progress",
    useCallback((payload: ScanProgress) => {
      setIsScanning(true);
      setProgress(payload);
    }, []),
  );

  useTauriEvent<ScanComplete>(
    "scan-complete",
    useCallback((_payload: ScanComplete) => {
      setIsScanning(false);
      setProgress(null);
    }, []),
  );

  return { progress, isScanning };
}
