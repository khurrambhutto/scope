import type { InstalledPackage } from "../../shared/types/package";
import { PackageRow } from "./PackageRow";

export function PackageList({
  packages,
  selectedKey,
  onSelect,
}: {
  packages: InstalledPackage[];
  selectedKey: string | null;
  onSelect: (pkg: InstalledPackage) => void;
}) {
  if (packages.length === 0) {
    return <div className="pkg-list pkg-list--empty">No installed apps match your search.</div>;
  }
  return (
    <div className="pkg-list">
      {packages.map((p) => (
        <PackageRow
          key={p.key}
          pkg={p}
          selected={p.key === selectedKey}
          onClick={onSelect}
        />
      ))}
    </div>
  );
}