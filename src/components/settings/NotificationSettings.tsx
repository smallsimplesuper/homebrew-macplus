import { ToggleSwitch } from "@/components/shared/ToggleSwitch";
import { useSettings, useUpdateSettings } from "@/hooks/useSettings";
import type { AppSettings } from "@/types/settings";

export function NotificationSettings() {
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
      {/* Notify on updates */}
      <div className="flex items-center justify-between rounded-lg border border-border bg-background px-4 py-3">
        <div>
          <p className="text-sm font-medium text-foreground">Update notifications</p>
          <p className="text-xs text-muted-foreground">
            Show a notification when new updates are found
          </p>
        </div>
        <ToggleSwitch
          checked={settings.notificationOnUpdates}
          onChange={(checked) => handleUpdate({ notificationOnUpdates: checked })}
        />
      </div>

      {/* Notification sound */}
      <div className="flex items-center justify-between rounded-lg border border-border bg-background px-4 py-3">
        <div>
          <p className="text-sm font-medium text-foreground">Notification sound</p>
          <p className="text-xs text-muted-foreground">Play the default sound with notifications</p>
        </div>
        <ToggleSwitch
          checked={settings.notificationSound}
          onChange={(checked) => handleUpdate({ notificationSound: checked })}
          disabled={!settings.notificationOnUpdates}
        />
      </div>

      {/* Show menu bar icon */}
      <div className="flex items-center justify-between rounded-lg border border-border bg-background px-4 py-3">
        <div>
          <p className="text-sm font-medium text-foreground">Menu bar icon</p>
          <p className="text-xs text-muted-foreground">Show macPlus icon in the menu bar</p>
        </div>
        <ToggleSwitch
          checked={settings.showMenuBarIcon}
          onChange={(checked) => handleUpdate({ showMenuBarIcon: checked })}
        />
      </div>

      {/* Badge count */}
      <div className="flex items-center justify-between rounded-lg border border-border bg-background px-4 py-3">
        <div>
          <p className="text-sm font-medium text-foreground">Update count badge</p>
          <p className="text-xs text-muted-foreground">
            Show the number of available updates on the tray icon
          </p>
        </div>
        <ToggleSwitch
          checked={settings.showBadgeCount}
          onChange={(checked) => handleUpdate({ showBadgeCount: checked })}
          disabled={!settings.showMenuBarIcon}
        />
      </div>
    </div>
  );
}
