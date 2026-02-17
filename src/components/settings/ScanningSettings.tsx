import { open } from "@tauri-apps/plugin-dialog";
import { ChevronRight, FolderOpen, RefreshCw, X } from "lucide-react";
import { useState } from "react";
import { CustomSelect } from "@/components/shared/CustomSelect";
import { useSettings, useUpdateSettings } from "@/hooks/useSettings";
import { checkAllUpdates, triggerFullScan } from "@/lib/tauri-commands";
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
  const [isScanning, setIsScanning] = useState(false);
  const [scanResult, setScanResult] = useState<string | null>(null);

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

  const [scanPhase, setScanPhase] = useState<"scanning" | "checking" | null>(null);

  const handleRescan = async () => {
    setIsScanning(true);
    setScanResult(null);
    setScanPhase("scanning");
    try {
      const count = await triggerFullScan();
      setScanPhase("checking");
      const updates = await checkAllUpdates();
      setScanResult(`Found ${count} apps · ${updates} update${updates === 1 ? "" : "s"} available`);
    } catch {
      setScanResult("Scan failed");
    } finally {
      setIsScanning(false);
      setScanPhase(null);
    }
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
            return (
              <div
                key={location}
                className="flex items-center justify-between rounded-md bg-muted/50 px-3 py-2"
              >
                <div className="flex items-center gap-2 min-w-0">
                  <FolderOpen className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                  <span className="truncate text-xs text-foreground">{location}</span>
                  {isDefault && (
                    <span className="shrink-0 rounded bg-muted px-1.5 py-0.5 text-caption font-medium text-muted-foreground">
                      default
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

      {/* Rescan */}
      <div className="rounded-lg border border-border bg-background px-4 py-3">
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm font-medium text-foreground">Rescan</p>
            <p className="text-xs text-muted-foreground">
              Scan all locations for installed apps now
            </p>
          </div>
          <button
            type="button"
            onClick={handleRescan}
            disabled={isScanning}
            className={cn(
              "flex items-center gap-1.5 rounded-md px-3 py-1.5",
              "bg-primary text-xs font-medium text-primary-foreground",
              "transition-colors hover:bg-primary/90",
              "disabled:cursor-not-allowed disabled:opacity-50",
            )}
          >
            <RefreshCw className={cn("h-3.5 w-3.5", isScanning && "animate-spin")} />
            {scanPhase === "checking"
              ? "Checking updates..."
              : isScanning
                ? "Scanning..."
                : "Scan Now"}
          </button>
        </div>
        {scanResult && <p className="mt-2 text-xs text-muted-foreground">{scanResult}</p>}
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
