// Typed Tauri invoke wrapper for the install-kind command.

import { invoke } from "@tauri-apps/api/core";
import type { InstallKind } from "../types/install";

export function getInstallKind(): Promise<InstallKind> {
  return invoke<InstallKind>("install_kind");
}
