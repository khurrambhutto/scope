// Typed Tauri invoke wrappers for the uninstall and update commands.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { OperationLog, OperationPlan, OperationResult, OperationStatus } from "../types/operations";

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

/// Subscribe to live command output, one line at a time, while an apply
/// command runs. Returns an unlisten function.
export function onOperationLog(
  handler: (log: OperationLog) => void,
): Promise<UnlistenFn> {
  return listen<OperationLog>("operation-log", (event) => {
    handler(event.payload);
  });
}

/// A dismissed Polkit password dialog comes back as a non-zero pkexec result
/// (exit 126, "Request dismissed") rather than a thrown error. Treat that as
/// the user declining the action so the UI can show a calm cancelled state
/// instead of an error.
export function isCancelledResult(result: OperationResult): boolean {
  if (result.success || result.exit_code !== 126) return false;
  return /dismiss|cancel/i.test(`${result.message}\n${result.logs}`);
}
