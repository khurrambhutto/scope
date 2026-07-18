import type { InstalledPackage } from "../../shared/types/package";
import type { Operation } from "../../shared/types/operations";
import { PackageRow } from "./PackageRow";
import type { ViewMode } from "./usePackages";

export function PackageList({
  packages,
  selectedKey,
  viewMode,
  busyByKey,
  onSelect,
  onAction,
}: {
  packages: InstalledPackage[];
  selectedKey: string | null;
  viewMode: ViewMode;
  busyByKey: Record<string, Operation>;
  onSelect: (pkg: InstalledPackage) => void;
  onAction: (pkg: InstalledPackage, kind: Operation) => void;
}) {
  if (packages.length === 0) {
    return (
      <div className="pkg-list pkg-list--empty">
        {viewMode === "updates"
          ? "Everything is up to date."
          : "No installed apps match your search."}
      </div>
    );
  }
  return (
    <div className="pkg-list">
      {packages.map((p) => (
        <PackageRow
          key={p.key}
          pkg={p}
          selected={p.key === selectedKey}
          viewMode={viewMode}
          busy={busyByKey[p.key] ?? null}
          onSelect={onSelect}
          onAction={onAction}
        />
      ))}
    </div>
  );
}
