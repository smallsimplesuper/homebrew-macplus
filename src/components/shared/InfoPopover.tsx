import { Info } from "lucide-react";
import { cn } from "@/lib/utils";
import { useUIStore } from "@/stores/uiStore";

export function InfoPopover({ app }: { app: { bundleId: string } }) {
  const selectApp = useUIStore((s) => s.selectApp);

  return (
    <button
      type="button"
      onClick={(e) => {
        e.stopPropagation();
        selectApp(app.bundleId);
      }}
      className={cn(
        "flex shrink-0 items-center justify-center rounded-md",
        "h-7 w-7 text-muted-foreground",
        "transition-colors hover:bg-muted hover:text-foreground",
      )}
      title="App info"
    >
      <Info className="h-3.5 w-3.5" />
    </button>
  );
}
