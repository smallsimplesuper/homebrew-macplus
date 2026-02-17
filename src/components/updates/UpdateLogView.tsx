import { useEffect, useRef } from "react";
import { cn } from "@/lib/utils";

interface UpdateLogViewProps {
  logs: string[];
}

export function UpdateLogView({ logs }: UpdateLogViewProps) {
  const scrollRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, []);

  return (
    <div
      ref={scrollRef}
      className={cn("h-48 overflow-y-auto rounded-lg", "border border-border bg-black/80 p-3")}
    >
      {logs.length === 0 ? (
        <p className="font-mono text-xs text-muted-foreground/50">No log output yet...</p>
      ) : (
        <pre className="whitespace-pre-wrap break-all font-mono text-xs leading-relaxed text-green-400">
          {logs.map((line, i) => (
            <div key={`${i}-${line.slice(0, 20)}`}>{line}</div>
          ))}
        </pre>
      )}
    </div>
  );
}
