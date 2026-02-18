import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  disable as disableAutostart,
  enable as enableAutostart,
} from "@tauri-apps/plugin-autostart";
import { AnimatePresence, motion } from "framer-motion";
import { useEffect, useMemo, useRef } from "react";
import { Toaster } from "sonner";
import { AppDetailSheet } from "@/components/app-detail/AppDetailSheet";
import { AppListView } from "@/components/app-list/AppListView";
import { default as DesktopShell } from "@/components/layout/DesktopShell";
import { default as MenuBarPanel } from "@/components/layout/MenuBarPanel";
import { MenuBarAppList } from "@/components/menubar/MenuBarAppList";
import { MenuBarFooter } from "@/components/menubar/MenuBarFooter";
import { MenuBarSummary } from "@/components/menubar/MenuBarSummary";
import { SettingsView } from "@/components/settings/SettingsView";
import { CommandPalette } from "@/components/shared/CommandPalette";
import { ErrorBoundary } from "@/components/shared/ErrorBoundary";
import { PermissionBanner } from "@/components/shared/PermissionBanner";
import { SelfUpdateBanner } from "@/components/shared/SelfUpdateBanner";
import { UpdateHistoryView } from "@/components/updates/UpdateHistoryView";
import { UpdatesOverview } from "@/components/updates/UpdatesOverview";
import { useApps, useFullScan } from "@/hooks/useApps";
import { useCheckAllUpdates } from "@/hooks/useAppUpdates";
import { useSettings } from "@/hooks/useSettings";
import { useToastNotifications } from "@/hooks/useToastNotifications";
import { useUpdateProgressListener } from "@/hooks/useUpdateProgress";
import { useWindowFocus, useWindowMode } from "@/hooks/useWindowMode";
import { springs } from "@/lib/animations";
import { executeUpdate } from "@/lib/tauri-commands";
import { useAppFilterStore } from "@/stores/appFilterStore";
import { useUIStore } from "@/stores/uiStore";

function DesktopApp() {
  const { data: apps } = useApps();
  const { data: settings } = useSettings();
  const fullScan = useFullScan();
  const checkUpdates = useCheckAllUpdates();
  const filterView = useAppFilterStore((s) => s.filterView);
  const detailOpen = useUIStore((s) => s.detailOpen);
  useToastNotifications();
  useUpdateProgressListener();

  const updateCount = useMemo(
    () => apps?.filter((a) => a.hasUpdate && !a.isIgnored).length ?? 0,
    [apps],
  );
  const ignoredCount = useMemo(() => apps?.filter((a) => a.isIgnored).length ?? 0, [apps]);

  // Startup: scan + check updates, gated on autoCheckOnLaunch setting
  const hasRunStartup = useRef(false);
  // biome-ignore lint/correctness/useExhaustiveDependencies: intentionally omitting mutate refs; hasRunStartup guard prevents re-execution
  useEffect(() => {
    if (!settings || hasRunStartup.current) return;

    if (!apps || apps.length === 0) {
      // First launch (empty DB): always scan then check updates
      hasRunStartup.current = true;
      fullScan.mutate(undefined, {
        onSuccess: () => checkUpdates.mutate(),
      });
    } else if (settings.autoCheckOnLaunch) {
      // Apps already in DB — only check if setting is on
      hasRunStartup.current = true;
      checkUpdates.mutate();
    } else {
      hasRunStartup.current = true;
    }
  }, [settings, apps]); // eslint-disable-line react-hooks/exhaustive-deps

  // Sync autostart system state with setting on startup
  useEffect(() => {
    if (!settings) return;
    (settings.launchAtLogin ? enableAutostart() : disableAutostart()).catch(console.error);
  }, [settings?.launchAtLogin, settings]);

  const renderContent = () => {
    switch (filterView) {
      case "updates":
        return <UpdatesOverview />;
      case "history":
        return <UpdateHistoryView />;
      case "settings":
        return <SettingsView />;
      case "ignored":
        return <AppListView />;
      default:
        return <AppListView />;
    }
  };

  return (
    <DesktopShell
      appCount={apps?.length ?? 0}
      updateCount={updateCount}
      ignoredCount={ignoredCount}
    >
      <PermissionBanner />
      <SelfUpdateBanner />
      <div className="flex flex-1 overflow-hidden">
        <AnimatePresence mode="wait">
          <motion.div
            key={filterView}
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -4 }}
            transition={springs.viewTransition}
            className="flex-1 overflow-auto"
          >
            {renderContent()}
          </motion.div>
        </AnimatePresence>
        {detailOpen && <AppDetailSheet />}
      </div>
      <CommandPalette />
    </DesktopShell>
  );
}

function MenuBarApp() {
  const { data: apps } = useApps();
  const checkUpdates = useCheckAllUpdates();
  const mode = useWindowMode();
  const focused = useWindowFocus();

  // Auto-hide panel when it loses focus
  useEffect(() => {
    if (mode === "panel" && !focused) {
      getCurrentWindow().hide();
    }
  }, [focused, mode]);

  const updatableApps = apps?.filter((a) => a.hasUpdate && !a.isIgnored) ?? [];

  return (
    <MenuBarPanel
      updateCount={updatableApps.length}
      onCheckNow={() => checkUpdates.mutate()}
      isChecking={checkUpdates.isPending}
    >
      <MenuBarSummary updateCount={updatableApps.length} />
      <MenuBarAppList
        apps={updatableApps}
        onUpdate={(bundleId) => {
          executeUpdate(bundleId).catch(console.error);
        }}
      />
      <MenuBarFooter
        onOpenMain={() => {
          import("@tauri-apps/api/webviewWindow").then(async ({ WebviewWindow }) => {
            const main = await WebviewWindow.getByLabel("main");
            if (main) {
              main.show();
              main.setFocus();
            }
          });
        }}
        onCheckNow={() => checkUpdates.mutate()}
        isChecking={checkUpdates.isPending}
      />
    </MenuBarPanel>
  );
}

export default function App() {
  const mode = useWindowMode();

  // Register keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.metaKey || e.ctrlKey) {
        switch (e.key) {
          case ",":
            e.preventDefault();
            useAppFilterStore.getState().setFilterView("settings");
            break;
        }
      }
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, []);

  // Apply theme from settings (system / light / dark)
  const { data: themeSettings } = useSettings();
  const theme = themeSettings?.theme ?? "system";

  useEffect(() => {
    const apply = (dark: boolean) => {
      document.documentElement.classList.toggle("dark", dark);
    };

    if (theme === "light") {
      apply(false);
      return;
    }
    if (theme === "dark") {
      apply(true);
      return;
    }

    // "system" — follow OS preference
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    apply(mq.matches);
    const handler = (e: MediaQueryListEvent) => apply(e.matches);
    mq.addEventListener("change", handler);
    return () => mq.removeEventListener("change", handler);
  }, [theme]);

  return (
    <ErrorBoundary>
      <Toaster
        position="bottom-right"
        toastOptions={{
          className: "!bg-card !text-card-foreground !border-border !shadow-lg",
        }}
      />
      {mode === "panel" ? <MenuBarApp /> : <DesktopApp />}
    </ErrorBoundary>
  );
}
