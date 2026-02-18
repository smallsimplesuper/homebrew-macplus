export interface AppSummary {
  id: number;
  bundleId: string;
  displayName: string;
  appPath: string;
  installedVersion: string | null;
  installSource: string;
  isIgnored: boolean;
  iconCachePath: string | null;
  hasUpdate: boolean;
  availableVersion: string | null;
  updateSource: string | null;
  homebrewCaskToken: string | null;
  homebrewFormulaName: string | null;
  releaseNotes: string | null;
  releaseNotesUrl: string | null;
  updateNotes: string | null;
  description: string | null;
}

export interface AppDetail {
  id: number;
  bundleId: string;
  displayName: string;
  appPath: string;
  installedVersion: string | null;
  bundleVersion: string | null;
  iconCachePath: string | null;
  architectures: string[] | null;
  installSource: string;
  obtainedFrom: string | null;
  homebrewCaskToken: string | null;
  isIgnored: boolean;
  firstSeenAt: string | null;
  lastSeenAt: string | null;
  description: string | null;
  updateSources: UpdateSourceInfo[];
  availableUpdate: AvailableUpdateInfo | null;
}

export interface UpdateSourceInfo {
  sourceType: string;
  sourceUrl: string | null;
  isPrimary: boolean;
  lastCheckedAt: string | null;
}

export interface AvailableUpdateInfo {
  availableVersion: string;
  sourceType: string;
  releaseNotesUrl: string | null;
  downloadUrl: string | null;
  releaseNotes: string | null;
  isPaidUpgrade: boolean;
  detectedAt: string | null;
  notes: string | null;
}
