export interface AppSettings {
  checkIntervalMinutes: number;
  launchAtLogin: boolean;
  showMenuBarIcon: boolean;
  notificationOnUpdates: boolean;
  autoCheckOnLaunch: boolean;
  theme: "system" | "light" | "dark";
  ignoredBundleIds: string[];
  scanLocations: string[];
  scanDepth: number;
  showBadgeCount: boolean;
  notificationSound: boolean;
}
