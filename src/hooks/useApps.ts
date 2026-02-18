import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import {
  getAllApps,
  getAppDetail,
  setAppIgnored,
  triggerFullScan,
  uninstallApp,
} from "@/lib/tauri-commands";

export function useApps() {
  return useQuery({
    queryKey: ["apps"],
    queryFn: getAllApps,
    staleTime: 5 * 60 * 1000,
    refetchOnWindowFocus: false,
  });
}

export function useAppDetail(bundleId: string | null) {
  return useQuery({
    queryKey: ["app-detail", bundleId],
    queryFn: () => getAppDetail(bundleId!),
    enabled: !!bundleId,
    staleTime: 2 * 60 * 1000,
  });
}

export function useFullScan() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: triggerFullScan,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["apps"] });
    },
  });
}

export function useToggleIgnored() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ bundleId, ignored }: { bundleId: string; ignored: boolean }) =>
      setAppIgnored(bundleId, ignored),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["apps"] });
    },
  });
}

export function useUninstallApp() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      bundleId,
      cleanupAssociated,
    }: {
      bundleId: string;
      cleanupAssociated: boolean;
    }) => uninstallApp(bundleId, cleanupAssociated),
    onSuccess: (result) => {
      if (result.success) {
        toast.success(`Moved to Trash`, {
          description:
            result.cleanedPaths.length > 0
              ? `Also removed ${result.cleanedPaths.length} associated file${result.cleanedPaths.length === 1 ? "" : "s"}`
              : undefined,
        });
        queryClient.invalidateQueries({ queryKey: ["apps"] });
        queryClient.invalidateQueries({ queryKey: ["app-detail"] });
      } else {
        toast.error("Uninstall failed", { description: result.message ?? undefined });
      }
    },
    onError: (error) => {
      toast.error("Uninstall failed", { description: String(error) });
    },
  });
}
