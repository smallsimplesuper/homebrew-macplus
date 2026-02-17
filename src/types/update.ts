export interface UpdateInfo {
  bundleId: string;
  currentVersion: string | null;
  availableVersion: string;
  sourceType: string;
  downloadUrl: string | null;
  releaseNotesUrl: string | null;
  isPaidUpgrade: boolean;
}

export interface UpdateResult {
  bundleId: string;
  success: boolean;
  message: string | null;
  sourceType: string;
  fromVersion: string | null;
  toVersion: string | null;
}

export interface ScanProgress {
  phase: string;
  current: number;
  total: number;
  appName: string | null;
}

export interface ScanComplete {
  appCount: number;
  durationMs: number;
}

export interface UpdateCheckProgress {
  checked: number;
  total: number;
  currentApp: string | null;
}

export interface UpdateFound {
  bundleId: string;
  currentVersion: string | null;
  availableVersion: string;
  source: string;
}

export interface UpdateCheckComplete {
  updatesFound: number;
  durationMs: number;
}

export interface UpdateExecuteProgress {
  bundleId: string;
  phase: string;
  percent: number;
  downloadedBytes: number | null;
  totalBytes: number | null;
}

export interface UpdateExecuteComplete {
  bundleId: string;
  displayName: string;
  success: boolean;
  message?: string;
  needsRelaunch: boolean;
  appPath?: string;
  delegated?: boolean;
}

export interface UpdateHistoryEntry {
  id: number;
  bundleId: string;
  displayName: string;
  iconCachePath: string | null;
  fromVersion: string;
  toVersion: string;
  sourceType: string;
  status: string;
  errorMessage: string | null;
  startedAt: string | null;
  completedAt: string | null;
}
