import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { getAllApps, getAppDetail, setAppIgnored, triggerFullScan } from "@/lib/tauri-commands";

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
