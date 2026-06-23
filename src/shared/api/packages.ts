// Typed Tauri invoke wrappers for the package commands.

import { invoke } from "@tauri-apps/api/core";
import type {
  CachedScan,
  InstalledPackage,
  PackageSource,
  AppKind,
  ScanStatus,
} from "../types/package";

export function scanPackages(): Promise<CachedScan> {
  return invoke<CachedScan>("scan_packages");
}

export function getCachedScan(): Promise<CachedScan | null> {
  return invoke<CachedScan | null>("get_cached_scan");
}

export function scanStatus(): Promise<ScanStatus> {
  return invoke<ScanStatus>("scan_status");
}

export function searchPackages(
  query?: string,
  source?: PackageSource,
  appKind?: AppKind
): Promise<InstalledPackage[]> {
  return invoke<InstalledPackage[]>("search_packages", {
    query: query ?? null,
    source: source ?? null,
    appKind: appKind ?? null,
  });
}