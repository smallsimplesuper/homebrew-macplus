import { useEffect, useState } from "react";
import { cn } from "@/lib/utils";
import { AppearanceSettings } from "./AppearanceSettings";
import { GeneralSettings } from "./GeneralSettings";
import { NotificationSettings } from "./NotificationSettings";
import { ScanningSettings } from "./ScanningSettings";
import { SetupView } from "./SetupView";

type SettingsTab = "general" | "appearance" | "scanning" | "notifications" | "setup";

const tabs: { id: SettingsTab; label: string }[] = [
  { id: "general", label: "General" },
  { id: "appearance", label: "Appearance" },
  { id: "scanning", label: "Scanning" },
  { id: "notifications", label: "Notifications" },
  { id: "setup", label: "Setup" },
];

export function SettingsView() {
  const [activeTab, setActiveTab] = useState<SettingsTab>("general");

  // Listen for cross-component navigation (e.g. GeneralSettings "Setup" link)
  useEffect(() => {
    const handler = (e: Event) => {
      const tab = (e as CustomEvent).detail as SettingsTab;
      if (tabs.some((t) => t.id === tab)) {
        setActiveTab(tab);
      }
    };
    window.addEventListener("navigate-settings-tab", handler);
    return () => window.removeEventListener("navigate-settings-tab", handler);
  }, []);

  return (
    <div className="flex flex-col gap-4 p-4">
      <h1 className="text-title text-foreground">Settings</h1>

      {/* Tab bar */}
      <div className="flex gap-1 rounded-lg border border-border bg-muted/50 p-1">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            type="button"
            onClick={() => setActiveTab(tab.id)}
            className={cn(
              "flex-1 rounded-md px-3 py-1.5 text-xs font-medium",
              "transition-colors",
              activeTab === tab.id
                ? "bg-background text-foreground shadow-sm"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Tab content */}
      <div className="mt-2">
        {activeTab === "general" && <GeneralSettings />}
        {activeTab === "appearance" && <AppearanceSettings />}
        {activeTab === "scanning" && <ScanningSettings />}
        {activeTab === "notifications" && <NotificationSettings />}
        {activeTab === "setup" && <SetupView />}
      </div>
    </div>
  );
}
