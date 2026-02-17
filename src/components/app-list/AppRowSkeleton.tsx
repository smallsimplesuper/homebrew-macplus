import { Skeleton } from "@/components/shared/Skeleton";

export function AppRowSkeleton() {
  return (
    <div className="grid min-h-[44px] items-center rounded-lg border border-border bg-card px-3 grid-cols-[28px_1fr_auto_90px] gap-2.5">
      <Skeleton className="h-7 w-7 rounded-[10px]" />
      <Skeleton className="h-3.5 w-40" />
      <Skeleton className="h-5 w-16 rounded-full" />
      <Skeleton className="ml-auto h-7 w-20 rounded-md" />
    </div>
  );
}

export function AppListSkeleton({ count = 12 }: { count?: number }) {
  return (
    <div className="flex flex-col gap-2">
      {Array.from({ length: count }, (_, i) => (
        <AppRowSkeleton key={i} />
      ))}
    </div>
  );
}
