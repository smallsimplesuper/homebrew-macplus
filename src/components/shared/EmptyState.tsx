import type { LucideIcon } from "lucide-react";
import { cn } from "@/lib/utils";

interface EmptyStateProps {
  icon: LucideIcon;
  title: string;
  description: string;
}

export function EmptyState({ icon: Icon, title, description }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 px-4 py-16">
      <Icon className="h-12 w-12 text-muted-foreground/40" />
      <div className="text-center">
        <p className={cn("text-sm font-medium text-muted-foreground")}>{title}</p>
        <p className="mt-1 max-w-xs text-xs text-muted-foreground/60">{description}</p>
      </div>
    </div>
  );
}
