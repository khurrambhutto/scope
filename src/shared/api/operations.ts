// Typed Tauri invoke wrappers for the uninstall commands.

import { invoke } from "@tauri-apps/api/core";
import type { OperationPlan, OperationResult } from "../types/operations";

/// Ask the backend to build (and store) an uninstall preview plan for the
/// package with the given backend key (`<source>:<package_id>`).
export function previewUninstall(packageKey: string): Promise<OperationPlan> {
  return invoke<OperationPlan>("preview_uninstall", { packageKey });
}

/// Apply a previously-issued plan by id. The backend revalidates against a
/// fresh scan before executing, so stale plans are rejected.
export function applyUninstall(planId: string): Promise<OperationResult> {
  return invoke<OperationResult>("apply_uninstall", { planId });
}
