// TypeScript models matching the Rust DTOs in src-tauri/src/package.rs and
// src-tauri/src/scanner/mod.rs. Keep in sync with the backend.

export type PackageSource = "apt" | "snap" | "flatpak" | "appimage" | "manual";

export type AppKind = "gui" | "cli" | "unknown";

export type InstallScope = "user" | "system";

export interface InstalledPackage {
  key: string;
  source: PackageSource;
  package_id: string;
  install_scope?: InstallScope;
  name: string;
  display_name?: string;
  description?: string;
  version: string;
  size_bytes: number;
  app_kind: AppKind;
  icon?: string;
  categories?: string;
  terminal: boolean;
  has_update: boolean;
  update_version?: string;
}

export interface ScanAvailability {
  apt: boolean;
  snap: boolean;
  flatpak: boolean;
  appimage: boolean;
  manual: boolean;
  apt_error?: string;
  snap_error?: string;
  flatpak_error?: string;
  appimage_dirs: string[];
}

export interface CachedScan {
  packages: InstalledPackage[];
  availability: ScanAvailability;
  scanned_at_ms: number;
}

export interface ScanStatus {
  apt_available: boolean;
  snap_available: boolean;
  flatpak_available: boolean;
  appimage_available: boolean;
  appimage_dirs: string[];
}

export const SOURCE_LABELS: Record<PackageSource, string> = {
  apt: "APT",
  snap: "Snap",
  flatpak: "Flatpak",
  appimage: "AppImage",
  manual: "Manual",
};

export const SOURCE_COLORS: Record<PackageSource, string> = {
  apt: "#a1352c",
  snap: "#2196f3",
  flatpak: "#4a154b",
  appimage: "#0b8a4f",
  manual: "#7c3aed",
};

// Light variants for text/badges on dark backgrounds (SOURCE_COLORS are
// fallback-icon backgrounds and too dark to read as text).
export const SOURCE_BADGE_COLORS: Record<PackageSource, string> = {
  apt: "#ff8f80",
  snap: "#6ab7ff",
  flatpak: "#d29bd8",
  appimage: "#5fd6a2",
  manual: "#a78bfa",
};
