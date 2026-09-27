import type { AppKind } from "../../shared/types/package";

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

export function kindIcon(kind: AppKind): string {
  return { gui: "🖼", cli: "⌨", unknown: "❔" }[kind];
}

/**
 * Footer status line. When nothing is filtered out it reads "Showing 130 apps";
 * once a filter or search narrows the list it reads "Showing 50 of 130 apps".
 */
export function formatAppCount(shown: number, total: number): string {
  if (shown === total) {
    return `Showing ${total} ${total === 1 ? "app" : "apps"}`;
  }
  return `Showing ${shown} of ${total} apps`;
}
