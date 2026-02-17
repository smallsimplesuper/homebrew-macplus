import { UpdateSourceIcon } from "@/components/app-list/UpdateSourceIcon";
import { cn } from "@/lib/utils";
import type { AppDetail } from "@/types/app";

interface AppInfoSectionProps {
  detail: AppDetail;
}

function InfoRow({ label, value }: { label: string; value: string | null | undefined }) {
  return (
    <div className="flex flex-col gap-0.5">
      <span className="text-xs text-muted-foreground">{label}</span>
      <span className={cn("text-sm", value ? "text-foreground" : "text-muted-foreground/60")}>
        {value || "Unknown"}
      </span>
    </div>
  );
}

export function AppInfoSection({ detail }: AppInfoSectionProps) {
  return (
    <div className="space-y-1">
      <h4 className="text-caption-uppercase tracking-wider text-muted-foreground">Information</h4>
      <div className="grid grid-cols-2 gap-x-4 gap-y-3 rounded-lg border border-border bg-background p-3">
        <InfoRow label="Bundle ID" value={detail.bundleId} />
        <InfoRow label="Version" value={detail.installedVersion} />
        <InfoRow label="Build" value={detail.bundleVersion} />
        <InfoRow label="Source" value={detail.installSource} />
        <InfoRow label="Architecture" value={detail.architectures?.join(", ") ?? null} />
        <InfoRow label="Path" value={detail.appPath} />
        {detail.availableUpdate && (
          <div className="flex flex-col gap-0.5">
            <span className="text-xs text-muted-foreground">Update Source</span>
            <UpdateSourceIcon source={detail.availableUpdate.sourceType} />
          </div>
        )}
      </div>
    </div>
  );
}
