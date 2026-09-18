import { memo } from "react";
import type { InstalledPackage } from "../../shared/types/package";
import { SOURCE_LABELS } from "../../shared/types/package";
import { formatSize } from "./format";
import { AppIcon } from "../../shared/components/AppIcon";

function PackageRowBase({
  pkg,
  selected,
  viewMode,
  onInfo,
  onUninstall,
  onUpdate,
}: {
  pkg: InstalledPackage;
  selected: boolean;
  viewMode: "uninstall" | "updates";
  onInfo: (pkg: InstalledPackage) => void;
  onUninstall: (pkg: InstalledPackage) => void;
  onUpdate: (pkg: InstalledPackage) => void;
}) {
  const title = pkg.display_name ?? pkg.name;
  const showUpdate = viewMode === "updates" && pkg.has_update;
  const showUninstall = viewMode === "uninstall";
  return (
    <div className={`pkg-row${selected ? " pkg-row--selected" : ""}`}>
      <AppIcon pkg={pkg} title={title} size="row" />
      <span className="pkg-row__main">
        <span className="pkg-row__title">{title}</span>
        <span className="pkg-row__meta-line">
          <span>{pkg.version || "—"}</span>
          <span>·</span>
          <span>{formatSize(pkg.size_bytes)}</span>
          <span>·</span>
          <span>{SOURCE_LABELS[pkg.source]}</span>
        </span>
      </span>
      <span className="pkg-row__actions">
        {showUpdate && (
          <button
            type="button"
            className="btn btn--primary btn--sm"
            onClick={() => onUpdate(pkg)}
          >
            Update
          </button>
        )}
        {showUninstall && (
          <button
            type="button"
            className="btn btn--danger btn--sm"
            onClick={() => onUninstall(pkg)}
          >
            Uninstall
          </button>
        )}
        <button
          type="button"
          className="btn btn--ghost btn--icon btn--sm"
          aria-label={`Details for ${title}`}
          aria-expanded={selected}
          title="Details"
          onClick={() => onInfo(pkg)}
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" aria-hidden="true">
            <circle cx="12" cy="12" r="9" stroke="currentColor" strokeWidth="2" />
            <path d="M12 11v5" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
            <circle cx="12" cy="8" r="1.2" fill="currentColor" />
          </svg>
        </button>
      </span>
    </div>
  );
}

/**
 * Memoized so opening a detail row, or typing in the search box, re-renders only
 * the rows that actually changed.
 */
export const PackageRow = memo(PackageRowBase);
