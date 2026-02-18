import { invoke } from "@tauri-apps/api/core";
import type { AppDetail, AppSummary } from "@/types/app";
import type { AppSettings } from "@/types/settings";
import type { UpdateHistoryEntry, UpdateInfo, UpdateResult } from "@/types/update";

export async function getAllApps(): Promise<AppSummary[]> {
  return invoke<AppSummary[]>("get_all_apps");
}

export async function getAppDetail(bundleId: string): Promise<AppDetail> {
  return invoke<AppDetail>("get_app_detail", { bundleId });
}

export async function triggerFullScan(): Promise<number> {
  return invoke<number>("trigger_full_scan");
}

export async function setAppIgnored(bundleId: string, ignored: boolean): Promise<void> {
  return invoke("set_app_ignored", { bundleId, ignored });
}

export async function checkAllUpdates(): Promise<number> {
  return invoke<number>("check_all_updates");
}

export async function checkSingleUpdate(bundleId: string): Promise<UpdateInfo | null> {
  return invoke<UpdateInfo | null>("check_single_update", { bundleId });
}

export async function getUpdateCount(): Promise<number> {
  return invoke<number>("get_update_count");
}

export async function executeUpdate(bundleId: string): Promise<UpdateResult> {
  return invoke<UpdateResult>("execute_update", { bundleId });
}

export async function executeBulkUpdate(bundleIds: string[]): Promise<UpdateResult[]> {
  return invoke<UpdateResult[]>("execute_bulk_update", { bundleIds });
}

export async function getSettings(): Promise<AppSettings> {
  return invoke<AppSettings>("get_settings");
}

export async function updateSettings(settings: AppSettings): Promise<void> {
  return invoke("update_settings", { settings });
}

export async function getUpdateHistory(limit?: number): Promise<UpdateHistoryEntry[]> {
  return invoke<UpdateHistoryEntry[]>("get_update_history", { limit: limit ?? 50 });
}

export async function relaunchApp(bundleId: string, appPath: string): Promise<void> {
  return invoke("relaunch_app", { bundleId, appPath });
}

export async function openApp(path: string): Promise<void> {
  return invoke("open_app", { path });
}

export async function revealInFinder(path: string): Promise<void> {
  return invoke("reveal_in_finder", { path });
}

export async function getAppIcon(appPath: string, bundleId: string): Promise<string | null> {
  return invoke<string | null>("get_app_icon", { appPath, bundleId });
}

export interface PermissionsStatus {
  automation: boolean;
  automationState: "granted" | "denied" | "unknown";
  fullDiskAccess: boolean;
  appManagement: boolean;
  notifications: boolean;
}

export async function getPermissionsStatus(): Promise<PermissionsStatus> {
  return invoke<PermissionsStatus>("get_permissions_status");
}

export async function getPermissionsPassive(): Promise<PermissionsStatus> {
  return invoke<PermissionsStatus>("get_permissions_passive");
}

export async function triggerAutomationPermission(): Promise<boolean> {
  return invoke<boolean>("trigger_automation_permission");
}

export async function openSystemPreferences(pane: string): Promise<void> {
  return invoke("open_system_preferences", { pane });
}

export interface SetupStatus {
  homebrewInstalled: boolean;
  homebrewVersion: string | null;
  homebrewPath: string | null;
  askpassInstalled: boolean;
  askpassPath: string | null;
  xcodeCltInstalled: boolean;
  permissions: PermissionsStatus;
  connectivity: ConnectivityStatus;
}

export async function checkSetupStatus(): Promise<SetupStatus> {
  return invoke<SetupStatus>("check_setup_status");
}

export async function ensureAskpassHelper(): Promise<string | null> {
  return invoke<string | null>("ensure_askpass_helper");
}

export async function openTerminalWithCommand(command: string): Promise<void> {
  return invoke("open_terminal_with_command", { command });
}

export interface SelfUpdateInfo {
  availableVersion: string;
  currentVersion: string;
  releaseNotesUrl: string | null;
  downloadUrl: string | null;
  canBrewUpgrade: boolean;
}

export async function checkSelfUpdate(): Promise<SelfUpdateInfo | null> {
  return invoke<SelfUpdateInfo | null>("check_self_update");
}

export interface SelfUpdateProgress {
  phase: string;
  percent: number;
  downloadedBytes: number | null;
  totalBytes: number | null;
}

export async function executeSelfUpdate(downloadUrl: string): Promise<void> {
  return invoke("execute_self_update", { downloadUrl });
}

export async function relaunchSelf(): Promise<void> {
  return invoke("relaunch_self");
}

export interface ConnectivityStatus {
  github: boolean;
  homebrew: boolean;
  itunes: boolean;
  overall: "connected" | "partial" | "disconnected";
}

export async function checkConnectivity(): Promise<ConnectivityStatus> {
  return invoke<ConnectivityStatus>("check_connectivity");
}

export interface CheckerDiagnostic {
  source: string;
  canCheck: boolean;
  result: string;
}

export interface UpdateCheckDiagnostic {
  bundleId: string;
  appPath: string;
  installedVersion: string | null;
  installSource: string;
  homebrewCaskToken: string | null;
  checkersTried: CheckerDiagnostic[];
}

export async function debugUpdateCheck(bundleId: string): Promise<UpdateCheckDiagnostic> {
  return invoke<UpdateCheckDiagnostic>("debug_update_check", { bundleId });
}

export async function checkPathsExist(paths: string[]): Promise<Record<string, boolean>> {
  return invoke<Record<string, boolean>>("check_paths_exist", { paths });
}
