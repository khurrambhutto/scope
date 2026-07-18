import { useEffect, useRef, useState } from "react";
import type { InstalledPackage } from "../../shared/types/package";
import { SOURCE_LABELS } from "../../shared/types/package";
import type { Operation } from "../../shared/types/operations";
import { formatSize } from "./format";
import { AppIcon } from "../../shared/components/AppIcon";
import { KebabIcon, TrashIcon, UpdateIcon } from "../../shared/components/icons";
import type { ViewMode } from "./usePackages";

function RowMenu({
  pkg,
  onDetails,
  onAction,
}: {
  pkg: InstalledPackage;
  onDetails: () => void;
  onAction: (kind: Operation) => void;
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  return (
    <div className="row-menu" ref={ref}>
      <button
        type="button"
        className="row-menu__trigger"
        aria-label={`More actions for ${pkg.display_name ?? pkg.name}`}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={(e) => {
          e.stopPropagation();
          setOpen((o) => !o);
        }}
      >
        <KebabIcon size={16} />
      </button>
      {open && (
        <ul className="row-menu__menu" role="menu">
          <li
            role="menuitem"
            className="row-menu__item"
            onClick={(e) => {
              e.stopPropagation();
              setOpen(false);
              onDetails();
            }}
          >
            View details
          </li>
          {pkg.has_update && (
            <li
              role="menuitem"
              className="row-menu__item"
              onClick={(e) => {
                e.stopPropagation();
                setOpen(false);
                onAction("update");
              }}
            >
              Update{pkg.update_version ? ` to ${pkg.update_version}` : ""}
            </li>
          )}
          <li
            role="menuitem"
            className="row-menu__item row-menu__item--danger"
            onClick={(e) => {
              e.stopPropagation();
              setOpen(false);
              onAction("uninstall");
            }}
          >
            Uninstall
          </li>
        </ul>
      )}
    </div>
  );
}

export function PackageRow({
  pkg,
  selected,
  viewMode,
  busy,
  onSelect,
  onAction,
}: {
  pkg: InstalledPackage;
  selected: boolean;
  viewMode: ViewMode;
  busy: Operation | null;
  onSelect: (pkg: InstalledPackage) => void;
  onAction: (pkg: InstalledPackage, kind: Operation) => void;
}) {
  const title = pkg.display_name ?? pkg.name;

  return (
    <div
      role="button"
      tabIndex={0}
      className={`pkg-row${selected ? " pkg-row--selected" : ""}${busy ? " pkg-row--busy" : ""}`}
      onClick={() => onSelect(pkg)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect(pkg);
        }
      }}
    >
      <AppIcon pkg={pkg} title={title} size="row" />
      <span className="pkg-row__main">
        <span className="pkg-row__title">
          {title}
          {viewMode === "uninstall" && pkg.has_update && (
            <span className="pkg-row__update-badge" title="Update available">
              ↑
            </span>
          )}
        </span>
        <span className="pkg-row__meta-line">
          {viewMode === "updates" && pkg.has_update ? (
            <>
              <span>{pkg.version || "—"}</span>
              <span className="pkg-row__arrow">→</span>
              <span className="pkg-row__target">{pkg.update_version || "latest"}</span>
            </>
          ) : (
            <span>{pkg.version || "—"}</span>
          )}
          <span>·</span>
          <span>{formatSize(pkg.size_bytes)}</span>
          <span>·</span>
          <span>
            {SOURCE_LABELS[pkg.source]}
            {pkg.install_scope ? ` (${pkg.install_scope})` : ""}
          </span>
        </span>
      </span>

      <span className="pkg-row__actions" onClick={(e) => e.stopPropagation()}>
        {busy ? (
          <span className="pkg-row__busy">
            <span className="spinner spinner--sm" aria-hidden />
            {busy === "uninstall" ? "Removing…" : "Updating…"}
          </span>
        ) : (
          <>
            {viewMode === "updates" && pkg.has_update ? (
              <button
                type="button"
                className="btn btn--small btn--primary pkg-row__action"
                onClick={() => onAction(pkg, "update")}
              >
                <UpdateIcon size={14} />
                Update
              </button>
            ) : (
              <button
                type="button"
                className="btn btn--small btn--danger pkg-row__action"
                onClick={() => onAction(pkg, "uninstall")}
              >
                <TrashIcon size={14} />
                Uninstall
              </button>
            )}
            <RowMenu
              pkg={pkg}
              onDetails={() => onSelect(pkg)}
              onAction={(kind) => onAction(pkg, kind)}
            />
          </>
        )}
      </span>
    </div>
  );
}
