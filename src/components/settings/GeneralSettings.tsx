import { disable, enable } from "@tauri-apps/plugin-autostart";
import { ChevronRight } from "lucide-react";
import { CustomSelect } from "@/components/shared/CustomSelect";
import { ToggleSwitch } from "@/components/shared/ToggleSwitch";
import { useSettings, useUpdateSettings } from "@/hooks/useSettings";
import { cn } from "@/lib/utils";
import type { AppSettings } from "@/types/settings";

const CHECK_INTERVALS = [
  { label: "Every 5 minutes", value: 5 },
  { label: "Every 10 minutes", value: 10 },
  { label: "Every 15 minutes", value: 15 },
  { label: "Every 30 minutes", value: 30 },
  { label: "Every hour", value: 60 },
  { label: "Every 4 hours", value: 240 },
  { label: "Daily", value: 1440 },
] as const;

export function GeneralSettings() {
  const { data: settings, isLoading } = useSettings();
  const updateSettings = useUpdateSettings();

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

  return (
    <div className="space-y-1">
      {/* Launch at login */}
      <div className="flex items-center justify-between rounded-lg border border-border bg-background px-4 py-3">
        <div>
          <p className="text-sm font-medium text-foreground">Launch at login</p>
          <p className="text-xs text-muted-foreground">
            Start macPlus automatically when you log in
          </p>
        </div>
        <ToggleSwitch
          checked={settings.launchAtLogin}
          onChange={(checked) => {
            handleUpdate({ launchAtLogin: checked });
            (checked ? enable() : disable()).catch(console.error);
          }}
        />
      </div>

      {/* Auto-check on launch */}
      <div className="flex items-center justify-between rounded-lg border border-border bg-background px-4 py-3">
        <div>
          <p className="text-sm font-medium text-foreground">Check on launch</p>
          <p className="text-xs text-muted-foreground">
            Automatically check for updates when macPlus starts
          </p>
        </div>
        <ToggleSwitch
          checked={settings.autoCheckOnLaunch}
          onChange={(checked) => handleUpdate({ autoCheckOnLaunch: checked })}
        />
      </div>

      {/* Check interval */}
      <div className="rounded-lg border border-border bg-background px-4 py-3">
        <div className="mb-2">
          <p className="text-sm font-medium text-foreground">Check interval</p>
          <p className="text-xs text-muted-foreground">
            How often to automatically check for updates
          </p>
        </div>
        <CustomSelect
          value={settings.checkIntervalMinutes}
          onChange={(value) => handleUpdate({ checkIntervalMinutes: value })}
          options={CHECK_INTERVALS}
        />
      </div>
      {/* Permissions & Setup link */}
      <button
        type="button"
        onClick={() => {
          window.dispatchEvent(new CustomEvent("navigate-settings-tab", { detail: "setup" }));
        }}
        className={cn(
          "mt-4 flex w-full items-center justify-between rounded-lg border border-border",
          "bg-background px-4 py-3 text-left",
          "transition-colors hover:bg-muted/50",
        )}
      >
        <div>
          <p className="text-sm font-medium text-foreground">Permissions & Setup</p>
          <p className="text-xs text-muted-foreground">
            Homebrew, permissions, and admin helper configuration
          </p>
        </div>
        <ChevronRight className="h-4 w-4 text-muted-foreground" />
      </button>
    </div>
  );
}
