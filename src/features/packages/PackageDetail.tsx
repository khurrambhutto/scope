import { useState } from "react";
import type { InstalledPackage } from "../../shared/types/package";
import { SOURCE_COLORS, SOURCE_LABELS } from "../../shared/types/package";
import { formatSize, kindIcon } from "./format";
import { AppIcon } from "../../shared/components/AppIcon";
import { UninstallDialog } from "../uninstall/UninstallDialog";

export function PackageDetail({
  pkg,
  onUninstalled,
}: {
  pkg: InstalledPackage | null;
  onUninstalled?: (pkg: InstalledPackage) => void;
}) {
  const [uninstallTarget, setUninstallTarget] = useState<InstalledPackage | null>(null);
  if (!pkg) return null;

  const title = pkg.display_name ?? pkg.name;
  const rows: { label: string; value: string }[] = [
    { label: "Source", value: SOURCE_LABELS[pkg.source] },
    { label: "Install scope", value: pkg.install_scope ?? "—" },
    { label: "Package id", value: pkg.package_id },
    { label: "Version", value: pkg.version || "—" },
    { label: "Installed size", value: formatSize(pkg.size_bytes) },
    { label: "Kind", value: `${kindIcon(pkg.app_kind)} ${pkg.app_kind}` },
    { label: "Categories", value: pkg.categories ?? "—" },
    { label: "Runs in terminal", value: pkg.terminal ? "Yes" : "No" },
    { label: "Update available", value: pkg.has_update ? "Yes" : "—" },
  ];

  return (
    <div className="pkg-detail">
      <div className="detail__head">
        <AppIcon pkg={pkg} title={title} size="detail" />
        <div className="detail__title">
          <h2>{title}</h2>
          <span
            className="detail__source"
            style={{ color: SOURCE_COLORS[pkg.source], borderColor: SOURCE_COLORS[pkg.source] }}
          >
            {SOURCE_LABELS[pkg.source]}
          </span>
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
      <div className="detail__actions">
        <button
          type="button"
          className="btn btn--danger detail__uninstall"
          onClick={() => setUninstallTarget(pkg)}
        >
          Uninstall
        </button>
        <span className="detail__actions-hint">Preview-first · protected packages are blocked</span>
      </div>
      {uninstallTarget && (
        <UninstallDialog
          pkg={uninstallTarget}
          onClose={() => setUninstallTarget(null)}
          onUninstalled={(p) => {
            setUninstallTarget(null);
            onUninstalled?.(p);
          }}
        />
      )}
    </div>
  );
}
