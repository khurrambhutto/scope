import type { InstalledPackage } from "../../shared/types/package";
import { SOURCE_LABELS } from "../../shared/types/package";
import { formatSize } from "./format";
import { AppIcon } from "../../shared/components/AppIcon";

export function PackageRow({
  pkg,
  selected,
  onClick,
}: {
  pkg: InstalledPackage;
  selected: boolean;
  onClick: (pkg: InstalledPackage) => void;
}) {
  const title = pkg.display_name ?? pkg.name;
  return (
    <button
      type="button"
      className={`pkg-row${selected ? " pkg-row--selected" : ""}`}
      onClick={() => onClick(pkg)}
    >
      <AppIcon pkg={pkg} title={title} size="row" />
      <span className="pkg-row__main">
        <span className="pkg-row__title">
          {title}
          {pkg.terminal && <span className="pkg-row__term" title="Runs in terminal">⌘</span>}
          {pkg.has_update && <span className="pkg-row__update" title="Update available">↑</span>}
        </span>
        <span className="pkg-row__meta-line">
          <span>{pkg.version || "—"}</span>
          <span>·</span>
          <span>{formatSize(pkg.size_bytes)}</span>
          <span>·</span>
          <span>{SOURCE_LABELS[pkg.source]}</span>
        </span>
      </span>
    </button>
  );
}