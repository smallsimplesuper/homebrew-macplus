import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";

export type WindowMode = "desktop" | "panel";

export function useWindowMode(): WindowMode {
  const [mode, setMode] = useState<WindowMode>("desktop");

  useEffect(() => {
    const label = getCurrentWindow().label;
    setMode(label === "panel" ? "panel" : "desktop");
  }, []);

  return mode;
}

export function useWindowFocus(): boolean {
  const [focused, setFocused] = useState(true);

  useEffect(() => {
    const window = getCurrentWindow();
    const unlisten = window.onFocusChanged(({ payload }) => {
      setFocused(payload);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return focused;
}
