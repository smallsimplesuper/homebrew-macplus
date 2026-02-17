import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-shell";
import { ArrowUpCircle, ExternalLink, Terminal, X } from "lucide-react";
import { useEffect, useState } from "react";
import {
  checkSelfUpdate,
  openTerminalWithCommand,
  type SelfUpdateInfo,
} from "@/lib/tauri-commands";
import { cn } from "@/lib/utils";

export function SelfUpdateBanner() {
  const [info, setInfo] = useState<SelfUpdateInfo | null>(null);
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    checkSelfUpdate()
      .then((result) => {
        if (result) setInfo(result);
      })
      .catch(() => {});

    const unlisten = listen<SelfUpdateInfo>("self-update-available", (event) => {
      setInfo(event.payload);
      setDismissed(false);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  if (dismissed || !info) return null;

  const handleUpdate = () => {
    if (info.canBrewUpgrade) {
      openTerminalWithCommand("brew upgrade --cask macplus").catch(console.error);
    } else {
      const url = info.releaseNotesUrl ?? `https://github.com/smallsimplesuper/homebrew-macplus/releases/latest`;
      open(url).catch(console.error);
    }
  };

  return (
    <div
      className={cn("flex items-center gap-3 border-b border-primary/20 bg-primary/5 px-4 py-2.5")}
    >
      <ArrowUpCircle className="h-4 w-4 shrink-0 text-primary" />
      <p className="flex-1 text-xs text-primary">
        macPlus {info.availableVersion} is available{" "}
        <span className="text-primary/60">(current: {info.currentVersion})</span>
      </p>
      <button
        type="button"
        onClick={handleUpdate}
        className={cn(
          "flex items-center gap-1 rounded-md px-2.5 py-1",
          "bg-primary/10 text-xs font-medium text-primary",
          "transition-colors hover:bg-primary/20",
        )}
      >
        {info.canBrewUpgrade ? (
          <Terminal className="h-3 w-3" />
        ) : (
          <ExternalLink className="h-3 w-3" />
        )}
        {info.canBrewUpgrade ? "Brew Upgrade" : "Download"}
      </button>
      <button
        type="button"
        onClick={() => setDismissed(true)}
        className="rounded-md p-1 text-primary/60 transition-colors hover:text-primary"
      >
        <X className="h-3.5 w-3.5" />
      </button>
    </div>
  );
}
