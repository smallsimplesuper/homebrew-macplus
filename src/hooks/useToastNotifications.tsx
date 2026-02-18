import { AnimatePresence, motion } from "framer-motion";
import { useState } from "react";
import { toast } from "sonner";
import type { ScanComplete, UpdateCheckComplete, UpdateExecuteComplete } from "@/types/update";
import { useTauriEvent } from "./useTauriEvent";

function ExpandableErrorDescription({ message }: { message: string }) {
  const [expanded, setExpanded] = useState(false);
  const isLong = message.length > 80 || message.includes("\n");

  if (!isLong) {
    return <span>{message}</span>;
  }

  const firstLine = message.split("\n")[0];
  const summary = firstLine.length > 60 ? `${firstLine.slice(0, 60)}...` : firstLine;

  return (
    <div>
      <span>{summary}</span>
      <AnimatePresence>
        {expanded && (
          <motion.div
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.2, ease: "easeInOut" }}
            className="overflow-hidden"
          >
            <pre className="mt-2 whitespace-pre-wrap font-mono text-[10px] max-h-[200px] overflow-y-auto text-muted-foreground">
              {message}
            </pre>
          </motion.div>
        )}
      </AnimatePresence>
      <button
        type="button"
        className="mt-1 text-[11px] text-muted-foreground underline hover:text-foreground"
        onClick={(e) => {
          e.stopPropagation();
          setExpanded((prev) => !prev);
        }}
      >
        {expanded ? "Less info" : "More info"}
      </button>
    </div>
  );
}

export function useToastNotifications() {
  useTauriEvent<ScanComplete>("scan-complete", (payload) => {
    toast.success("Scan complete", {
      id: "scan-complete",
      description: `Found ${payload.appCount} apps in ${(payload.durationMs / 1000).toFixed(1)}s`,
    });
  });

  useTauriEvent<UpdateCheckComplete>("update-check-complete", (payload) => {
    const { updatesFound } = payload;
    if (updatesFound > 0) {
      toast.info(`${updatesFound} update${updatesFound === 1 ? "" : "s"} available`, {
        id: "update-check",
        description: "Click Updates in the sidebar to view them.",
      });
    } else {
      toast.success("All apps are up to date", {
        id: "update-check",
      });
    }
  });

  useTauriEvent<UpdateExecuteComplete>("update-execute-complete", (payload) => {
    if (payload.success && payload.delegated) {
      toast.success(`Opened ${payload.displayName} — update within the app`, {
        id: `update-${payload.displayName}`,
      });
    } else if (payload.success) {
      toast.success(`Updated ${payload.displayName}`, {
        id: `update-${payload.displayName}`,
      });
    } else {
      toast.error(`Failed to update ${payload.displayName}`, {
        id: `update-error-${payload.displayName}`,
        description: payload.message ? (
          <ExpandableErrorDescription message={payload.message} />
        ) : undefined,
        duration: 10000,
      });
    }
  });
}
