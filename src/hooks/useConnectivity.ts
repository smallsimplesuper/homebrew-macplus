import { useQuery } from "@tanstack/react-query";
import { checkConnectivity } from "@/lib/tauri-commands";

export function useConnectivity() {
  const { data, isLoading } = useQuery({
    queryKey: ["connectivity"],
    queryFn: checkConnectivity,
    refetchInterval: 60_000,
    staleTime: 30_000,
    retry: 1,
  });

  return {
    status: data?.overall ?? "disconnected",
    details: data,
    isChecking: isLoading,
  };
}
