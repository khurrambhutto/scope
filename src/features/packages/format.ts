import type { PackageSource, AppKind } from "../../shared/types/package";

export function formatSize(bytes: number): string {
  if (!bytes) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  const digits = unit === 0 ? 0 : value < 10 ? 1 : 0;
  return `${value.toFixed(digits)} ${units[unit]}`;
}

export function sourceBadgeColor(source: PackageSource): string {
  const map: Record<PackageSource, string> = {
    apt: "#a1352c",
    snap: "#2196f3",
    flatpak: "#4a154b",
    appimage: "#0b8a4f",
  };
  return map[source];
}

export function sourceLabel(source: PackageSource): string {
  return { apt: "APT", snap: "Snap", flatpak: "Flatpak", appimage: "AppImage" }[source];
}

export function kindIcon(kind: AppKind): string {
  return { gui: "🖼", cli: "⌨", unknown: "❔" }[kind];
}