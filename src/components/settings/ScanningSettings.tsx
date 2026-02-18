import { useQuery } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import { AlertCircle, ChevronRight, FolderOpen, X } from "lucide-react";
import { CustomSelect } from "@/components/shared/CustomSelect";
import { useSettings, useUpdateSettings } from "@/hooks/useSettings";
import { checkPathsExist } from "@/lib/tauri-commands";
import { cn } from "@/lib/utils";
import { useAppFilterStore } from "@/stores/appFilterStore";
import type { AppSettings } from "@/types/settings";

const DEFAULT_LOCATIONS = ["/Applications", "~/Applications"];

const SCAN_DEPTH_OPTIONS = [
  { value: 1, label: "Top level only", description: "Only finds apps directly in scan folders" },
  {
    value: 2,
    label: "Include subfolders",
    description: "Finds apps in one subfolder deep (e.g. Adobe apps)",
  },
  { value: 3, label: "Deep scan", description: "Scans two subfolders deep — slower but thorough" },
] as const;

export function ScanningSettings() {
  const { data: settings, isLoading } = useSettings();
  const updateSettings = useUpdateSettings();

  const { data: pathStatus } = useQuery({
    queryKey: ["path-status", settings?.scanLocations],
    queryFn: () => checkPathsExist(settings?.scanLocations ?? []),
    enabled: !!settings?.scanLocations?.length,
    staleTime: 30 * 1000,
  });

  if (isLoading || !settings) {
    return (
      <div className="rounded-lg border border-border bg-background p-6">
        <p className="text-sm text-muted-foreground">Loading settings...</p>
      </div>
    );
  }

  const handleUpdate = (partial: Partial<AppSettings>) => {
    updateSettings.mutate({ ...settings, ...partial });
  };

  const handleAddFolder = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      title: "Select scan location",
    });

    if (selected && typeof selected === "string") {
      if (!settings.scanLocations.includes(selected)) {
        handleUpdate({
          scanLocations: [...settings.scanLocations, selected],
        });
      }
    }
  };

  const handleRemoveLocation = (location: string) => {
    handleUpdate({
      scanLocations: settings.scanLocations.filter((l) => l !== location),
    });
  };

  return (
    <div className="space-y-1">
      {/* Scan Locations */}
      <div className="rounded-lg border border-border bg-background px-4 py-3">
        <div className="mb-3">
          <p className="text-sm font-medium text-foreground">Scan Locations</p>
          <p className="text-xs text-muted-foreground">
            Directories to search for installed applications
          </p>
        </div>
        <div className="space-y-1.5">
          {settings.scanLocations.map((location) => {
            const isDefault = DEFAULT_LOCATIONS.includes(location);
            const exists = pathStatus?.[location] ?? true;
            const isVolume = location.startsWith("/Volumes/");
            return (
              <div
                key={location}
                className={cn(
                  "flex items-center justify-between rounded-md px-3 py-2",
                  exists ? "bg-muted/50" : "bg-destructive/5",
                )}
              >
                <div className="flex items-center gap-2 min-w-0">
                  {exists ? (
                    <FolderOpen className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                  ) : (
                    <AlertCircle className="h-3.5 w-3.5 shrink-0 text-destructive" />
                  )}
                  <span
                    className={cn(
                      "truncate text-xs",
                      exists ? "text-foreground" : "text-destructive",
                    )}
                  >
                    {location}
                  </span>
                  {isDefault && (
                    <span className="shrink-0 rounded bg-muted px-1.5 py-0.5 text-caption font-medium text-muted-foreground">
                      default
                    </span>
                  )}
                  {!exists && (
                    <span className="shrink-0 rounded bg-destructive/10 px-1.5 py-0.5 text-caption font-medium text-destructive">
                      {isVolume ? "Not mounted" : "Not found"}
                    </span>
                  )}
                </div>
                {!isDefault && (
                  <button
                    type="button"
                    onClick={() => handleRemoveLocation(location)}
                    className="ml-2 shrink-0 rounded p-0.5 text-muted-foreground hover:bg-muted hover:text-foreground"
                  >
                    <X className="h-3.5 w-3.5" />
                  </button>
                )}
              </div>
            );
          })}
        </div>
        <button
          type="button"
          onClick={handleAddFolder}
          className={cn(
            "mt-2 flex w-full items-center justify-center gap-1.5",
            "rounded-md border border-dashed border-border px-3 py-2",
            "text-xs font-medium text-muted-foreground",
            "transition-colors hover:border-foreground/30 hover:text-foreground",
            "disabled:cursor-not-allowed disabled:opacity-50",
          )}
        >
          <FolderOpen className="h-3.5 w-3.5" />
          Add Folder
        </button>
      </div>

      {/* Scan Depth */}
      <div className="rounded-lg border border-border bg-background px-4 py-3">
        <div className="mb-2">
          <p className="text-sm font-medium text-foreground">Scan Depth</p>
          <p className="text-xs text-muted-foreground">
            How deep to look for apps inside scan locations
          </p>
        </div>
        <CustomSelect
          value={settings.scanDepth}
          onChange={(value) => handleUpdate({ scanDepth: value })}
          options={SCAN_DEPTH_OPTIONS}
        />
      </div>

      {/* Ignored apps link */}
      <button
        type="button"
        onClick={() => {
          useAppFilterStore.getState().setFilterView("ignored");
        }}
        className={cn(
          "flex w-full items-center justify-between rounded-lg border border-border",
          "bg-background px-4 py-3 text-left",
          "transition-colors hover:bg-muted/50",
        )}
      >
        <div>
          <p className="text-sm font-medium text-foreground">Ignored Apps</p>
          <p className="text-xs text-muted-foreground">Manage apps excluded from update checks</p>
        </div>
        <ChevronRight className="h-4 w-4 text-muted-foreground" />
      </button>
    </div>
  );
}
