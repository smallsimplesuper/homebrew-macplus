import { create } from "zustand";

interface ProgressEntry {
  bundleId: string;
  phase: string;
  percent: number;
  downloadedBytes?: number;
  totalBytes?: number | null;
}

interface RelaunchEntry {
  bundleId: string;
  appPath: string;
}

interface UpdateProgressState {
  progress: Record<string, ProgressEntry>;
  relaunchNeeded: Record<string, RelaunchEntry>;
  setProgress: (
    bundleId: string,
    phase: string,
    percent: number,
    downloadedBytes?: number,
    totalBytes?: number | null,
  ) => void;
  clearProgress: (bundleId: string) => void;
  setRelaunchNeeded: (bundleId: string, appPath: string) => void;
  clearRelaunch: (bundleId: string) => void;
}

export const useUpdateProgressStore = create<UpdateProgressState>((set) => ({
  progress: {},
  relaunchNeeded: {},
  setProgress: (bundleId, phase, percent, downloadedBytes, totalBytes) =>
    set((s) => ({
      progress: {
        ...s.progress,
        [bundleId]: { bundleId, phase, percent, downloadedBytes, totalBytes },
      },
    })),
  clearProgress: (bundleId) =>
    set((s) => {
      const { [bundleId]: _, ...rest } = s.progress;
      return { progress: rest };
    }),
  setRelaunchNeeded: (bundleId, appPath) =>
    set((s) => ({
      relaunchNeeded: {
        ...s.relaunchNeeded,
        [bundleId]: { bundleId, appPath },
      },
    })),
  clearRelaunch: (bundleId) =>
    set((s) => {
      const { [bundleId]: _, ...rest } = s.relaunchNeeded;
      return { relaunchNeeded: rest };
    }),
}));
