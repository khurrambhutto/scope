import { Fragment } from "react";
import type { InstalledPackage } from "../../shared/types/package";
import { PackageRow } from "./PackageRow";
import { PackageDetail } from "./PackageDetail";
import type { ViewMode } from "./usePackages";

export function PackageList({
  packages,
  selectedKey,
  selectedPkg,
  viewMode,
  onSelect,
  onUninstalled,
}: {
  packages: InstalledPackage[];
  selectedKey: string | null;
  selectedPkg: InstalledPackage | null;
  viewMode: ViewMode;
  onSelect: (pkg: InstalledPackage) => void;
  onUninstalled: (pkg: InstalledPackage) => void;
}) {
  if (packages.length === 0) {
    return <div className="pkg-list pkg-list--empty">No installed apps match your search.</div>;
  }
  return (
    <div className="pkg-list">
      {packages.map((p) => (
        <Fragment key={p.key}>
          <PackageRow
            pkg={p}
            selected={p.key === selectedKey}
            onClick={onSelect}
          />
          {p.key === selectedKey && selectedPkg && (
            <PackageDetail pkg={selectedPkg} viewMode={viewMode} onUninstalled={onUninstalled} />
          )}
        </Fragment>
      ))}
    </div>
  );
}