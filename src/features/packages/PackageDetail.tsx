import type { InstalledPackage } from "../../shared/types/package";
import {
  KIND_COLORS,
  KIND_LABELS,
  SOURCE_COLORS,
  SOURCE_LABELS,
} from "../../shared/types/package";
import { formatSize } from "./format";
import { AppIcon } from "../../shared/components/AppIcon";

export function PackageDetail({ pkg }: { pkg: InstalledPackage | null }) {
  if (!pkg) return null;

  const title = pkg.display_name ?? pkg.name;
  const rows: { label: string; value: string }[] = [
    { label: "Package id", value: pkg.package_id },
    { label: "Version", value: pkg.version || "—" },
    { label: "Installed size", value: formatSize(pkg.size_bytes) },
    { label: "Categories", value: pkg.categories ?? "—" },
    { label: "Update available", value: pkg.has_update ? "Yes" : "—" },
  ].filter((row) => row.value && row.value !== "—");

  return (
    <div className="pkg-detail">
      <div className="detail__head">
        <AppIcon pkg={pkg} title={title} size="detail" />
        <div className="detail__title">
          <h2>{title}</h2>
          <div className="detail__tags">
            <span
              className="detail__tag"
              style={{ color: SOURCE_COLORS[pkg.source], borderColor: SOURCE_COLORS[pkg.source] }}
            >
              {SOURCE_LABELS[pkg.source]}
            </span>
            <span
              className="detail__tag"
              style={{
                color: KIND_COLORS[pkg.app_kind],
                borderColor: KIND_COLORS[pkg.app_kind],
              }}
            >
              {KIND_LABELS[pkg.app_kind]}
            </span>
          </div>
        </div>
      </div>
      {pkg.description && <p className="detail__desc">{pkg.description}</p>}
      <dl className="detail__rows">
        {rows.map((r) => (
          <div key={r.label} className="detail__row">
            <dt>{r.label}</dt>
            <dd>{r.value}</dd>
          </div>
        ))}
      </dl>
    </div>
  );
}
