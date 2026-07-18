import type { InstalledPackage } from "../../shared/types/package";
import { SOURCE_BADGE_COLORS, SOURCE_LABELS } from "../../shared/types/package";
import type { Operation } from "../../shared/types/operations";
import { formatSize, kindIcon } from "./format";
import { AppIcon } from "../../shared/components/AppIcon";
import { TrashIcon, UpdateIcon, XIcon } from "../../shared/components/icons";

export function PackageDetail({
  pkg,
  busy,
  onClose,
  onAction,
}: {
  pkg: InstalledPackage;
  busy: Operation | null;
  onClose: () => void;
  onAction: (pkg: InstalledPackage, kind: Operation) => void;
}) {
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
    {
      label: "Update available",
      value: pkg.has_update ? pkg.update_version || "Yes" : "—",
    },
  ];

  return (
    <aside className="drawer" aria-label={`Details for ${title}`}>
      <div className="drawer__head">
        <AppIcon pkg={pkg} title={title} size="detail" />
        <div className="drawer__title">
          <h2>{title}</h2>
          <div className="drawer__badges">
            <span
              className="drawer__source"
              style={{
                color: SOURCE_BADGE_COLORS[pkg.source],
                borderColor: SOURCE_BADGE_COLORS[pkg.source],
              }}
            >
              {SOURCE_LABELS[pkg.source]}
            </span>
            {pkg.has_update && (
              <span className="drawer__update-badge">
                Update {pkg.update_version ? `→ ${pkg.update_version}` : "available"}
              </span>
            )}
          </div>
        </div>
        <button
          type="button"
          className="drawer__close"
          onClick={onClose}
          aria-label="Close details"
        >
          <XIcon size={16} />
        </button>
      </div>

      <div className="drawer__scroll">
        {pkg.description && <p className="drawer__desc">{pkg.description}</p>}
        <dl className="drawer__rows">
          {rows.map((r) => (
            <div key={r.label} className="drawer__row">
              <dt>{r.label}</dt>
              <dd>{r.value}</dd>
            </div>
          ))}
        </dl>
      </div>

      <div className="drawer__actions">
        {busy ? (
          <span className="drawer__busy">
            <span className="spinner spinner--sm" aria-hidden />
            {busy === "uninstall"
              ? "Removal in progress…"
              : "Update in progress…"}
          </span>
        ) : (
          <>
            {pkg.has_update && (
              <button
                type="button"
                className="btn btn--primary"
                onClick={() => onAction(pkg, "update")}
              >
                <UpdateIcon size={15} />
                Update{pkg.update_version ? ` to ${pkg.update_version}` : ""}
              </button>
            )}
            <button
              type="button"
              className="btn btn--danger"
              onClick={() => onAction(pkg, "uninstall")}
            >
              <TrashIcon size={15} />
              Uninstall
            </button>
          </>
        )}
        <span className="drawer__hint">
          Preview-first · protected packages are blocked
        </span>
      </div>
    </aside>
  );
}
