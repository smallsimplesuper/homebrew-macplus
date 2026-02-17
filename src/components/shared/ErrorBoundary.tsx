import { AlertTriangle, RotateCcw } from "lucide-react";
import { Component, type ErrorInfo, type ReactNode } from "react";
import { cn } from "@/lib/utils";

interface ErrorBoundaryProps {
  children: ReactNode;
  fallback?: ReactNode;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo): void {
    console.error("[ErrorBoundary]", error, errorInfo);
  }

  handleReload = () => {
    this.setState({ hasError: false, error: null });
    window.location.reload();
  };

  render() {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }

      return (
        <div className="flex flex-col items-center justify-center gap-4 p-8">
          <div
            className={cn(
              "flex h-14 w-14 items-center justify-center",
              "rounded-full bg-red-500/10",
            )}
          >
            <AlertTriangle className="h-7 w-7 text-red-500" />
          </div>
          <div className="text-center">
            <h3 className="text-sm font-semibold text-red-600">Something went wrong</h3>
            <p className="mt-1 max-w-sm text-xs text-muted-foreground">
              {this.state.error?.message || "An unexpected error occurred."}
            </p>
          </div>
          <button
            type="button"
            onClick={this.handleReload}
            className={cn(
              "flex items-center gap-2 rounded-lg",
              "border border-red-500/20 bg-red-500/5 px-4 py-2",
              "text-xs font-medium text-red-600",
              "transition-colors hover:bg-red-500/10",
            )}
          >
            <RotateCcw className="h-3.5 w-3.5" />
            Reload
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}
