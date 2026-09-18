import { Fragment, useState } from "react";
import type { InstalledPackage } from "../../shared/types/package";
import { PackageRow } from "./PackageRow";
import { PackageDetail } from "./PackageDetail";
import { UninstallDialog } from "../uninstall/UninstallDialog";
import { UpdateDialog } from "../update/UpdateDialog";
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
  const [uninstallTarget, setUninstallTarget] = useState<InstalledPackage | null>(null);
  const [updateTarget, setUpdateTarget] = useState<InstalledPackage | null>(null);
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
            viewMode={viewMode}
            onInfo={onSelect}
            onUninstall={setUninstallTarget}
            onUpdate={setUpdateTarget}
          />
          {p.key === selectedKey && selectedPkg && (
            <PackageDetail pkg={selectedPkg} viewMode={viewMode} onUninstalled={onUninstalled} />
          )}
        </Fragment>
      ))}
      {updateTarget && (
        <UpdateDialog
          pkg={updateTarget}
          onClose={() => setUpdateTarget(null)}
          onUpdated={(p) => {
            setUpdateTarget(null);
            onUninstalled(p);
          }}
        />
      )}
      {uninstallTarget && (
        <UninstallDialog
          pkg={uninstallTarget}
          onClose={() => setUninstallTarget(null)}
          onUninstalled={(p) => {
            setUninstallTarget(null);
            onUninstalled(p);
          }}
        />
      )}
    </div>
  );
}