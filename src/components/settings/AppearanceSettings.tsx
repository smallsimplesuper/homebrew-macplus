import { Monitor, Moon, Sun } from "lucide-react";
import { useSettings, useUpdateSettings } from "@/hooks/useSettings";
import { cn } from "@/lib/utils";
import type { AppSettings } from "@/types/settings";

const THEME_OPTIONS = [
  { value: "system" as const, label: "System", icon: Monitor },
  { value: "light" as const, label: "Light", icon: Sun },
  { value: "dark" as const, label: "Dark", icon: Moon },
];

export function AppearanceSettings() {
  const { data: settings, isLoading } = useSettings();
  const updateSettings = useUpdateSettings();

  if (isLoading || !settings) {
    return (
      <div className="rounded-lg border border-border bg-background p-6">
        <p className="text-sm text-muted-foreground">Loading settings...</p>
      </div>
    );
  }

  const handleThemeChange = (theme: AppSettings["theme"]) => {
    updateSettings.mutate({ ...settings, theme });
  };

  return (
    <div className="space-y-3">
      <div className="rounded-lg border border-border bg-background px-4 py-3">
        <div className="mb-3">
          <p className="text-sm font-medium text-foreground">Theme</p>
          <p className="text-xs text-muted-foreground">Choose how macPlus looks on your system</p>
        </div>
        <div className="grid grid-cols-3 gap-2">
          {THEME_OPTIONS.map((option) => {
            const Icon = option.icon;
            const isActive = settings.theme === option.value;

            return (
              <button
                key={option.value}
                type="button"
                onClick={() => handleThemeChange(option.value)}
                className={cn(
                  "flex flex-col items-center gap-2 rounded-lg border-2 p-3",
                  "transition-colors",
                  isActive
                    ? "border-primary bg-primary/5"
                    : "border-border hover:border-muted-foreground/30 hover:bg-muted/50",
                )}
              >
                <Icon
                  className={cn("h-5 w-5", isActive ? "text-primary" : "text-muted-foreground")}
                />
                <span
                  className={cn(
                    "text-xs font-medium",
                    isActive ? "text-primary" : "text-muted-foreground",
                  )}
                >
                  {option.label}
                </span>
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
