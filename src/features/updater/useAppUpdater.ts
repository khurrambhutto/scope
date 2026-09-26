import { useEffect, useState, useCallback } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { getInstallKind } from "../../shared/api/install";
import type { InstallKind } from "../../shared/types/install";

export const RELEASES_URL = "https://github.com/khurrambhutto/scope/releases";

// The updater serves an AppImage, which cannot install over a system-package
// (.deb) install — that path always fails with "invalid updater binary
// format". System installs get directions instead of a doomed Update button.
export const MANUAL_UPDATE_MESSAGE =
  "This install can't update itself. Download the new package from the Releases page and install it yourself.";

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
  installKind: InstallKind | null;
  error: string | null;
  manualUpdate: boolean;
  downloaded: number;
  total: number | null;
}

export function useAppUpdater() {
  const [state, setState] = useState<UpdateState>({
    status: "idle",
    update: null,
    installKind: null,
    error: null,
    manualUpdate: false,
    downloaded: 0,
    total: null,
  });

  useEffect(() => {
    let cancelled = false;
    const timer = setTimeout(async () => {
      if (cancelled) return;
      setState((s) => ({ ...s, status: "checking" }));
      try {
        const [update, installKind] = await Promise.all([
          check(),
          // Fail closed: if detection breaks, assume a system install so we
          // never offer a one-click update that is guaranteed to fail.
          getInstallKind().catch<InstallKind>(() => "system"),
        ]);
        if (cancelled) return;
        if (update) {
          setState((s) => ({
            ...s,
            status: "available",
            update,
            installKind,
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
      const message = String(e);
      const manualUpdate = message.includes("invalid updater binary format");
      setState((s) => ({
        ...s,
        status: "error",
        error: manualUpdate ? MANUAL_UPDATE_MESSAGE : message,
        manualUpdate,
      }));
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
