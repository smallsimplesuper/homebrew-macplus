import { useMutation, useQueryClient } from "@tanstack/react-query";
import { checkAllUpdates, checkSingleUpdate } from "@/lib/tauri-commands";

export function useCheckAllUpdates() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: checkAllUpdates,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["apps"] });
    },
  });
}

export function useCheckSingleUpdate() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (bundleId: string) => checkSingleUpdate(bundleId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["apps"] });
    },
  });
}
