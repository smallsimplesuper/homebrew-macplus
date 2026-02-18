import type { ReactNode } from "react";
import { useWindowFocus } from "@/hooks/useWindowMode";
import { cn } from "@/lib/utils";
import AppToolbar from "./AppToolbar";
import StatusBar from "./StatusBar";

interface DesktopShellProps {
  children: ReactNode;
  appCount?: number;
  updateCount?: number;
  ignoredCount?: number;
}

export default function DesktopShell({
  children,
  appCount = 0,
  updateCount = 0,
  ignoredCount = 0,
}: DesktopShellProps) {
  const focused = useWindowFocus();

  return (
    <div
      className={cn(
        "flex h-screen w-screen flex-col overflow-hidden rounded-[10px] bg-background transition-opacity duration-200",
        !focused && "opacity-[0.92]",
      )}
    >
      <AppToolbar updateCount={updateCount} />

      {/* Glass frame content area */}
      <div className="mx-2 mb-2 flex flex-1 flex-col overflow-hidden glass-frame">
        <main className="flex-1 overflow-auto">{children}</main>
      </div>

      <StatusBar appCount={appCount} updateCount={updateCount} ignoredCount={ignoredCount} />
    </div>
  );
}
