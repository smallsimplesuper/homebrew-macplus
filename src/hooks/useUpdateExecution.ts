import { useMutation, useQueryClient } from "@tanstack/react-query";
import { executeBulkUpdate, executeUpdate } from "@/lib/tauri-commands";

export function useExecuteUpdate() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (bundleId: string) => executeUpdate(bundleId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["apps"] });
    },
  });
}

export function useExecuteBulkUpdate() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (bundleIds: string[]) => executeBulkUpdate(bundleIds),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["apps"] });
    },
  });
}
