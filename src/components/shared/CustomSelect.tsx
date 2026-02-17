import { ChevronDown } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { cn } from "@/lib/utils";

interface CustomSelectOption<T extends string | number> {
  value: T;
  label: string;
  description?: string;
}

interface CustomSelectProps<T extends string | number> {
  value: T;
  onChange: (value: T) => void;
  options: readonly CustomSelectOption<T>[];
  disabled?: boolean;
}

export function CustomSelect<T extends string | number>({
  value,
  onChange,
  options,
  disabled,
}: CustomSelectProps<T>) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handleClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, [open]);

  const currentLabel = options.find((o) => o.value === value)?.label ?? "";

  return (
    <div ref={ref} className="relative">
      <button
        type="button"
        onClick={() => !disabled && setOpen(!open)}
        disabled={disabled}
        className={cn(
          "flex w-full items-center justify-between rounded-md border border-border bg-background",
          "px-3 py-1.5 text-sm text-foreground",
          "transition-colors hover:bg-muted/50",
          "focus:outline-none focus:ring-2 focus:ring-primary/40",
          disabled && "cursor-not-allowed opacity-50",
        )}
      >
        <span>{currentLabel}</span>
        <ChevronDown
          className={cn(
            "h-3.5 w-3.5 text-muted-foreground transition-transform",
            open && "rotate-180",
          )}
        />
      </button>
      {open && (
        <div className="absolute left-0 right-0 z-50 mt-1 overflow-hidden rounded-md border border-border bg-popover shadow-md">
          {options.map((opt) => (
            <button
              key={String(opt.value)}
              type="button"
              onClick={() => {
                onChange(opt.value);
                setOpen(false);
              }}
              className={cn(
                "flex w-full flex-col px-3 py-2 text-left text-sm transition-colors",
                opt.value === value
                  ? "bg-accent text-accent-foreground"
                  : "text-popover-foreground hover:bg-muted/50",
              )}
            >
              <span className="font-medium">{opt.label}</span>
              {opt.description && (
                <span className="text-xs text-muted-foreground">{opt.description}</span>
              )}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
