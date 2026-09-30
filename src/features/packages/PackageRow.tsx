import { memo, type KeyboardEvent } from "react";
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

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onInfo(pkg);
    }
  };

  return (
    <div
      className={`pkg-row${selected ? " pkg-row--selected" : ""}`}
      role="button"
      tabIndex={0}
      aria-expanded={selected}
      aria-label={`Details for ${title}`}
      onClick={() => onInfo(pkg)}
      onKeyDown={handleKeyDown}
    >
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
            onClick={(event) => {
              event.stopPropagation();
              onUpdate(pkg);
            }}
          >
            Update
          </button>
        )}
        {showUninstall && (
          <button
            type="button"
            className="btn btn--danger btn--sm"
            onClick={(event) => {
              event.stopPropagation();
              onUninstall(pkg);
            }}
          >
            Uninstall
          </button>
        )}
      </span>
    </div>
  );
}

/**
 * Memoized so opening a detail row, or typing in the search box, re-renders only
 * the rows that actually changed.
 */
export const PackageRow = memo(PackageRowBase);
