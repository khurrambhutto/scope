// Typed Tauri invoke wrappers for the package commands.

import { invoke } from "@tauri-apps/api/core";
import type { CachedScan } from "../types/package";

export function scanPackages(): Promise<CachedScan> {
  return invoke<CachedScan>("scan_packages");
}

export function getCachedScan(): Promise<CachedScan | null> {
  return invoke<CachedScan | null>("get_cached_scan");
}
