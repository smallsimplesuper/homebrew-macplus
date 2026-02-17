import { Beer, Github, HelpCircle, Sparkles, Store } from "lucide-react";
import type { ComponentType } from "react";
import { cn } from "@/lib/utils";

interface UpdateSourceIconProps {
  source: string | null;
}

const sourceMap: Record<
  string,
  { icon: ComponentType<{ className?: string; size?: number }>; label: string }
> = {
  sparkle: { icon: Sparkles, label: "Sparkle" },
  homebrew_cask: { icon: Beer, label: "Homebrew" },
  homebrew_api: { icon: Beer, label: "Homebrew API" },
  mas: { icon: Store, label: "MAS" },
  github: { icon: Github, label: "GitHub" },
};

export function UpdateSourceIcon({ source }: UpdateSourceIconProps) {
  if (!source) return null;

  const entry = sourceMap[source];
  const Icon = entry?.icon ?? HelpCircle;
  const label = entry?.label ?? source;

  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-full px-2 py-0.5",
        "bg-muted text-caption font-medium text-muted-foreground",
      )}
    >
      <Icon size={10} className="shrink-0" />
      {label}
    </span>
  );
}
