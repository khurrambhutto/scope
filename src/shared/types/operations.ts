// TypeScript models matching the Rust DTOs in src-tauri/src/operations/mod.rs.
// Keep in sync with the backend.

import type { InstallScope, PackageSource } from "./package";

export type Operation = "uninstall";
export type AuthMethod = "none" | "pkexec";

export interface PlanStep {
  description: string;
  command_summary: string;
}

export interface OperationPlan {
  plan_id: string;
  operation: Operation;
  source: PackageSource;
  package_id: string;
  install_scope?: InstallScope;
  display_name: string;
  current_version: string;
  requires_auth: boolean;
  auth_method: AuthMethod;
  protected: boolean;
  protection_reason?: string;
  steps: PlanStep[];
  created_at_ms: number;
}

export interface OperationResult {
  success: boolean;
  message: string;
  logs: string;
  exit_code: number | null;
}
