import { Info } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { cn } from "@/lib/utils";
import type { AppSummary } from "@/types/app";

function formatSource(source: string): string {
  switch (source) {
    case "homebrew":
      return "Homebrew Cask";
    case "homebrew_formula":
      return "Homebrew Formula";
    case "mas":
      return "Mac App Store";
    case "direct":
      return "Direct Install";
    default:
      return source;
  }
}

export function InfoPopover({ app }: { app: AppSummary }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  return (
    <div ref={ref} className="relative">
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          setOpen((v) => !v);
        }}
        className={cn(
          "flex shrink-0 items-center justify-center rounded-md",
          "h-7 w-7 text-muted-foreground",
          "transition-colors hover:bg-muted hover:text-foreground",
          open && "bg-muted text-foreground",
        )}
        title="App info"
      >
        <Info className="h-3.5 w-3.5" />
      </button>
      {open && (
        <div
          className={cn(
            "absolute right-0 top-full z-50 mt-1",
            "w-64 rounded-lg border border-border bg-card p-3 shadow-lg",
          )}
          onClick={(e) => e.stopPropagation()}
        >
          <div className="space-y-1.5 text-xs">
            {app.description && (
              <p className="text-foreground leading-relaxed">{app.description}</p>
            )}
            {app.appPath && (
              <div>
                <span className="font-medium text-muted-foreground">Location: </span>
                <span className="text-foreground break-all">{app.appPath}</span>
              </div>
            )}
            <div>
              <span className="font-medium text-muted-foreground">Source: </span>
              <span className="text-foreground">{formatSource(app.installSource)}</span>
            </div>
            <div>
              <span className="font-medium text-muted-foreground">Bundle ID: </span>
              <span className="text-foreground break-all">{app.bundleId}</span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
