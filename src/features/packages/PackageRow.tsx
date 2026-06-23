import type { InstalledPackage } from "../../shared/types/package";
import { SOURCE_COLORS, SOURCE_LABELS } from "../../shared/types/package";
import { formatSize, kindIcon } from "./format";
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
  const color = SOURCE_COLORS[pkg.source];
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
        <span className="pkg-row__sub">
          <span className="pkg-row__kind" title={`Kind: ${pkg.app_kind}`}>
            {kindIcon(pkg.app_kind)}
          </span>
          <span className="pkg-row__version">{pkg.version || "—"}</span>
          {pkg.description && <span className="pkg-row__desc">{truncate(pkg.description, 90)}</span>}
        </span>
      </span>
      <span className="pkg-row__meta">
        <span
          className="pkg-row__source"
          style={{ color, borderColor: color }}
        >
          {SOURCE_LABELS[pkg.source]}
        </span>
        <span className="pkg-row__size">{formatSize(pkg.size_bytes)}</span>
      </span>
    </button>
  );
}

function truncate(s: string, n: number): string {
  return s.length > n ? `${s.slice(0, n - 1)}…` : s;
}