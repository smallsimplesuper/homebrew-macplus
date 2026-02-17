import { Download } from "lucide-react";
import { AppIcon } from "@/components/app-list/AppIcon";
import { cn } from "@/lib/utils";
import type { AppSummary } from "@/types/app";

interface MenuBarAppListProps {
  apps: AppSummary[];
  onUpdate: (bundleId: string) => void;
}

export function MenuBarAppList({ apps, onUpdate }: MenuBarAppListProps) {
  if (apps.length === 0) {
    return null;
  }

  return (
    <div className="flex flex-col py-1">
      {apps.map((app) => (
        <div
          key={app.bundleId}
          className={cn(
            "flex items-center gap-2.5 px-3 py-1.5",
            "rounded-md transition-colors hover:bg-muted/40",
          )}
        >
          {/* Icon */}
          <AppIcon
            iconPath={app.iconCachePath}
            appPath={app.appPath}
            displayName={app.displayName}
            bundleId={app.bundleId}
            size={24}
          />

          {/* Name */}
          <div className="min-w-0 flex-1">
            <p className="truncate text-xs font-medium text-foreground">{app.displayName}</p>
          </div>

          {/* Version badge */}
          {app.availableVersion && (
            <span className="shrink-0 rounded-full bg-primary/10 px-1.5 py-0.5 text-caption font-medium text-primary">
              {app.availableVersion}
            </span>
          )}

          {/* Update button */}
          <button
            type="button"
            onClick={() => onUpdate(app.bundleId)}
            className={cn(
              "shrink-0 rounded-md p-1",
              "text-primary transition-colors hover:bg-primary/10",
            )}
          >
            <Download className="h-3.5 w-3.5" />
          </button>
        </div>
      ))}
    </div>
  );
}
