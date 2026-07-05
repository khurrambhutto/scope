import { useEffect, useState, useCallback } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

type Status =
  | "idle"
  | "checking"
  | "available"
  | "downloading"
  | "installing"
  | "ready"
  | "error"
  | "no-update";

interface UpdateState {
  status: Status;
  update: Update | null;
  error: string | null;
  downloaded: number;
  total: number | null;
}

export function useAppUpdater() {
  const [state, setState] = useState<UpdateState>({
    status: "idle",
    update: null,
    error: null,
    downloaded: 0,
    total: null,
  });

  useEffect(() => {
    let cancelled = false;
    const timer = setTimeout(async () => {
      if (cancelled) return;
      setState((s) => ({ ...s, status: "checking" }));
      try {
        const update = await check();
        if (cancelled) return;
        if (update) {
          setState((s) => ({
            ...s,
            status: "available",
            update,
          }));
        } else {
          setState((s) => ({ ...s, status: "no-update" }));
        }
      } catch (e) {
        if (cancelled) return;
        setState((s) => ({ ...s, status: "error", error: String(e) }));
      }
    }, 1000);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, []);

  const download = useCallback(async () => {
    const update = state.update;
    if (!update) return;

    setState((s) => ({ ...s, status: "downloading", downloaded: 0, total: null, error: null }));
    try {
      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            setState((s) => ({
              ...s,
              total: event.data.contentLength ?? null,
            }));
            break;
          case "Progress":
            setState((s) => ({
              ...s,
              downloaded: s.downloaded + event.data.chunkLength,
            }));
            break;
          case "Finished":
            break;
        }
      });
      setState((s) => ({ ...s, status: "ready" }));
    } catch (e) {
      setState((s) => ({ ...s, status: "error", error: String(e) }));
    }
  }, [state.update]);

  const restart = useCallback(async () => {
    await relaunch();
  }, []);

  const dismiss = useCallback(() => {
    setState((s) => ({ ...s, status: "no-update" }));
  }, []);

  return { ...state, download, restart, dismiss };
}
