import { convertFileSrc } from "@tauri-apps/api/core";
import { Package, Terminal } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { getAppIcon } from "@/lib/tauri-commands";
import { cn } from "@/lib/utils";

interface AppIconProps {
  iconPath: string | null;
  appPath?: string;
  displayName: string;
  bundleId?: string;
  size?: number;
}

export function AppIcon({ iconPath, appPath, displayName, bundleId, size = 40 }: AppIconProps) {
  const letter = displayName.charAt(0).toUpperCase();
  const isFormula = bundleId?.startsWith("homebrew.formula.");
  const isCLICask = bundleId?.startsWith("homebrew.cask.") && !iconPath && !appPath;

  const [resolvedPath, setResolvedPath] = useState<string | null>(iconPath);
  const fetchFailed = useRef(false);

  // Sync when the iconPath prop changes
  useEffect(() => {
    setResolvedPath(iconPath);
    fetchFailed.current = false;
  }, [iconPath]);

  // Lazy fetch when resolvedPath is null and appPath is available
  useEffect(() => {
    if (resolvedPath || !appPath || !bundleId || isFormula || isCLICask || fetchFailed.current)
      return;

    let cancelled = false;
    getAppIcon(appPath, bundleId).then((path) => {
      if (!cancelled && path) {
        setResolvedPath(path);
      }
    });

    return () => {
      cancelled = true;
    };
  }, [resolvedPath, appPath, bundleId, isFormula, isCLICask]);

  if (resolvedPath) {
    return (
      <img
        src={convertFileSrc(resolvedPath)}
        alt={displayName}
        width={size}
        height={size}
        className="rounded-[10px] object-cover shadow-[0_1px_3px_rgba(0,0,0,0.12),0_0_0_0.5px_rgba(0,0,0,0.06)]"
        style={{ width: size, height: size }}
        draggable={false}
        onError={() => {
          fetchFailed.current = true;
          setResolvedPath(null);
        }}
      />
    );
  }

  if (isFormula) {
    return (
      <div
        className={cn(
          "flex items-center justify-center rounded-[10px] bg-amber-500/10 text-amber-600 dark:text-amber-400 select-none",
          "shadow-[0_1px_3px_rgba(0,0,0,0.12),0_0_0_0.5px_rgba(0,0,0,0.06)]",
        )}
        style={{ width: size, height: size }}
      >
        <Terminal style={{ width: size * 0.5, height: size * 0.5 }} />
      </div>
    );
  }

  if (isCLICask) {
    return (
      <div
        className={cn(
          "flex items-center justify-center rounded-[10px] bg-orange-500/10 text-orange-600 dark:text-orange-400 select-none",
          "shadow-[0_1px_3px_rgba(0,0,0,0.12),0_0_0_0.5px_rgba(0,0,0,0.06)]",
        )}
        style={{ width: size, height: size }}
      >
        <Package style={{ width: size * 0.5, height: size * 0.5 }} />
      </div>
    );
  }

  return (
    <div
      className={cn(
        "flex items-center justify-center rounded-[10px] bg-primary/10 text-primary font-semibold select-none",
        "shadow-[0_1px_3px_rgba(0,0,0,0.12),0_0_0_0.5px_rgba(0,0,0,0.06)]",
      )}
      style={{ width: size, height: size, fontSize: size * 0.45 }}
    >
      {letter}
    </div>
  );
}
