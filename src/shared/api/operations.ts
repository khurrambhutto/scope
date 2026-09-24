// Typed Tauri invoke wrappers for the uninstall and update commands.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { OperationPlan, OperationResult, OperationStatus } from "../types/operations";

/// Ask the backend to build (and store) an uninstall preview plan for the
/// package with the given backend key (`<source>:<package_id>`).
export function previewUninstall(packageKey: string): Promise<OperationPlan> {
  return invoke<OperationPlan>("preview_uninstall", { packageKey });
}

/// Apply a previously-issued plan by id. The backend probes the live state of
/// this one package before executing, so stale plans are rejected.
export function applyUninstall(planId: string): Promise<OperationResult> {
  return invoke<OperationResult>("apply_uninstall", { planId });
}

/// Ask the backend to build (and store) an update preview plan for the
/// package with the given backend key.
export function previewUpdate(packageKey: string): Promise<OperationPlan> {
  return invoke<OperationPlan>("preview_update", { packageKey });
}

/// Apply a previously-issued update plan by id.
export function applyUpdate(planId: string): Promise<OperationResult> {
  return invoke<OperationResult>("apply_update", { planId });
}

/// Subscribe to backend progress events emitted while an apply command runs
/// ("verifying" before revalidation, "executing" once the command starts).
/// Returns an unlisten function.
export function onOperationStatus(
  handler: (status: OperationStatus) => void,
): Promise<UnlistenFn> {
  return listen<OperationStatus>("operation-status", (event) => {
    handler(event.payload);
  });
}
