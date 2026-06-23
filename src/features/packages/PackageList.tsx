import { Fragment } from "react";
import type { InstalledPackage } from "../../shared/types/package";
import { PackageRow } from "./PackageRow";
import { PackageDetail } from "./PackageDetail";

export function PackageList({
  packages,
  selectedKey,
  selectedPkg,
  onSelect,
  onUninstalled,
}: {
  packages: InstalledPackage[];
  selectedKey: string | null;
  selectedPkg: InstalledPackage | null;
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
            <PackageDetail pkg={selectedPkg} onUninstalled={onUninstalled} />
          )}
        </Fragment>
      ))}
    </div>
  );
}